use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt;

#[derive(Debug)]
pub(crate) enum AppError {
    Validation(String),
    Database(String),
    NotFound(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(msg) => write!(f, "validation error: {msg}"),
            Self::Database(msg) => write!(f, "database error: {msg}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
        }
    }
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            Self::Validation(msg) => HttpResponse::BadRequest().json(ErrorBody {
                success: false,
                message: msg,
                payload: None::<()>,
            }),
            Self::Database(msg) => {
                log::error!("Database error: {msg}");
                HttpResponse::InternalServerError().json(ErrorBody {
                    success: false,
                    message: "database connection error",
                    payload: None::<()>,
                })
            }
            Self::NotFound(msg) => HttpResponse::NotFound().json(ErrorBody {
                success: false,
                message: msg,
                payload: None::<()>,
            }),
        }
    }
}

impl From<tokio_postgres::Error> for AppError {
    fn from(err: tokio_postgres::Error) -> Self {
        let msg = if let Some(db_err) = err.as_db_error() {
            format!(
                "{}: {} (code: {})",
                db_err.severity(),
                db_err.message(),
                db_err.code().code()
            )
        } else {
            err.to_string()
        };
        Self::Database(msg)
    }
}

impl From<deadpool_postgres::PoolError> for AppError {
    fn from(err: deadpool_postgres::PoolError) -> Self {
        Self::Database(err.to_string())
    }
}

#[derive(Serialize)]
struct ErrorBody<'a, T: Serialize> {
    success: bool,
    message: &'a str,
    payload: Option<T>,
}
