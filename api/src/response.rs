use actix_web::HttpResponse;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ApiResponse<T: Serialize> {
    pub code: u16,
    pub message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(payload: T) -> HttpResponse {
        HttpResponse::Ok().json(Self {
            code: 200,
            message: "success",
            payload: Some(payload),
        })
    }
}
