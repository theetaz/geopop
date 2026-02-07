use actix_web::HttpResponse;

use crate::models::responses::HealthPayload;
use crate::response::ApiResponse;

#[utoipa::path(
    get,
    path = "/health",
    tag = "System",
    responses((status = 200, description = "Service is healthy"))
)]
pub async fn health() -> HttpResponse {
    ApiResponse::ok(HealthPayload {
        status: "ok".to_string(),
    })
}
