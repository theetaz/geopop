use crate::errors::AppError;
use crate::grid;
use deadpool_postgres::Object;

pub struct PopulationRepository;

impl PopulationRepository {
    pub async fn get_population(
        client: &Object,
        lat: f64,
        lon: f64,
    ) -> Result<f32, AppError> {
        let cell = grid::cell_id(lat, lon)
            .ok_or_else(|| AppError::Validation(
                "Coordinates out of range. lat: [-90, 90], lon: [-180, 180)".to_string(),
            ))?;

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
        for (lat, lon) in points {
            let population = match grid::cell_id(*lat, *lon) {
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

    pub async fn get_cell_population(
        client: &Object,
        lat: f64,
        lon: f64,
    ) -> Result<f32, AppError> {
        match grid::cell_id(lat, lon) {
            Some(cell) => Ok(client
                .query_opt("SELECT pop FROM population WHERE cell_id = $1", &[&cell])
                .await?
                .map_or(0.0, |r| r.get(0))),
            None => Ok(0.0),
        }
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
