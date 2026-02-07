use std::env;

pub(crate) const API_PREFIX: &str = "/api/v1";

pub(crate) struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub pool_size: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://geopop:geopop@localhost:5432/geopop".into()),
            host: env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: env::var("API_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            pool_size: env::var("POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|&s| s > 0)
                .unwrap_or(16),
        }
    }
}
