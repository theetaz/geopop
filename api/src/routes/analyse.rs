use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{
    AnalysePayload, CoordinateInfo, NearestPlace, PointQuery, PopulationSummary,
};
use crate::repositories::{CountryRepository, GeocodingRepository, PopulationRepository};
use crate::response::ApiResponse;

const STEP_KM: f64 = 5.0;
const MAX_RADIUS_KM: f64 = 1000.0;

#[inline]
fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[inline]
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Disaster impact analysis with auto-expanding radius search.
#[utoipa::path(
    get,
    path = "/analyse",
    tag = "Risk Assessment",
    summary = "Disaster impact analysis",
    description = "Comprehensive disaster impact analysis for a coordinate. Takes only lat/lon — \
        no radius needed.\n\n\
        The endpoint automatically:\n\
        1. Identifies the country (or nearest country if in ocean)\n\
        2. Finds the nearest named place (city/town/village) with distance and direction\n\
        3. Checks population at the epicentre grid cell\n\
        4. If no population at the epicentre, expands the search radius in 5 km increments \
           (up to 1000 km) until population is found\n\n\
        The `population.search_radius_km` field indicates how remote the epicentre is — \
        a value of 5 means population was found within 5 km; a value of 500 means \
        the nearest populated area is ~500 km away.\n\n\
        Ideal for disaster events where the epicentre may be in ocean, desert, or uninhabited terrain.",
    params(
        ("lat" = f64, Query, description = "Epicentre latitude in decimal degrees", example = 20.4657, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Epicentre longitude in decimal degrees", example = 93.9572, minimum = -180, maximum = 180)
    ),
    responses(
        (status = 200, description = "Disaster impact analysis results", body = AnalysePayload),
        (status = 400, description = "Invalid or out-of-range coordinates")
    )
)]
pub(crate) async fn analyse(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let (lat, lon) = (query.lat, query.lon);
    let client = pool.get().await.map_err(AppError::from)?;
    let _ = client.execute("SET LOCAL jit = off", &[]).await;

    // Step 1: Country and nearest place (independent — could be concurrent but share the connection)
    let country = CountryRepository::get_by_coordinate(&client, lat, lon).await?;
    let nearest_place = GeocodingRepository::find_nearest_place(&client, lat, lon).await?;

    // Step 2: Check epicentre cell population
    let epicentre_pop = PopulationRepository::get_cell_population(&client, lat, lon)
        .await
        .unwrap_or(0.0);

    // Step 3: Find population — expand radius if epicentre is empty
    let (search_radius, total_pop) = if epicentre_pop > 0.0 {
        let pop = PopulationRepository::get_exposure_population(&client, lat, lon, STEP_KM).await?;
        (STEP_KM, pop)
    } else {
        find_population_radius(&client, lat, lon).await?
    };

    let area = std::f64::consts::PI * search_radius * search_radius;
    let density = if area > 0.0 { total_pop / area } else { 0.0 };

    Ok(ApiResponse::ok(AnalysePayload {
        coordinate: CoordinateInfo { lat, lon },
        country,
        nearest_place,
        population: PopulationSummary {
            search_radius_km: search_radius,
            total_population: round1(total_pop),
            area_km2: round2(area),
            density_per_km2: round1(density),
            epicentre_population: epicentre_pop,
        },
    }))
}

/// Expand radius in STEP_KM increments until population > 0 or MAX_RADIUS_KM is reached.
async fn find_population_radius(
    client: &deadpool_postgres::Object,
    lat: f64,
    lon: f64,
) -> Result<(f64, f64), AppError> {
    let mut radius = STEP_KM;
    while radius <= MAX_RADIUS_KM {
        let pop = PopulationRepository::get_exposure_population(client, lat, lon, radius).await?;
        if pop > 0.0 {
            return Ok((radius, pop));
        }
        radius += STEP_KM;
    }
    // No population found even at max radius
    Ok((MAX_RADIUS_KM, 0.0))
}
