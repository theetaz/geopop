use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{CoordinateInfo, ExposurePayload, ExposureQuery};
use crate::repositories::{GeocodingRepository, PopulationRepository};
use crate::response::ApiResponse;

const KM_PER_DEG: f64 = 111.32;

#[inline]
fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[inline]
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Analyse population exposure within a circular area around a coordinate.
#[utoipa::path(
    get,
    path = "/exposure",
    tag = "Risk Assessment",
    summary = "Population exposure analysis",
    description = "Calculates the total estimated population within a circular area of the given \
        radius around the coordinate. Also returns population density metrics and a list of \
        named places found inside the search area. Useful for disaster risk assessment, \
        infrastructure planning, and impact analysis.\n\n\
        The analysis combines WorldPop 1 km grid data with GeoNames place data.",
    params(
        ("lat" = f64, Query, description = "Centre latitude in decimal degrees", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Centre longitude in decimal degrees", example = 79.8612, minimum = -180, maximum = 180),
        ("radius" = Option<f64>, Query, description = "Search radius in kilometres (default: 1, max: 500)", example = 10.0)
    ),
    responses(
        (status = 200, description = "Exposure analysis results", body = ExposurePayload),
        (status = 400, description = "Invalid coordinates or radius out of range (0–500 km)")
    )
)]
pub(crate) async fn exposure(
    pool: web::Data<Pool>,
    query: web::Query<ExposureQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    client.execute("SET jit = off", &[]).await.ok();
    client.execute("SET statement_timeout = '30s'", &[]).await.ok();

    let (lat, lon, radius_km) = (query.lat, query.lon, query.radius);

    let total_pop = PopulationRepository::get_exposure_population(&client, lat, lon, radius_km).await?;
    let places = GeocodingRepository::get_exposed_places(&client, lat, lon, radius_km)
        .await
        .unwrap_or_default();
    let cell_pop = PopulationRepository::get_cell_population(&client, lat, lon)
        .await
        .unwrap_or(0.0);

    let deg = 1.0 / 120.0;
    let cell_area = deg * deg * KM_PER_DEG * KM_PER_DEG * lat.to_radians().cos();
    let cell_density = if cell_area > 0.0 { cell_pop as f64 / cell_area } else { 0.0 };
    let area = std::f64::consts::PI * radius_km * radius_km;
    let density = if area > 0.0 { total_pop / area } else { 0.0 };

    Ok(ApiResponse::ok(ExposurePayload {
        coordinate: CoordinateInfo { lat, lon },
        radius_km,
        total_population: round1(total_pop),
        area_km2: round2(area),
        density_per_km2: round1(density),
        cell_population: cell_pop,
        cell_area_km2: round2(cell_area),
        cell_density_per_km2: round1(cell_density),
        places,
    }))
}
