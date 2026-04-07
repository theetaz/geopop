use std::env;

pub(crate) const API_PREFIX: &str = "/api/v1";

pub(crate) struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub pool_size: usize,
    /// Shared-secret expected in the `X-API-Key` header on protected routes.
    ///
    /// Empty string disables the auth middleware entirely (local dev default).
    /// Production deployments must set this to match the value configured on
    /// every consumer (tg-backend-api, tg-event-processor, etc.).
    pub api_key: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://geopop:geopop@localhost:5432/geopop".into()),
            host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("API_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            pool_size: env::var("POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|&s| s > 0)
                .unwrap_or(32),
            api_key: env::var("API_KEY").unwrap_or_default(),
        }
    }
}
