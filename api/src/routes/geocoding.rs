use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{PointQuery, ReversePayload};
use crate::repositories::GeocodingRepository;
use crate::response::ApiResponse;

/// Find the nearest named place for a given coordinate.
#[utoipa::path(
    get,
    path = "/reverse",
    tag = "Geocoding",
    summary = "Reverse geocode",
    description = "Returns the nearest named place (city, town, village, etc.) for the given \
        coordinate using the GeoNames gazetteer. The response includes a structured address \
        with administrative hierarchy (city, state, country).",
    params(
        ("lat" = f64, Query, description = "Latitude in decimal degrees", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Longitude in decimal degrees", example = 79.8612, minimum = -180, maximum = 180)
    ),
    responses(
        (status = 200, description = "Nearest named place found", body = ReversePayload),
        (status = 400, description = "Invalid or out-of-range coordinates"),
        (status = 404, description = "No named place found near the given coordinate")
    )
)]
pub(crate) async fn reverse_geocode(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let result = GeocodingRepository::reverse_geocode(&client, query.lat, query.lon).await?;

    Ok(ApiResponse::ok(result))
}
