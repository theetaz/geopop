use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{AnalysePayload, CoordinateInfo, PointQuery, PopulationSummary};
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

    // Run country, geocoding, and epicentre lookups concurrently on separate connections
    let (country_res, place_res, epicentre_res) = tokio::join!(
        async {
            let c = pool.get().await.map_err(AppError::from)?;
            configure_conn(&c).await;
            CountryRepository::get_by_coordinate(&c, lat, lon).await
        },
        async {
            let c = pool.get().await.map_err(AppError::from)?;
            configure_conn(&c).await;
            GeocodingRepository::find_nearest_place(&c, lat, lon).await
        },
        async {
            let c = pool.get().await.map_err(AppError::from)?;
            configure_conn(&c).await;
            PopulationRepository::get_cell_population(&c, lat, lon).await
        },
    );

    let country = country_res?;
    let nearest_place = place_res?;
    let epicentre_pop = epicentre_res.unwrap_or(0.0);

    // Population radius search on its own connection
    let client = pool.get().await.map_err(AppError::from)?;
    configure_conn(&client).await;

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

async fn configure_conn(client: &deadpool_postgres::Object) {
    client.execute("SET jit = off", &[]).await.ok();
    client.execute("SET statement_timeout = '30s'", &[]).await.ok();
}

/// Exponential probe + binary search to find nearest populated radius.
/// Worst case ~13 queries instead of 200 with the old linear scan.
async fn find_population_radius(
    client: &deadpool_postgres::Object,
    lat: f64,
    lon: f64,
) -> Result<(f64, f64), AppError> {
    // Phase 1: exponential probe to find a bracket [lo, hi] where population appears
    let mut lo = 0.0_f64;
    let mut hi = STEP_KM;
    while hi <= MAX_RADIUS_KM {
        let pop = PopulationRepository::get_exposure_population(client, lat, lon, hi).await?;
        if pop > 0.0 {
            break;
        }
        lo = hi;
        hi = (hi * 2.0).min(MAX_RADIUS_KM);
        if lo >= MAX_RADIUS_KM {
            return Ok((MAX_RADIUS_KM, 0.0));
        }
    }

    // Phase 2: binary search within [lo, hi] to narrow down to STEP_KM precision
    while hi - lo > STEP_KM {
        let mid = ((lo + hi) / 2.0 / STEP_KM).round() * STEP_KM;
        if mid <= lo || mid >= hi {
            break;
        }
        let pop = PopulationRepository::get_exposure_population(client, lat, lon, mid).await?;
        if pop > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let pop = PopulationRepository::get_exposure_population(client, lat, lon, hi).await?;
    Ok((hi, pop))
}
