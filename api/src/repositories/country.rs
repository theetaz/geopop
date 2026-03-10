use crate::errors::AppError;
use crate::models::{CountryDetailPayload, CountryPayload, NearbyCountryEntry};
use deadpool_postgres::Object;

pub(crate) struct CountryRepository;

impl CountryRepository {
    pub async fn is_land(client: &Object, lat: f64, lon: f64) -> Result<bool, AppError> {
        let sql = r#"
            SELECT EXISTS(
                SELECT 1 FROM countries
                WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 4326))
            )
        "#;
        let row = client.query_one(sql, &[&lon, &lat]).await?;
        Ok(row.get(0))
    }

    pub async fn get_land_country(
        client: &Object,
        lat: f64,
        lon: f64,
    ) -> Result<Option<CountryPayload>, AppError> {
        let sql = r#"
            SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion
            FROM countries
            WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 4326))
            LIMIT 1
        "#;
        Ok(client
            .query_opt(sql, &[&lon, &lat])
            .await?
            .map(|r| Self::build_country_payload(&r)))
    }

    pub async fn get_nearby_countries(
        client: &Object,
        lat: f64,
        lon: f64,
        radius_km: f64,
    ) -> Result<Vec<NearbyCountryEntry>, AppError> {
        let sql = r#"
            SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion,
                   ST_Distance(geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography) / 1000.0
            FROM countries
            WHERE ST_DWithin(geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography, $3)
            ORDER BY ST_Distance(geom::geography, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography)
        "#;
        let rows = client.query(sql, &[&lon, &lat, &(radius_km * 1000.0)]).await?;
        Ok(rows
            .iter()
            .map(|r| {
                let distance_km: f64 = r.get(7);
                NearbyCountryEntry {
                    country: Self::build_country_payload(r),
                    distance_km: (distance_km * 100.0).round() / 100.0,
                }
            })
            .collect())
    }

    pub async fn get_by_coordinate(
        client: &Object,
        lat: f64,
        lon: f64,
    ) -> Result<CountryPayload, AppError> {
        let sql = r#"
            SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion
            FROM countries
            WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 4326))
            LIMIT 1
        "#;

        let row = match client.query_opt(sql, &[&lon, &lat]).await? {
            Some(r) => r,
            None => {
                let fallback = r#"
                    SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion
                    FROM countries ORDER BY geom <-> ST_SetSRID(ST_MakePoint($1, $2), 4326) LIMIT 1
                "#;
                client
                    .query_opt(fallback, &[&lon, &lat])
                    .await?
                    .ok_or_else(|| AppError::NotFound("No country found at this coordinate".into()))?
            }
        };

        Ok(Self::build_country_payload(&row))
    }

    pub async fn get_by_iso3(
        client: &Object,
        iso3: &str,
    ) -> Result<CountryDetailPayload, AppError> {
        let sql = r#"
            SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion,
                   pop_est, ST_XMin(geom), ST_YMin(geom), ST_XMax(geom), ST_YMax(geom)
            FROM countries WHERE UPPER(iso_a3) = $1 ORDER BY sovereign DESC LIMIT 1
        "#;

        let row = client
            .query_opt(sql, &[&iso3])
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Country not found: {iso3}")))?;

        Ok(CountryDetailPayload {
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

    pub async fn get_by_continent(
        client: &Object,
        continent: &str,
    ) -> Result<Vec<CountryPayload>, AppError> {
        let base = "SELECT iso_a2, iso_a3, name, formal_name, continent, region_un, subregion \
                    FROM countries WHERE sovereign = true AND iso_a2 IS NOT NULL AND iso_a3 IS NOT NULL";

        let rows = if continent == "americas" {
            client
                .query(&format!("{base} AND LOWER(region_un) = 'americas' ORDER BY name"), &[])
                .await?
        } else if continent == "north-america" {
            client
                .query(&format!("{base} AND LOWER(continent) = 'north america' ORDER BY name"), &[])
                .await?
        } else if continent == "south-america" {
            client
                .query(&format!("{base} AND LOWER(continent) = 'south america' ORDER BY name"), &[])
                .await?
        } else {
            client
                .query(&format!("{base} AND LOWER(region_un) = LOWER($1) ORDER BY name"), &[&continent])
                .await?
        };

        Ok(rows.iter().map(Self::build_country_payload).collect())
    }

    fn build_country_payload(row: &tokio_postgres::Row) -> CountryPayload {
        CountryPayload {
            iso_a2: row.get::<_, Option<String>>(0).map(|s| s.trim().to_string()),
            iso_a3: row.get::<_, Option<String>>(1).map(|s| s.trim().to_string()),
            name: row.get(2),
            formal_name: row.get(3),
            continent: row.get(4),
            region: row.get(5),
            subregion: row.get(6),
        }
    }
}
