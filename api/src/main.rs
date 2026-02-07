mod config;
mod grid;
mod handlers;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use deadpool_postgres::{Config as PgConfig, ManagerConfig, PoolConfig, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

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
        handlers::health,
        handlers::get_population,
        handlers::batch_population,
        handlers::reverse_geocode,
        handlers::exposure,
        handlers::country_lookup,
        handlers::country_by_iso3,
        handlers::countries_by_continent,
    ),
    components(schemas(
        handlers::PointQuery, handlers::PointResponse,
        handlers::BatchQuery, handlers::BatchResponse,
        handlers::ErrorResponse, handlers::HealthResponse,
        handlers::ReverseResponse, handlers::ExposureQuery,
        handlers::ExposureResponse, handlers::ExposedPlace,
        handlers::CoordinateInfo, handlers::CountryResponse,
        handlers::CountryDetailResponse, handlers::ContinentQuery,
        handlers::CountryListResponse,
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
    env_logger::init();
    let cfg = config::Config::from_env();

    let pg_config: tokio_postgres::Config = cfg.database_url
        .parse()
        .expect("Invalid DATABASE_URL");

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
    pool_cfg.pool = Some(PoolConfig::new(cfg.pool_size));

    let pool = pool_cfg.create_pool(Some(Runtime::Tokio1), NoTls)
        .expect("Failed to create connection pool");

    let bind = format!("{}:{}", cfg.host, cfg.port);
    log::info!("Starting GeoPop API on {bind}");
    log::info!("Swagger UI: http://{bind}/swagger-ui/");

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(pool.clone()))
            .service(SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .route("/health", web::get().to(handlers::health))
            .route("/population", web::get().to(handlers::get_population))
            .route("/population/batch", web::post().to(handlers::batch_population))
            .route("/reverse", web::get().to(handlers::reverse_geocode))
            .route("/exposure", web::get().to(handlers::exposure))
            .route("/country", web::get().to(handlers::country_lookup))
            .route("/country/{iso3}", web::get().to(handlers::country_by_iso3))
            .route("/countries", web::get().to(handlers::countries_by_continent))
    })
    .bind(&bind)?
    .run()
    .await
}
