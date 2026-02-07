use actix_web::HttpResponse;

use crate::models::HealthPayload;
use crate::response::ApiResponse;

/// Returns the current health status of the API service.
#[utoipa::path(
    get,
    path = "/health",
    tag = "System",
    summary = "Health check",
    description = "Returns the current health status of the API. Use this endpoint for uptime monitoring and load-balancer health probes.",
    responses(
        (status = 200, description = "Service is healthy", body = HealthPayload)
    )
)]
pub(crate) async fn health() -> HttpResponse {
    ApiResponse::ok(HealthPayload {
        status: "ok".into(),
    })
}
