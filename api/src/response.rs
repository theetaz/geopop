use actix_web::HttpResponse;
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: u16,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(payload: T) -> HttpResponse {
        HttpResponse::Ok().json(ApiResponse {
            code: 200,
            message: "success".to_string(),
            payload: Some(payload),
        })
    }

    pub fn created(payload: T) -> HttpResponse {
        HttpResponse::Created().json(ApiResponse {
            code: 201,
            message: "created".to_string(),
            payload: Some(payload),
        })
    }
}
