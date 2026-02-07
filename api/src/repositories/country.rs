use crate::errors::AppError;
use crate::models::responses::{CountryDetailPayload, CountryPayload};
use deadpool_postgres::Object;

pub struct CountryRepository;

impl CountryRepository {
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
                    .ok_or_else(|| AppError::NotFound("No country found at this coordinate".to_string()))?
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
            .ok_or_else(|| AppError::NotFound(format!("Country not found: {}", iso3)))?;

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
                .query(
                    &format!("{base} AND LOWER(region_un) = 'americas' ORDER BY name"),
                    &[],
                )
                .await?
        } else if continent == "north-america" {
            client
                .query(
                    &format!("{base} AND LOWER(continent) = 'north america' ORDER BY name"),
                    &[],
                )
                .await?
        } else if continent == "south-america" {
            client
                .query(
                    &format!("{base} AND LOWER(continent) = 'south america' ORDER BY name"),
                    &[],
                )
                .await?
        } else {
            client
                .query(
                    &format!("{base} AND LOWER(region_un) = LOWER($1) ORDER BY name"),
                    &[&continent],
                )
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
