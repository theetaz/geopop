use crate::errors::AppError;
use crate::grid;
use crate::models::{CellBounds, GridCell};
use deadpool_postgres::Object;

const KM_PER_DEG: f64 = 111.32;
const ROW_MAX: i32 = 21599;

fn search_bounds(lat: f64, lon: f64, radius_km: f64) -> (i32, i32, i32, i32) {
    let dlat = radius_km / KM_PER_DEG;
    let cos_lat = lat.to_radians().cos().max(0.01);
    let dlon = radius_km / (KM_PER_DEG * cos_lat);
    (
        (((90.0 - (lat + dlat)) * 120.0).floor() as i32).clamp(0, ROW_MAX),
        (((90.0 - (lat - dlat)) * 120.0).floor() as i32).clamp(0, ROW_MAX),
        ((lon - dlon + 180.0) * 120.0).floor() as i32,
        ((lon + dlon + 180.0) * 120.0).floor() as i32,
    )
}

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

    /// Sum population within a circular radius.
    /// LATERAL forces PostgreSQL into nested loop + index scan on every row,
    /// preventing the planner from choosing a catastrophic hash join on 175M rows.
    pub async fn get_exposure_population(
        client: &Object,
        lat: f64,
        lon: f64,
        radius_km: f64,
    ) -> Result<f64, AppError> {
        let (min_row, max_row, min_col, max_col) = search_bounds(lat, lon, radius_km);
        let sql = r#"
            SELECT COALESCE(SUM(sub.pop), 0)::float8
            FROM generate_series($4::int, $5::int) AS r(r)
            CROSS JOIN LATERAL (
                SELECT p.pop, p.cell_id
                FROM population p
                WHERE p.cell_id BETWEEN r.r * 43200 + $6::int AND r.r * 43200 + $7::int
            ) sub
            WHERE 111.32 * sqrt(
                pow((90.0 - (sub.cell_id / 43200 + 0.5) / 120.0) - $1::float8, 2) +
                pow(((mod(sub.cell_id, 43200) + 0.5) / 120.0 - 180.0 - $2::float8) * cos(radians($1::float8)), 2)
            ) <= $3::float8
        "#;
        set_seqscan_off(client).await?;
        let query_result = client
            .query_one(sql, &[&lat, &lon, &radius_km, &min_row, &max_row, &min_col, &max_col])
            .await;
        reset_seqscan(client).await;
        Ok(query_result?.get(0))
    }

    /// Fast existence check: is there ANY populated cell within the bounding box?
    /// LATERAL + LIMIT 1 stops at the very first populated cell found — empty
    /// ocean rows cost a single B-tree probe that returns nothing.
    pub async fn has_population_within(
        client: &Object,
        lat: f64,
        lon: f64,
        search_km: f64,
    ) -> Result<bool, AppError> {
        let (min_row, max_row, min_col, max_col) = search_bounds(lat, lon, search_km);
        let sql = r#"
            SELECT EXISTS(
                SELECT 1
                FROM generate_series($1::int, $2::int) AS r(r)
                CROSS JOIN LATERAL (
                    SELECT 1 FROM population p
                    WHERE p.cell_id BETWEEN r.r * 43200 + $3::int AND r.r * 43200 + $4::int
                    AND p.pop > 0
                    LIMIT 1
                ) sub
            )
        "#;
        set_seqscan_off(client).await?;
        let query_result = client
            .query_one(sql, &[&min_row, &max_row, &min_col, &max_col])
            .await;
        reset_seqscan(client).await;
        Ok(query_result?.get(0))
    }
}

async fn set_seqscan_off(client: &Object) -> Result<(), AppError> {
    client.execute("SET enable_seqscan = off", &[]).await?;
    Ok(())
}

async fn reset_seqscan(client: &Object) {
    if let Err(err) = client.execute("SET enable_seqscan = on", &[]).await {
        log::warn!("failed to reset enable_seqscan session parameter: {err}");
    }
}

#[inline]
fn round5(v: f64) -> f64 {
    (v * 100_000.0).round() / 100_000.0
}
