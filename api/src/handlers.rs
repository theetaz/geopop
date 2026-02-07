use actix_web::{web, HttpResponse};
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::grid;

const MAX_BATCH_SIZE: usize = 1000;
const MAX_RADIUS_KM: f64 = 500.0;
const KM_PER_DEG: f64 = 111.32;

// ── Request / Response types ──

#[derive(Deserialize, ToSchema)]
pub struct PointQuery {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Serialize, ToSchema)]
pub struct PointResponse {
    pub lat: f64,
    pub lon: f64,
    pub population: f32,
    pub resolution_km: f32,
}

#[derive(Deserialize, ToSchema)]
pub struct BatchQuery {
    pub points: Vec<PointQuery>,
}

#[derive(Serialize, ToSchema)]
pub struct BatchResponse {
    pub results: Vec<PointResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Serialize, ToSchema)]
pub struct ReverseResponse {
    pub place_id: i32,
    pub lat: String,
    pub lon: String,
    pub name: String,
    pub display_name: String,
    pub address: HashMap<String, String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ExposureQuery {
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "default_radius")]
    pub radius: f64,
}

fn default_radius() -> f64 {
    1.0
}

#[derive(Serialize, ToSchema)]
pub struct ExposedPlace {
    pub place_id: i32,
    pub lat: String,
    pub lon: String,
    pub name: String,
    pub display_name: String,
    pub address: HashMap<String, String>,
    pub distance_km: f64,
}

#[derive(Serialize, ToSchema)]
pub struct ExposureResponse {
    pub coordinate: CoordinateInfo,
    pub radius_km: f64,
    pub total_population: f64,
    pub area_km2: f64,
    pub density_per_km2: f64,
    pub cell_population: f32,
    pub cell_area_km2: f64,
    pub cell_density_per_km2: f64,
    pub places: Vec<ExposedPlace>,
}

#[derive(Serialize, ToSchema)]
pub struct CoordinateInfo {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Serialize, ToSchema)]
pub struct CountryResponse {
    pub iso_a2: Option<String>,
    pub iso_a3: Option<String>,
    pub name: String,
    pub formal_name: Option<String>,
    pub continent: String,
    pub region: Option<String>,
    pub subregion: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct CountryDetailResponse {
    pub iso_a2: Option<String>,
    pub iso_a3: Option<String>,
    pub name: String,
    pub formal_name: Option<String>,
    pub continent: String,
    pub region: Option<String>,
    pub subregion: Option<String>,
    pub pop_est: Option<i64>,
    pub bbox: [f64; 4],
}

#[derive(Deserialize, ToSchema)]
pub struct ContinentQuery {
    pub continent: String,
}

#[derive(Serialize, ToSchema)]
pub struct CountryListResponse {
    pub continent: String,
    pub count: usize,
    pub countries: Vec<CountryResponse>,
}

// ── Validation ──

fn validate_coords(lat: f64, lon: f64) -> Result<(), HttpResponse> {
    if !lat.is_finite() || !lon.is_finite() || lat < -90.0 || lat > 90.0 || lon < -180.0 || lon >= 180.0 {
        return Err(HttpResponse::BadRequest().json(ErrorResponse {
            error: "Coordinates out of range. Lat: [-90, 90], Lon: [-180, 180)".into(),
        }));
    }
    Ok(())
}

fn db_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ErrorResponse {
        error: "Database connection error".into(),
    })
}

fn query_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ErrorResponse {
        error: "Query execution error".into(),
    })
}

// ── Handlers ──

#[utoipa::path(get, path = "/health", tag = "System",
    responses((status = 200, description = "Service is healthy", body = HealthResponse)))]
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(HealthResponse { status: "ok".into() })
}

#[utoipa::path(get, path = "/population", tag = "Population",
    params(("lat" = f64, Query), ("lon" = f64, Query)),
    responses(
        (status = 200, body = PointResponse),
        (status = 400, body = ErrorResponse)))]
pub async fn get_population(pool: web::Data<Pool>, query: web::Query<PointQuery>) -> HttpResponse {
    let cell = match grid::cell_id(query.lat, query.lon) {
        Some(id) => id,
        None => return HttpResponse::BadRequest().json(ErrorResponse {
            error: "Coordinates out of range. Lat: [-90, 90], Lon: [-180, 180)".into(),
        }),
    };

    let client = pool.get().await.map_err(|e| { log::error!("DB pool: {e}"); }).ok();
    let client = match client { Some(c) => c, None => return db_error() };

    let population = match client.query_opt("SELECT pop FROM population WHERE cell_id = $1", &[&cell]).await {
        Ok(Some(r)) => r.get::<_, f32>(0),
        Ok(None) => 0.0,
        Err(e) => { log::error!("Query: {e}"); return query_error(); }
    };

    HttpResponse::Ok().json(PointResponse {
        lat: query.lat, lon: query.lon, population, resolution_km: 1.0,
    })
}

#[utoipa::path(post, path = "/population/batch", tag = "Population",
    request_body = BatchQuery,
    responses(
        (status = 200, body = BatchResponse),
        (status = 400, body = ErrorResponse)))]
pub async fn batch_population(pool: web::Data<Pool>, body: web::Json<BatchQuery>) -> HttpResponse {
    if body.points.len() > MAX_BATCH_SIZE {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!("Maximum {MAX_BATCH_SIZE} points per batch request"),
        });
    }

    let client = match pool.get().await {
        Ok(c) => c, Err(e) => { log::error!("DB pool: {e}"); return db_error(); }
    };

    let stmt = match client.prepare_cached("SELECT pop FROM population WHERE cell_id = $1").await {
        Ok(s) => s, Err(e) => { log::error!("Prepare: {e}"); return query_error(); }
    };

    let mut results = Vec::with_capacity(body.points.len());
    for point in &body.points {
        let population = match grid::cell_id(point.lat, point.lon) {
            Some(cell) => client.query_opt(&stmt, &[&cell]).await
                .map(|r| r.map_or(0.0, |r| r.get::<_, f32>(0)))
                .unwrap_or(0.0),
            None => 0.0,
        };
        results.push(PointResponse {
            lat: point.lat, lon: point.lon, population, resolution_km: 1.0,
        });
    }

    HttpResponse::Ok().json(BatchResponse { results })
}

#[utoipa::path(get, path = "/reverse", tag = "Geocoding",
    params(("lat" = f64, Query), ("lon" = f64, Query)),
    responses(
        (status = 200, body = ReverseResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse)))]
pub async fn reverse_geocode(pool: web::Data<Pool>, query: web::Query<PointQuery>) -> HttpResponse {
    if let Err(e) = validate_coords(query.lat, query.lon) { return e; }

    let client = match pool.get().await {
        Ok(c) => c, Err(e) => { log::error!("DB pool: {e}"); return db_error(); }
    };

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

    let row = match client.query_opt(sql, &[&query.lon, &query.lat]).await {
        Ok(Some(r)) => r,
        Ok(None) => return HttpResponse::NotFound().json(ErrorResponse { error: "No nearby place found".into() }),
        Err(e) => { log::error!("Reverse geocode: {e}"); return query_error(); }
    };

    HttpResponse::Ok().json(build_reverse_response(&row))
}

#[utoipa::path(get, path = "/exposure", tag = "Risk Assessment",
    params(
        ("lat" = f64, Query), ("lon" = f64, Query),
        ("radius" = Option<f64>, Query, description = "Radius in km (default: 1, max: 500)")),
    responses(
        (status = 200, body = ExposureResponse),
        (status = 400, body = ErrorResponse)))]
pub async fn exposure(pool: web::Data<Pool>, query: web::Query<ExposureQuery>) -> HttpResponse {
    if let Err(e) = validate_coords(query.lat, query.lon) { return e; }

    let radius_km = query.radius;
    if !radius_km.is_finite() || radius_km <= 0.0 || radius_km > MAX_RADIUS_KM {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!("Radius must be between 0 and {MAX_RADIUS_KM} km"),
        });
    }

    let client = match pool.get().await {
        Ok(c) => c, Err(e) => { log::error!("DB pool: {e}"); return db_error(); }
    };

    // Disable JIT — compilation overhead far exceeds query time for these lookups
    let _ = client.execute("SET LOCAL jit = off", &[]).await;

    let (lat, lon) = (query.lat, query.lon);

    let total_pop = match query_exposure(&client, lat, lon, radius_km).await {
        Ok(v) => v,
        Err(e) => { log::error!("Exposure: {e}"); return query_error(); }
    };

    let places = query_exposed_places(&client, lat, lon, radius_km).await.unwrap_or_default();
    let cell_pop = query_cell_population(&client, lat, lon).await.unwrap_or(0.0);

    let deg = 1.0 / 120.0;
    let cell_area = deg * deg * KM_PER_DEG * KM_PER_DEG * lat.to_radians().cos();
    let cell_density = if cell_area > 0.0 { cell_pop as f64 / cell_area } else { 0.0 };
    let area = std::f64::consts::PI * radius_km * radius_km;
    let density = if area > 0.0 { total_pop / area } else { 0.0 };

    HttpResponse::Ok().json(ExposureResponse {
        coordinate: CoordinateInfo { lat, lon },
        radius_km,
        total_population: round1(total_pop),
        area_km2: round2(area),
        density_per_km2: round1(density),
        cell_population: cell_pop,
        cell_area_km2: round2(cell_area),
        cell_density_per_km2: round1(cell_density),
        places,
    })
}

#[utoipa::path(get, path = "/country", tag = "Country",
    params(("lat" = f64, Query), ("lon" = f64, Query)),
    responses(
        (status = 200, body = CountryResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse)))]
pub async fn country_lookup(pool: web::Data<Pool>, query: web::Query<PointQuery>) -> HttpResponse {
    if let Err(e) = validate_coords(query.lat, query.lon) { return e; }

    let client = match pool.get().await {
        Ok(c) => c, Err(e) => { log::error!("DB pool: {e}"); return db_error(); }
    };

    let sql = r#"
        SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion
        FROM countries
        WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 4326))
        LIMIT 1
    "#;

    let row = match client.query_opt(sql, &[&query.lon, &query.lat]).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            let fallback = r#"
                SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion
                FROM countries ORDER BY geom <-> ST_SetSRID(ST_MakePoint($1, $2), 4326) LIMIT 1
            "#;
            match client.query_opt(fallback, &[&query.lon, &query.lat]).await {
                Ok(Some(r)) => r,
                Ok(None) => return HttpResponse::NotFound().json(ErrorResponse {
                    error: "No country found at this coordinate".into(),
                }),
                Err(e) => { log::error!("Country fallback: {e}"); return query_error(); }
            }
        }
        Err(e) => { log::error!("Country lookup: {e}"); return query_error(); }
    };

    HttpResponse::Ok().json(build_country_response(&row))
}

#[utoipa::path(get, path = "/country/{iso3}", tag = "Country",
    params(("iso3" = String, Path)),
    responses(
        (status = 200, body = CountryDetailResponse),
        (status = 404, body = ErrorResponse)))]
pub async fn country_by_iso3(pool: web::Data<Pool>, path: web::Path<String>) -> HttpResponse {
    let iso3 = path.into_inner().to_uppercase();

    let client = match pool.get().await {
        Ok(c) => c, Err(e) => { log::error!("DB pool: {e}"); return db_error(); }
    };

    let sql = r#"
        SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion,
               pop_est, ST_XMin(geom), ST_YMin(geom), ST_XMax(geom), ST_YMax(geom)
        FROM countries WHERE UPPER(iso_a3) = $1 ORDER BY sovereign DESC LIMIT 1
    "#;

    let row = match client.query_opt(sql, &[&iso3]).await {
        Ok(Some(r)) => r,
        Ok(None) => return HttpResponse::NotFound().json(ErrorResponse {
            error: format!("Country not found: {iso3}"),
        }),
        Err(e) => { log::error!("Country ISO3: {e}"); return query_error(); }
    };

    HttpResponse::Ok().json(CountryDetailResponse {
        iso_a2: row.get::<_, Option<String>>(0).map(|s| s.trim().to_string()),
        iso_a3: row.get::<_, Option<String>>(1).map(|s| s.trim().to_string()),
        name: row.get(2),
        formal_name: row.get(3),
        continent: row.get(4),
        region: row.get(5),
        subregion: row.get(6),
        pop_est: row.get(7),
        bbox: [row.get(8), row.get(9), row.get(10), row.get(11)],
    })
}

#[utoipa::path(get, path = "/countries", tag = "Country",
    params(("continent" = String, Query)),
    responses(
        (status = 200, body = CountryListResponse),
        (status = 400, body = ErrorResponse)))]
pub async fn countries_by_continent(pool: web::Data<Pool>, query: web::Query<ContinentQuery>) -> HttpResponse {
    let continent = query.continent.trim().to_lowercase();
    if continent.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "Missing required parameter: continent".into(),
        });
    }

    let client = match pool.get().await {
        Ok(c) => c, Err(e) => { log::error!("DB pool: {e}"); return db_error(); }
    };

    let rows = if continent == "americas" {
        client.query(
            "SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion \
             FROM countries WHERE LOWER(region_un) = 'americas' \
             AND sovereign = true AND iso_a2 IS NOT NULL AND iso_a3 IS NOT NULL ORDER BY name",
            &[],
        ).await
    } else {
        client.query(
            "SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion \
             FROM countries WHERE LOWER(region_un) = LOWER($1) \
             AND sovereign = true AND iso_a2 IS NOT NULL AND iso_a3 IS NOT NULL ORDER BY name",
            &[&continent],
        ).await
    };

    let rows = match rows {
        Ok(r) => r, Err(e) => { log::error!("Countries: {e}"); return query_error(); }
    };

    let countries: Vec<CountryResponse> = rows.iter().map(|r| build_country_response(r)).collect();
    HttpResponse::Ok().json(CountryListResponse {
        continent: query.continent.clone(),
        count: countries.len(),
        countries,
    })
}

// ── Internal helpers ──

fn round1(v: f64) -> f64 { (v * 10.0).round() / 10.0 }
fn round2(v: f64) -> f64 { (v * 100.0).round() / 100.0 }

fn feature_code_to_address_key(code: &str) -> &'static str {
    match code {
        "PPLC" | "PPLA" | "PPLA2" | "PPL" => "city",
        "PPLA3" | "PPLA4" => "town",
        "PPLX" | "PPLL" | "PPLF" => "village",
        _ => "municipality",
    }
}

fn build_address(row: &tokio_postgres::Row, name: &str, fc: &str, cc: &str) -> (String, HashMap<String, String>) {
    let admin1: Option<String> = row.get(8);
    let admin2: Option<String> = row.get(9);
    let country: Option<String> = row.get(10);

    let mut parts = vec![name.to_string()];
    if let Some(ref a2) = admin2 { parts.push(a2.clone()); }
    if let Some(ref a1) = admin1 { parts.push(a1.clone()); }
    if let Some(ref cn) = country { parts.push(cn.clone()); }
    let display_name = parts.join(", ");

    let mut address = HashMap::new();
    address.insert(feature_code_to_address_key(fc).into(), name.to_string());
    if let Some(a2) = admin2 { address.insert("county".into(), a2); }
    if let Some(a1) = admin1 { address.insert("state".into(), a1); }
    if let Some(cn) = country { address.insert("country".into(), cn); }
    if !cc.is_empty() { address.insert("country_code".into(), cc.to_lowercase()); }

    (display_name, address)
}

fn build_reverse_response(row: &tokio_postgres::Row) -> ReverseResponse {
    let name: String = row.get(1);
    let fc = row.get::<_, Option<String>>(4).unwrap_or_default();
    let cc = row.get::<_, Option<String>>(5).unwrap_or_default();
    let (display_name, address) = build_address(row, &name, &fc, &cc);

    ReverseResponse {
        place_id: row.get(0),
        lat: format!("{}", row.get::<_, f64>(2)),
        lon: format!("{}", row.get::<_, f64>(3)),
        name, display_name, address,
    }
}

fn build_country_response(row: &tokio_postgres::Row) -> CountryResponse {
    CountryResponse {
        iso_a2: row.get::<_, Option<String>>(0).map(|s| s.trim().to_string()),
        iso_a3: row.get::<_, Option<String>>(1).map(|s| s.trim().to_string()),
        name: row.get(2),
        formal_name: row.get(3),
        continent: row.get(4),
        region: row.get(5),
        subregion: row.get(6),
    }
}

async fn query_cell_population(client: &deadpool_postgres::Object, lat: f64, lon: f64) -> Result<f32, tokio_postgres::Error> {
    match grid::cell_id(lat, lon) {
        Some(cell) => Ok(client.query_opt("SELECT pop FROM population WHERE cell_id = $1", &[&cell]).await?
            .map_or(0.0, |r| r.get(0))),
        None => Ok(0.0),
    }
}

/// Sum population within a circular radius using the canonical grid.
/// Uses generate_series to enumerate candidate cells in a bounding box,
/// then filters by actual distance.
async fn query_exposure(client: &deadpool_postgres::Object, lat: f64, lon: f64, radius_km: f64) -> Result<f64, tokio_postgres::Error> {
    let sql = r#"
        SELECT COALESCE(SUM(pop), 0)::float8
        FROM (
            SELECT p.pop
            FROM generate_series(
                GREATEST(FLOOR((90.0 - ($1::float8 + $3::float8/111.32)) * 120.0)::int, 0),
                LEAST(FLOOR((90.0 - ($1::float8 - $3::float8/111.32)) * 120.0)::int, 21599)
            ) r,
            generate_series(
                FLOOR(($2::float8 - $3::float8/(111.32 * cos(radians($1::float8))) + 180.0) * 120.0)::int,
                FLOOR(($2::float8 + $3::float8/(111.32 * cos(radians($1::float8))) + 180.0) * 120.0)::int
            ) c,
            population p
            WHERE p.cell_id = r.r * 43200 + c.c
            AND 111.32 * sqrt(
                pow((90.0 - (r.r + 0.5) / 120.0) - $1::float8, 2) +
                pow((((c.c + 0.5) / 120.0 - 180.0) - $2::float8) * cos(radians($1::float8)), 2)
            ) <= $3::float8
        ) sub
    "#;
    Ok(client.query_one(sql, &[&lat, &lon, &radius_km]).await?.get(0))
}

/// Find all GeoNames populated places within a radius using the geography index.
async fn query_exposed_places(client: &deadpool_postgres::Object, lat: f64, lon: f64, radius_km: f64) -> Result<Vec<ExposedPlace>, tokio_postgres::Error> {
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
    "#;

    let rows = client.query(sql, &[&lon, &lat, &(radius_km * 1000.0)]).await?;

    Ok(rows.iter().map(|row| {
        let name: String = row.get(1);
        let fc = row.get::<_, Option<String>>(4).unwrap_or_default();
        let cc = row.get::<_, Option<String>>(5).unwrap_or_default();
        let (display_name, address) = build_address(row, &name, &fc, &cc);

        ExposedPlace {
            place_id: row.get(0),
            lat: format!("{}", row.get::<_, f64>(2)),
            lon: format!("{}", row.get::<_, f64>(3)),
            name, display_name, address,
            distance_km: round2(row.get::<_, f64>(11)),
        }
    }).collect())
}
