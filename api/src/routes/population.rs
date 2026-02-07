use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use utoipa::OpenApi;
use validator::Validate;

use crate::errors::AppError;
use crate::models::requests::{BatchQuery, PointQuery};
use crate::models::responses::{BatchPayload, PointPayload};
use crate::repositories::PopulationRepository;
use crate::response::ApiResponse;
use crate::validation::validate_batch_size;

#[utoipa::path(
    get,
    path = "/population",
    tag = "Population",
    params(("lat" = f64, Query), ("lon" = f64, Query)),
    responses(
        (status = 200, description = "Population at coordinate"),
        (status = 400, description = "Invalid coordinates")
    )
)]
pub async fn get_population(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {}", e)).into()
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let population = PopulationRepository::get_population(&client, query.lat, query.lon)
        .await
        .map_err(|e| AppError::from(e).into())?;

    Ok(ApiResponse::ok(PointPayload {
        lat: query.lat,
        lon: query.lon,
        population,
        resolution_km: 1.0,
    }))
}

#[utoipa::path(
    post,
    path = "/population/batch",
    tag = "Population",
    request_body = BatchQuery,
    responses(
        (status = 200, description = "Batch population results"),
        (status = 400, description = "Invalid request")
    )
)]
pub async fn batch_population(
    pool: web::Data<Pool>,
    body: web::Json<BatchQuery>,
) -> ActixResult<HttpResponse> {
    body.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {}", e)).into()
    })?;

    validate_batch_size(body.points.len()).map_err(|e| e.into())?;

    let client = pool.get().await.map_err(AppError::from)?;
    let points: Vec<(f64, f64)> = body.points.iter().map(|p| (p.lat, p.lon)).collect();
    let populations = PopulationRepository::get_batch_population(&client, &points)
        .await
        .map_err(|e| AppError::from(e).into())?;

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
