//! API key authentication middleware.
//!
//! Protects all routes except a small allowlist (root, health, Swagger docs,
//! OpenAPI schema) behind an `X-API-Key` header. When the configured key is
//! empty, the middleware is a no-op — useful for local dev without secrets.
//!
//! Mirrors the pattern used by the Go services (`tg-search-api`,
//! `tg-event-processor`, `tg-web-crawler-api`) so every TerraGuard internal
//! service authenticates the same way.

use std::future::{ready, Ready};
use std::pin::Pin;

use actix_web::body::EitherBody;
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpResponse};
use serde::Serialize;

#[derive(Clone)]
pub(crate) struct ApiKeyAuth {
    pub expected_key: String,
}

impl ApiKeyAuth {
    pub fn new(expected_key: String) -> Self {
        Self { expected_key }
    }
}

/// Paths that are always reachable without an API key.
///
/// Keep this list in sync with the public endpoints defined in `main.rs`.
/// Everything else requires a valid `X-API-Key` header.
fn is_public_path(path: &str) -> bool {
    // Root is public so uptime checks can hit `GET /` without credentials.
    if path == "/" {
        return true;
    }
    // Health check + OpenAPI JSON + Swagger UI static assets.
    if path == "/api/v1/health" || path == "/api/v1/openapi.json" {
        return true;
    }
    if path.starts_with("/api/v1/docs") {
        return true;
    }
    false
}

impl<S, B> Transform<S, ServiceRequest> for ApiKeyAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = ApiKeyAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ApiKeyAuthMiddleware {
            service,
            expected_key: self.expected_key.clone(),
        }))
    }
}

pub(crate) struct ApiKeyAuthMiddleware<S> {
    service: S,
    expected_key: String,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    success: bool,
    message: &'a str,
    payload: Option<()>,
}

impl<S, B> Service<ServiceRequest> for ApiKeyAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Empty key == middleware disabled. Matches the Go services' behavior:
        // local dev can run without any secret configured.
        if self.expected_key.is_empty() {
            let fut = self.service.call(req);
            return Box::pin(async move {
                fut.await.map(ServiceResponse::map_into_left_body)
            });
        }

        if is_public_path(req.path()) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                fut.await.map(ServiceResponse::map_into_left_body)
            });
        }

        let presented = req
            .headers()
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if presented == self.expected_key {
            let fut = self.service.call(req);
            return Box::pin(async move {
                fut.await.map(ServiceResponse::map_into_left_body)
            });
        }

        // Missing or wrong key — return the standard error envelope.
        let body = serde_json::to_string(&ErrorBody {
            success: false,
            message: "invalid or missing API key",
            payload: None,
        })
        .unwrap_or_else(|_| {
            r#"{"success":false,"message":"invalid or missing API key","payload":null}"#
                .to_string()
        });

        let response = HttpResponse::Unauthorized()
            .content_type("application/json")
            .body(body);

        let (request, _) = req.into_parts();
        Box::pin(async move {
            Ok(ServiceResponse::new(request, response).map_into_right_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_paths_bypass_auth() {
        assert!(is_public_path("/"));
        assert!(is_public_path("/api/v1/health"));
        assert!(is_public_path("/api/v1/openapi.json"));
        assert!(is_public_path("/api/v1/docs/"));
        assert!(is_public_path("/api/v1/docs/index.html"));
        assert!(is_public_path("/api/v1/docs/swagger-ui.css"));
    }

    #[test]
    fn protected_paths_require_auth() {
        assert!(!is_public_path("/api/v1/population"));
        assert!(!is_public_path("/api/v1/analyse"));
        assert!(!is_public_path("/api/v1/reverse"));
        assert!(!is_public_path("/api/v1/exposure"));
        assert!(!is_public_path("/api/v1/country"));
        // Close-but-not-public paths must also be protected.
        assert!(!is_public_path("/api/v1/healthz"));
        assert!(!is_public_path("/api/v1/health/status"));
        assert!(!is_public_path("/healthh"));
    }
}
