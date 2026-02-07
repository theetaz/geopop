use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::requests::ExposureQuery;
use crate::models::responses::{CoordinateInfo, ExposurePayload};
use crate::repositories::{GeocodingRepository, PopulationRepository};
use crate::response::ApiResponse;

const KM_PER_DEG: f64 = 111.32;

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[utoipa::path(
    get,
    path = "/exposure",
    tag = "Risk Assessment",
    params(
        ("lat" = f64, Query),
        ("lon" = f64, Query),
        ("radius" = Option<f64>, Query, description = "Radius in km (default: 1, max: 500)")
    ),
    responses(
        (status = 200, description = "Exposure analysis"),
        (status = 400, description = "Invalid parameters")
    )
)]
pub async fn exposure(
    pool: web::Data<Pool>,
    query: web::Query<ExposureQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {}", e))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let _ = client.execute("SET LOCAL jit = off", &[]).await;

    let (lat, lon, radius_km) = (query.lat, query.lon, query.radius);

    let total_pop = PopulationRepository::get_exposure_population(&client, lat, lon, radius_km)
        .await
        .map_err(AppError::from)?;
    let places = GeocodingRepository::get_exposed_places(&client, lat, lon, radius_km)
        .await
        .unwrap_or_default();
    let cell_pop = PopulationRepository::get_cell_population(&client, lat, lon)
        .await
        .unwrap_or(0.0);

    let deg = 1.0 / 120.0;
    let cell_area = deg * deg * KM_PER_DEG * KM_PER_DEG * lat.to_radians().cos();
    let cell_density = if cell_area > 0.0 {
        cell_pop as f64 / cell_area
    } else {
        0.0
    };
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
