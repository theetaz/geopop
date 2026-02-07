use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Validation(String),
    Database(String),
    NotFound(String),
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AppError::Database(msg) => write!(f, "Database error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::Validation(msg) => HttpResponse::BadRequest().json(ErrorResponse {
                code: 400,
                message: msg.clone(),
                payload: None,
            }),
            AppError::Database(msg) => {
                log::error!("Database error: {}", msg);
                HttpResponse::InternalServerError().json(ErrorResponse {
                    code: 500,
                    message: "Database connection error".to_string(),
                    payload: None,
                })
            }
            AppError::NotFound(msg) => HttpResponse::NotFound().json(ErrorResponse {
                code: 404,
                message: msg.clone(),
                payload: None,
            }),
            AppError::Internal(msg) => {
                log::error!("Internal error: {}", msg);
                HttpResponse::InternalServerError().json(ErrorResponse {
                    code: 500,
                    message: "Internal server error".to_string(),
                    payload: None,
                })
            }
        }
    }
}

impl From<tokio_postgres::Error> for AppError {
    fn from(err: tokio_postgres::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for AppError {
    fn from(err: deadpool_postgres::PoolError) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<AppError> for actix_web::Error {
    fn from(err: AppError) -> Self {
        actix_web::error::InternalError::from_response("", err.error_response()).into()
    }
}


#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: u16,
    pub message: String,
    pub payload: Option<()>,
}
