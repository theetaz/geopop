use crate::errors::AppError;
use crate::grid;
use crate::models::{CellBounds, GridCell};
use deadpool_postgres::Object;

pub(crate) struct PopulationRepository;

impl PopulationRepository {
    pub async fn get_population(client: &Object, lat: f64, lon: f64) -> Result<f32, AppError> {
        let cell = grid::cell_id(lat, lon).ok_or_else(|| {
            AppError::Validation("Coordinates out of range. lat: [-90, 90], lon: [-180, 180)".into())
        })?;

        let population = client
            .query_opt("SELECT pop FROM population WHERE cell_id = $1", &[&cell])
            .await?
            .map_or(0.0, |r| r.get::<_, f32>(0));

        Ok(population)
    }

    pub async fn get_batch_population(
        client: &Object,
        points: &[(f64, f64)],
    ) -> Result<Vec<f32>, AppError> {
        let stmt = client
            .prepare_cached("SELECT pop FROM population WHERE cell_id = $1")
            .await?;

        let mut results = Vec::with_capacity(points.len());
        for &(lat, lon) in points {
            let population = match grid::cell_id(lat, lon) {
                Some(cell) => client
                    .query_opt(&stmt, &[&cell])
                    .await?
                    .map_or(0.0, |r| r.get::<_, f32>(0)),
                None => 0.0,
            };
            results.push(population);
        }

        Ok(results)
    }

    pub async fn get_cell_population(client: &Object, lat: f64, lon: f64) -> Result<f32, AppError> {
        match grid::cell_id(lat, lon) {
            Some(cell) => Ok(client
                .query_opt("SELECT pop FROM population WHERE cell_id = $1", &[&cell])
                .await?
                .map_or(0.0, |r| r.get(0))),
            None => Ok(0.0),
        }
    }

    /// Returns all non-empty grid cells within a radius, with their centre coordinates and bounds.
    pub async fn get_grid_cells(
        client: &Object,
        lat: f64,
        lon: f64,
        radius_km: f64,
    ) -> Result<Vec<GridCell>, AppError> {
        let sql = r#"
            SELECT r.r, c.c, p.pop
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
            AND p.pop > 0
            AND 111.32 * sqrt(
                pow((90.0 - (r.r + 0.5) / 120.0) - $1::float8, 2) +
                pow((((c.c + 0.5) / 120.0 - 180.0) - $2::float8) * cos(radians($1::float8)), 2)
            ) <= $3::float8
            ORDER BY p.pop DESC
        "#;

        let rows = client.query(sql, &[&lat, &lon, &radius_km]).await?;
        let step = 1.0 / 120.0;

        Ok(rows
            .iter()
            .map(|row| {
                let r: i32 = row.get(0);
                let c: i32 = row.get(1);
                let pop: f32 = row.get(2);
                let center_lat = 90.0 - (r as f64 + 0.5) * step;
                let center_lon = (c as f64 + 0.5) * step - 180.0;
                let min_lat = 90.0 - (r as f64 + 1.0) * step;
                let max_lat = 90.0 - r as f64 * step;
                let min_lon = c as f64 * step - 180.0;
                let max_lon = (c as f64 + 1.0) * step - 180.0;

                GridCell {
                    lat: round5(center_lat),
                    lon: round5(center_lon),
                    population: pop,
                    bounds: CellBounds {
                        min_lat: round5(min_lat),
                        max_lat: round5(max_lat),
                        min_lon: round5(min_lon),
                        max_lon: round5(max_lon),
                    },
                }
            })
            .collect())
    }

    pub async fn get_exposure_population(
        client: &Object,
        lat: f64,
        lon: f64,
        radius_km: f64,
    ) -> Result<f64, AppError> {
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
}

#[inline]
fn round5(v: f64) -> f64 {
    (v * 100_000.0).round() / 100_000.0
}
