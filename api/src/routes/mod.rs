pub mod health;
pub mod population;
pub mod geocoding;
pub mod country;
pub mod exposure;

use actix_web::web;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .service(health::health)
            .service(population::get_population)
            .service(population::batch_population)
            .service(geocoding::reverse_geocode)
            .service(exposure::exposure)
            .service(country::country_lookup)
            .service(country::country_by_iso3)
            .service(country::countries_by_continent),
    );
}
