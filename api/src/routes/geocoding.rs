use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use utoipa::OpenApi;
use validator::Validate;

use crate::errors::AppError;
use crate::models::requests::PointQuery;
use crate::repositories::GeocodingRepository;
use crate::response::ApiResponse;

#[utoipa::path(
    get,
    path = "/reverse",
    tag = "Geocoding",
    params(("lat" = f64, Query), ("lon" = f64, Query)),
    responses(
        (status = 200, description = "Nearest place"),
        (status = 400, description = "Invalid coordinates"),
        (status = 404, description = "No place found")
    )
)]
pub async fn reverse_geocode(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {}", e)).into()
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let result = GeocodingRepository::reverse_geocode(&client, query.lat, query.lon)
        .await
        .map_err(|e| AppError::from(e).into())?;

    Ok(ApiResponse::ok(result))
}
