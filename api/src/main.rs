mod config;
mod errors;
mod grid;
mod models;
mod repositories;
mod response;
mod routes;
mod validation;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use deadpool_postgres::{Config as PgConfig, ManagerConfig, PoolConfig, RecyclingMethod, Runtime, Timeouts};
use env_logger::Env;
use native_tls::{Certificate, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use std::{env, fs};
use tokio_postgres::NoTls;
use utoipa::openapi::Server;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::API_PREFIX;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "GeoPop API",
        description = "High-performance API for global population lookups, reverse geocoding, \
            country boundaries, and disaster risk exposure assessment.\n\n\
            Data sources: WorldPop 1km grid, Natural Earth boundaries, GeoNames places.",
        version = "1.0.0"
    ),
    paths(
        routes::health::health,
        routes::population::get_population,
        routes::population::batch_population,
        routes::geocoding::reverse_geocode,
        routes::exposure::exposure,
        routes::analyse::analyse,
        routes::country::country_lookup,
        routes::country::country_by_iso3,
        routes::country::countries_by_continent,
    ),
    components(schemas(
        models::PointQuery, models::PopulationQuery, models::PointPayload,
        models::BatchQuery, models::BatchPayload,
        models::PopulationGridPayload, models::GridCell, models::CellBounds,
        models::HealthPayload, models::ReversePayload,
        models::ExposureQuery, models::ExposurePayload,
        models::ExposedPlace, models::CoordinateInfo,
        models::AnalysePayload, models::NearestPlace, models::PopulationSummary,
        models::CountryPayload, models::CountryDetailPayload,
        models::ContinentQuery, models::CountryListPayload,
    )),
    tags(
        (name = "System", description = "Health and status"),
        (name = "Population", description = "WorldPop 1km grid lookups"),
        (name = "Geocoding", description = "Reverse geocoding via GeoNames"),
        (name = "Risk Assessment", description = "Population exposure analysis"),
        (name = "Country", description = "Country lookup via Natural Earth"),
    )
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();
    let cfg = config::Config::from_env();

    let pg_config: tokio_postgres::Config = cfg.database_url
        .parse()
        .expect("invalid DATABASE_URL");

    let mut pool_cfg = PgConfig::new();
    if let Some(host) = pg_config.get_hosts().first() {
        match host {
            tokio_postgres::config::Host::Tcp(h) => pool_cfg.host = Some(h.clone()),
            #[cfg(unix)]
            tokio_postgres::config::Host::Unix(p) => pool_cfg.host = Some(p.to_string_lossy().into()),
        }
    }
    if let Some(port) = pg_config.get_ports().first() { pool_cfg.port = Some(*port); }
    if let Some(user) = pg_config.get_user() { pool_cfg.user = Some(user.into()); }
    if let Some(pw) = pg_config.get_password() { pool_cfg.password = Some(String::from_utf8_lossy(pw).into()); }
    if let Some(db) = pg_config.get_dbname() { pool_cfg.dbname = Some(db.into()); }

    pool_cfg.manager = Some(ManagerConfig { recycling_method: RecyclingMethod::Fast });
    let mut pool_config = PoolConfig::new(cfg.pool_size);
    pool_config.timeouts = Timeouts {
        wait: Some(std::time::Duration::from_secs(5)),
        create: Some(std::time::Duration::from_secs(5)),
        recycle: Some(std::time::Duration::from_secs(5)),
    };
    pool_cfg.pool = Some(pool_config);

    let ssl_mode = DbSslMode::from_database_url(&cfg.database_url);
    let pool = if ssl_mode == DbSslMode::Disable {
        log::warn!("Database TLS mode: disabled (sslmode=disable)");
        pool_cfg
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .expect("failed to create database connection pool")
    } else {
        let mut tls_builder = TlsConnector::builder();
        if matches!(ssl_mode, DbSslMode::Require | DbSslMode::Prefer) {
            // Match libpq `sslmode=require`: encrypt traffic but skip cert/hostname checks.
            tls_builder.danger_accept_invalid_certs(true);
            tls_builder.danger_accept_invalid_hostnames(true);
        }
        add_ssl_root_cert_if_present(&cfg.database_url, &mut tls_builder);

        let native_tls = tls_builder
            .build()
            .expect("failed to initialize TLS connector");
        let tls = MakeTlsConnector::new(native_tls);
        log::info!("Database TLS mode: {}", ssl_mode.as_str());
        pool_cfg
            .create_pool(Some(Runtime::Tokio1), tls)
            .expect("failed to create TLS database connection pool")
    };

    let bind = format!("{}:{}", cfg.host, cfg.port);
    log::info!("Starting GeoPop API on {bind}");
    log::info!("Swagger UI: http://{bind}{API_PREFIX}/docs/");

    let mut openapi = ApiDoc::openapi();
    openapi.servers = Some(vec![Server::new(API_PREFIX)]);

    let openapi_url: &'static str = Box::leak(format!("{API_PREFIX}/openapi.json").into_boxed_str());
    let docs_path: &'static str = Box::leak(format!("{API_PREFIX}/docs/{{_:.*}}").into_boxed_str());

    HttpServer::new(move || {
        App::new()
            .wrap(
                Logger::new(r#"%a "%r" %s %b %Dms "%{User-Agent}i""#)
                    .exclude("/api/v1/health"),
            )
            .wrap(Cors::permissive())
            .app_data(web::Data::new(pool.clone()))
            .service(SwaggerUi::new(docs_path).url(openapi_url, openapi.clone()))
            .service(
                web::scope(API_PREFIX)
                    .route("/health", web::get().to(routes::health::health))
                    .route("/population", web::get().to(routes::population::get_population))
                    .route("/population/batch", web::post().to(routes::population::batch_population))
                    .route("/reverse", web::get().to(routes::geocoding::reverse_geocode))
                    .route("/exposure", web::get().to(routes::exposure::exposure))
                    .route("/analyse", web::get().to(routes::analyse::analyse))
                    .route("/country", web::get().to(routes::country::country_lookup))
                    .route("/country/{iso3}", web::get().to(routes::country::country_by_iso3))
                    .route("/countries", web::get().to(routes::country::countries_by_continent))
            )
    })
    .bind(&bind)?
    .run()
    .await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DbSslMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl DbSslMode {
    fn from_database_url(database_url: &str) -> Self {
        match extract_query_param(database_url, "sslmode")
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("disable") => Self::Disable,
            Some("verify-ca") => Self::VerifyCa,
            Some("verify-full") => Self::VerifyFull,
            Some("require") => Self::Require,
            Some("prefer") => Self::Prefer,
            _ => Self::Disable,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disabled",
            Self::Prefer => "prefer (TLS with non-strict verification)",
            Self::Require => "require (TLS with non-strict verification)",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }
}

fn extract_query_param(database_url: &str, key: &str) -> Option<String> {
    let (_, query) = database_url.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if name.eq_ignore_ascii_case(key) {
            Some(value.to_string())
        } else {
            None
        }
    })
}

fn add_ssl_root_cert_if_present(database_url: &str, tls_builder: &mut native_tls::TlsConnectorBuilder) {
    let cert_path = extract_query_param(database_url, "sslrootcert")
        .or_else(|| env::var("PGSSLROOTCERT").ok())
        .or_else(|| env::var("DATABASE_SSL_ROOT_CERT").ok());

    let Some(cert_path) = cert_path else {
        return;
    };

    match fs::read(&cert_path) {
        Ok(cert_bytes) => match Certificate::from_pem(&cert_bytes) {
            Ok(cert) => {
                tls_builder.add_root_certificate(cert);
                log::info!("Loaded database root certificate from {cert_path}");
            }
            Err(err) => {
                log::warn!("Failed to parse database root certificate at {cert_path}: {err}");
            }
        },
        Err(err) => {
            log::warn!("Failed to read database root certificate at {cert_path}: {err}");
        }
    }
}
