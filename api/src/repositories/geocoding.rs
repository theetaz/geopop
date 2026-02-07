use crate::errors::AppError;
use crate::models::responses::{ExposedPlace, ReversePayload};
use deadpool_postgres::Object;
use std::collections::HashMap;

pub struct GeocodingRepository;

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
            .ok_or_else(|| AppError::NotFound("No nearby place found".to_string()))?;

        Ok(Self::build_reverse_payload(&row))
    }

    pub async fn get_exposed_places(
        client: &Object,
        lat: f64,
        lon: f64,
        radius_km: f64,
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
        "#;

        let rows = client
            .query(sql, &[&lon, &lat, &(radius_km * 1000.0)])
            .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let name: String = row.get(1);
                let fc = row.get::<_, Option<String>>(4).unwrap_or_default();
                let cc = row.get::<_, Option<String>>(5).unwrap_or_default();
                let (display_name, address) = Self::build_address(row, &name, &fc, &cc);

                ExposedPlace {
                    place_id: row.get(0),
                    lat: format!("{}", row.get::<_, f64>(2)),
                    lon: format!("{}", row.get::<_, f64>(3)),
                    name,
                    display_name,
                    address,
                    distance_km: Self::round2(row.get::<_, f64>(11)),
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
        if let Some(ref a2) = admin2 {
            parts.push(a2.clone());
        }
        if let Some(ref a1) = admin1 {
            parts.push(a1.clone());
        }
        if let Some(ref cn) = country {
            parts.push(cn.clone());
        }
        let display_name = parts.join(", ");

        let mut address = HashMap::new();
        address.insert(Self::feature_code_to_address_key(fc).into(), name.to_string());
        if let Some(a2) = admin2 {
            address.insert("county".into(), a2);
        }
        if let Some(a1) = admin1 {
            address.insert("state".into(), a1);
        }
        if let Some(cn) = country {
            address.insert("country".into(), cn);
        }
        if !cc.is_empty() {
            address.insert("country_code".into(), cc.to_lowercase());
        }

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

    fn round2(v: f64) -> f64 {
        (v * 100.0).round() / 100.0
    }
}
