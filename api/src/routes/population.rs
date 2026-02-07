use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{BatchPayload, BatchQuery, PointPayload, PointQuery};
use crate::repositories::PopulationRepository;
use crate::response::ApiResponse;
use crate::validation::validate_batch_size;

/// Look up estimated population at a single coordinate.
#[utoipa::path(
    get,
    path = "/population",
    tag = "Population",
    summary = "Population at a coordinate",
    description = "Returns the estimated population for the 1 km² WorldPop grid cell that contains \
        the given coordinate. Data source: WorldPop 2025 Unconstrained 1 km resolution.",
    params(
        ("lat" = f64, Query, description = "Latitude in decimal degrees", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Longitude in decimal degrees", example = 79.8612, minimum = -180, maximum = 180)
    ),
    responses(
        (status = 200, description = "Population at the given coordinate", body = PointPayload),
        (status = 400, description = "Invalid or out-of-range coordinates")
    )
)]
pub(crate) async fn get_population(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let population = PopulationRepository::get_population(&client, query.lat, query.lon).await?;

    Ok(ApiResponse::ok(PointPayload {
        lat: query.lat,
        lon: query.lon,
        population,
        resolution_km: 1.0,
    }))
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
