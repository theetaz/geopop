use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{
    BatchPayload, BatchQuery, CoordinateInfo, PointPayload, PointQuery,
    PopulationGridPayload, PopulationQuery,
};
use crate::repositories::PopulationRepository;
use crate::response::ApiResponse;
use crate::validation::validate_batch_size;

/// Look up population at a coordinate, optionally within a radius to get individual grid cells.
#[utoipa::path(
    get,
    path = "/population",
    tag = "Population",
    summary = "Population lookup",
    description = "Without `radius`: returns the estimated population for the single 1 km² WorldPop \
        grid cell at the given coordinate.\n\n\
        With `radius` (max 10 km): returns all non-empty 1 km² grid cells within the circle, \
        including each cell's centre point and geographic bounds — ideal for map visualisation. \
        Cells are sorted by population descending.\n\n\
        Data source: WorldPop 2025 Unconstrained 1 km resolution.",
    params(
        ("lat" = f64, Query, description = "Latitude in decimal degrees", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Longitude in decimal degrees", example = 79.8612, minimum = -180, maximum = 180),
        ("radius" = Option<f64>, Query, description = "Optional search radius in km. When provided, returns all non-empty grid cells within the circle (max: 10 km).", example = 5.0)
    ),
    responses(
        (status = 200, description = "Population data — single cell (no radius) or grid cells (with radius)"),
        (status = 400, description = "Invalid coordinates or radius out of range (0–10 km)")
    )
)]
pub(crate) async fn get_population(
    pool: web::Data<Pool>,
    query: web::Query<PopulationQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;

    match query.radius {
        Some(radius_km) => {
            let cells = PopulationRepository::get_grid_cells(
                &client, query.lat, query.lon, radius_km,
            ).await?;
            let total: f64 = cells.iter().map(|c| c.population as f64).sum();

            Ok(ApiResponse::ok(PopulationGridPayload {
                coordinate: CoordinateInfo { lat: query.lat, lon: query.lon },
                radius_km,
                total_population: (total * 10.0).round() / 10.0,
                cell_count: cells.len(),
                cells,
            }))
        }
        None => {
            let population = PopulationRepository::get_population(
                &client, query.lat, query.lon,
            ).await?;

            Ok(ApiResponse::ok(PointPayload {
                lat: query.lat,
                lon: query.lon,
                population,
                resolution_km: 1.0,
            }))
        }
    }
}

/// Look up estimated population for multiple coordinates in a single request.
#[utoipa::path(
    post,
    path = "/population/batch",
    tag = "Population",
    summary = "Batch population lookup",
    description = "Accepts an array of coordinate points (1–1000) and returns the estimated \
        population for each 1 km² grid cell. All points are queried in a single database round-trip \
        for optimal performance.",
    request_body(
        content = BatchQuery,
        description = "JSON body with an array of coordinate points",
        example = json!({"points": [{"lat": 6.9271, "lon": 79.8612}, {"lat": 7.2906, "lon": 80.6337}]})
    ),
    responses(
        (status = 200, description = "Population results for all queried points", body = BatchPayload),
        (status = 400, description = "Invalid coordinates or batch size exceeds 1000")
    )
)]
pub(crate) async fn batch_population(
    pool: web::Data<Pool>,
    body: web::Json<BatchQuery>,
) -> ActixResult<HttpResponse> {
    body.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;
    validate_batch_size(body.points.len())?;

    let client = pool.get().await.map_err(AppError::from)?;
    let points: Vec<(f64, f64)> = body.points.iter().map(|p| (p.lat, p.lon)).collect();
    let populations = PopulationRepository::get_batch_population(&client, &points).await?;

    let results: Vec<PointPayload> = body
        .points
        .iter()
        .zip(populations.iter())
        .map(|(point, &pop)| PointPayload {
            lat: point.lat,
            lon: point.lon,
            population: pop,
            resolution_km: 1.0,
        })
        .collect();

    Ok(ApiResponse::ok(BatchPayload { results }))
}
