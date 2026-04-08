use crate::errors::AppError;
use crate::models::{CityHit, ExposedPlace, NearestPlace, ReversePayload};
use deadpool_postgres::Object;
use std::collections::HashMap;

pub(crate) struct GeocodingRepository;

impl GeocodingRepository {
    pub async fn reverse_geocode(
        client: &Object,
        lat: f64,
        lon: f64,
    ) -> Result<ReversePayload, AppError> {
        let sql = r#"
            SELECT g.geonameid, g.name, g.latitude, g.longitude,
                   g.feature_code, g.country_code, g.admin1_code, g.admin2_code,
                   a1.name, a2.name, c.name
            FROM geonames g
            LEFT JOIN admin1_codes a1 ON a1.code = g.country_code || '.' || g.admin1_code
            LEFT JOIN admin2_codes a2 ON a2.code = g.country_code || '.' || g.admin1_code || '.' || g.admin2_code
            LEFT JOIN countries c ON c.iso_a2 = g.country_code
            ORDER BY g.geom <-> ST_SetSRID(ST_MakePoint($1, $2), 4326)
            LIMIT 1
        "#;

        let row = client
            .query_opt(sql, &[&lon, &lat])
            .await?
            .ok_or_else(|| AppError::NotFound("No nearby place found".into()))?;

        Ok(Self::build_reverse_payload(&row))
    }

    /// Fuzzy city search for Google-Places-style autocomplete.
    ///
    /// Strategy:
    ///   * Short queries (< 4 chars): prefix-only via idx_geonames_name_lower.
    ///     Trigram fuzziness on 3-char inputs matches ~50K rows and blows up the
    ///     heap scan, so we skip it entirely.
    ///   * Longer queries (>= 4 chars): prefix OR trigram. Trigram is bounded by
    ///     the pg_trgm.similarity_threshold (0.35 — tight enough to stay fast,
    ///     loose enough to catch common typos like "lonon" → "London").
    ///
    /// Ranking: `match_quality + population_boost`.
    /// - match_quality = 1.0 exact | 0.9 prefix | similarity() fuzzy
    /// - population_boost is a log-scaled bump of up to 0.8. A megacity like
    ///   London (pop 9M) gets ~+0.75, enough to beat a population-0 village
    ///   that happens to share the exact typed string (e.g. "Londo" or
    ///   "Lononwei" vs "London").
    pub async fn search_cities(
        client: &Object,
        query: &str,
        country: Option<&str>,
        limit: i64,
        min_population: i64,
    ) -> Result<Vec<CityHit>, AppError> {
        let use_fuzzy = query.chars().count() >= 4;

        if use_fuzzy {
            // Tighter threshold = fewer rows pulled from the trigram index.
            // 0.35 still catches typical typos ("lonon" → "London") on 4+ char queries.
            client
                .batch_execute("SET LOCAL pg_trgm.similarity_threshold = 0.35")
                .await?;
        }

        // Feature codes we consider "a city the user might search for":
        //   PPLC  = capital
        //   PPLA  = first-order admin capital (state/province)
        //   PPLA2 = second-order admin capital
        //   PPLA3 = third-order admin capital
        //   PPLA4 = fourth-order admin capital
        //   PPL   = generic populated place
        //   PPLG  = seat of government
        // Everything else (PPLX sections, PPLL localities, PPLF farms, PPLH historical,
        // STLMT settlements, ...) is excluded to keep results tight.
        //
        // The WHERE clause toggles the trigram branch based on `use_fuzzy`.
        // The score/ORDER BY are shared so both paths produce the same shape.
        let match_clause = if use_fuzzy {
            "(LOWER(g.name) LIKE LOWER($1) || '%' OR g.name % $1)"
        } else {
            "(LOWER(g.name) LIKE LOWER($1) || '%')"
        };

        let sql = format!(
            r#"
            SELECT g.geonameid, g.name, g.latitude, g.longitude,
                   g.feature_code, g.country_code, g.admin1_code, g.admin2_code,
                   a1.name AS admin1_name,
                   a2.name AS admin2_name,
                   c.name  AS country_name,
                   COALESCE(g.population, 0) AS population,
                   (
                       CASE
                           WHEN LOWER(g.name) = LOWER($1)           THEN 1.0::float8
                           WHEN LOWER(g.name) LIKE LOWER($1) || '%' THEN 0.9::float8
                           ELSE similarity(g.name, $1)::float8
                       END
                       + LEAST(
                           0.8::float8,
                           0.8::float8 * LN(GREATEST(COALESCE(g.population, 0), 1)::float8 + 1.0) / 17.0::float8
                         )
                   ) AS score
            FROM geonames g
            LEFT JOIN admin1_codes a1 ON a1.code = g.country_code || '.' || g.admin1_code
            LEFT JOIN admin2_codes a2 ON a2.code = g.country_code || '.' || g.admin1_code || '.' || g.admin2_code
            LEFT JOIN countries   c  ON c.iso_a2 = g.country_code
            WHERE g.feature_code IN ('PPLC','PPLA','PPLA2','PPLA3','PPLA4','PPL','PPLG')
              AND COALESCE(g.population, 0) >= $4
              AND ($2::char(2) IS NULL OR g.country_code = $2)
              AND {match_clause}
            ORDER BY score DESC, population DESC NULLS LAST, g.name ASC
            LIMIT $3
        "#,
            match_clause = match_clause,
        );

        let country_param: Option<String> = country.map(|c| c.to_uppercase());
        let rows = client
            .query(
                sql.as_str(),
                &[&query, &country_param, &limit, &min_population],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let name: String = row.get(1);
                let lat: f64 = row.get(2);
                let lon: f64 = row.get(3);
                let fc = row.get::<_, Option<String>>(4);
                let cc = row.get::<_, Option<String>>(5);
                let admin1: Option<String> = row.get(8);
                let admin2: Option<String> = row.get(9);
                let country_name: Option<String> = row.get(10);
                let population: i64 = row.get(11);
                let score: f64 = row.get(12);

                let mut parts = vec![name.clone()];
                if let Some(ref a1) = admin1 { parts.push(a1.clone()); }
                if let Some(ref cn) = country_name { parts.push(cn.clone()); }
                let display_name = parts.join(", ");

                let bbox = bbox_from_population(lat, lon, population);

                CityHit {
                    place_id: row.get(0),
                    name,
                    display_name,
                    country_code: cc.map(|s| s.trim().to_string()),
                    country: country_name,
                    admin1,
                    admin2,
                    feature_code: fc,
                    lat,
                    lon,
                    population,
                    score: round3(score),
                    bbox,
                }
            })
            .collect())
    }

    /// Find the single nearest named place globally (KNN, no radius limit) with distance and direction.
    pub async fn find_nearest_place(
        client: &Object,
        lat: f64,
        lon: f64,
    ) -> Result<NearestPlace, AppError> {
        let sql = r#"
            SELECT g.geonameid, g.name, g.latitude, g.longitude,
                   g.feature_code, g.country_code, g.admin1_code, g.admin2_code,
                   a1.name, a2.name, c.name,
                   ST_Distance(g.geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography) / 1000.0
            FROM geonames g
            LEFT JOIN admin1_codes a1 ON a1.code = g.country_code || '.' || g.admin1_code
            LEFT JOIN admin2_codes a2 ON a2.code = g.country_code || '.' || g.admin1_code || '.' || g.admin2_code
            LEFT JOIN countries c ON c.iso_a2 = g.country_code
            ORDER BY g.geom <-> ST_SetSRID(ST_MakePoint($1, $2), 4326)
            LIMIT 1
        "#;

        let row = client
            .query_opt(sql, &[&lon, &lat])
            .await?
            .ok_or_else(|| AppError::NotFound("No nearby place found".into()))?;

        let name: String = row.get(1);
        let place_lat: f64 = row.get(2);
        let place_lon: f64 = row.get(3);
        let fc = row.get::<_, Option<String>>(4).unwrap_or_default();
        let cc = row.get::<_, Option<String>>(5).unwrap_or_default();
        let (display_name, address) = Self::build_address(&row, &name, &fc, &cc);
        let bearing = bearing_deg(lat, lon, place_lat, place_lon);

        Ok(NearestPlace {
            place_id: row.get(0),
            name,
            display_name,
            address,
            distance_km: round2(row.get::<_, f64>(11)),
            direction: compass_direction(bearing),
            bearing_deg: round1(bearing),
        })
    }

    pub async fn count_exposed_places(
        client: &Object,
        lat: f64,
        lon: f64,
        radius_km: f64,
    ) -> Result<i64, AppError> {
        let sql = r#"
            SELECT COUNT(*)::bigint
            FROM geonames g
            WHERE ST_DWithin(g.geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography, $3)
        "#;
        let row = client.query_one(sql, &[&lon, &lat, &(radius_km * 1000.0)]).await?;
        Ok(row.get(0))
    }

    pub async fn get_exposed_places(
        client: &Object,
        lat: f64,
        lon: f64,
        radius_km: f64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ExposedPlace>, AppError> {
        let sql = r#"
            SELECT g.geonameid, g.name, g.latitude, g.longitude,
                   g.feature_code, g.country_code, g.admin1_code, g.admin2_code,
                   a1.name, a2.name, c.name,
                   ST_Distance(g.geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography) / 1000.0
            FROM geonames g
            LEFT JOIN admin1_codes a1 ON a1.code = g.country_code || '.' || g.admin1_code
            LEFT JOIN admin2_codes a2 ON a2.code = g.country_code || '.' || g.admin1_code || '.' || g.admin2_code
            LEFT JOIN countries c ON c.iso_a2 = g.country_code
            WHERE ST_DWithin(g.geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography, $3)
            ORDER BY ST_Distance(g.geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography)
            LIMIT $4 OFFSET $5
        "#;

        let rows = client
            .query(sql, &[&lon, &lat, &(radius_km * 1000.0), &limit, &offset])
            .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let name: String = row.get(1);
                let place_lat: f64 = row.get(2);
                let place_lon: f64 = row.get(3);
                let fc = row.get::<_, Option<String>>(4).unwrap_or_default();
                let cc = row.get::<_, Option<String>>(5).unwrap_or_default();
                let (display_name, address) = Self::build_address(row, &name, &fc, &cc);
                let bearing = bearing_deg(lat, lon, place_lat, place_lon);

                ExposedPlace {
                    place_id: row.get(0),
                    lat: format!("{place_lat}"),
                    lon: format!("{place_lon}"),
                    name,
                    display_name,
                    address,
                    distance_km: round2(row.get::<_, f64>(11)),
                    direction: compass_direction(bearing),
                    bearing_deg: round1(bearing),
                }
            })
            .collect())
    }

    fn feature_code_to_address_key(code: &str) -> &'static str {
        match code {
            "PPLC" | "PPLA" | "PPLA2" | "PPL" => "city",
            "PPLA3" | "PPLA4" => "town",
            "PPLX" | "PPLL" | "PPLF" => "village",
            _ => "municipality",
        }
    }

    fn build_address(
        row: &tokio_postgres::Row,
        name: &str,
        fc: &str,
        cc: &str,
    ) -> (String, HashMap<String, String>) {
        let admin1: Option<String> = row.get(8);
        let admin2: Option<String> = row.get(9);
        let country: Option<String> = row.get(10);

        let mut parts = vec![name.to_string()];
        if let Some(ref a2) = admin2 { parts.push(a2.clone()); }
        if let Some(ref a1) = admin1 { parts.push(a1.clone()); }
        if let Some(ref cn) = country { parts.push(cn.clone()); }
        let display_name = parts.join(", ");

        let mut address = HashMap::with_capacity(5);
        address.insert(Self::feature_code_to_address_key(fc).into(), name.to_string());
        if let Some(a2) = admin2 { address.insert("district".into(), a2); }
        if let Some(a1) = admin1 { address.insert("state".into(), a1); }
        if let Some(cn) = country { address.insert("country".into(), cn); }
        if !cc.is_empty() { address.insert("country_code".into(), cc.to_lowercase()); }

        (display_name, address)
    }

    fn build_reverse_payload(row: &tokio_postgres::Row) -> ReversePayload {
        let name: String = row.get(1);
        let fc = row.get::<_, Option<String>>(4).unwrap_or_default();
        let cc = row.get::<_, Option<String>>(5).unwrap_or_default();
        let (display_name, address) = Self::build_address(row, &name, &fc, &cc);

        ReversePayload {
            place_id: row.get(0),
            lat: format!("{}", row.get::<_, f64>(2)),
            lon: format!("{}", row.get::<_, f64>(3)),
            name,
            display_name,
            address,
        }
    }
}

#[inline]
fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[inline]
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[inline]
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Synthesise a crude bounding box for a city when no real polygon is available.
/// Radius grows with population so "London" gets a ~20km box and a hamlet gets ~1km.
/// This is deliberately approximate — it exists so the frontend always has *something*
/// to frame/zoom to until OSM admin boundaries are ingested.
fn bbox_from_population(lat: f64, lon: f64, population: i64) -> [f64; 4] {
    // Heuristic: sqrt(pop) / 200, clamped to [1.0, 30.0] km.
    let radius_km = ((population.max(0) as f64).sqrt() / 200.0).clamp(1.0, 30.0);
    let dlat = radius_km / 111.0; // 1° lat ≈ 111 km
    let dlon = radius_km / (111.0 * lat.to_radians().cos().abs().max(1e-6));
    [
        round3(lon - dlon),
        round3(lat - dlat),
        round3(lon + dlon),
        round3(lat + dlat),
    ]
}

/// Compute initial bearing (forward azimuth) from point 1 to point 2 in degrees (0–360).
fn bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (lat1, lat2) = (lat1.to_radians(), lat2.to_radians());
    let d_lon = (lon2 - lon1).to_radians();
    let x = d_lon.sin() * lat2.cos();
    let y = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * d_lon.cos();
    (x.atan2(y).to_degrees() + 360.0) % 360.0
}

/// Convert a bearing in degrees to an 8-point compass direction.
fn compass_direction(deg: f64) -> String {
    const DIRS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    DIRS[((deg + 22.5) % 360.0 / 45.0) as usize].into()
}
