use actix_web::HttpResponse;
use serde::Serialize;

/// Standard API response wrapper matching the Python backend's CommonResponse.
#[derive(Serialize)]
pub(crate) struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub message: &'static str,
    pub payload: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(payload: T) -> HttpResponse {
        HttpResponse::Ok().json(Self {
            success: true,
            message: "success",
            payload: Some(payload),
        })
    }
}
