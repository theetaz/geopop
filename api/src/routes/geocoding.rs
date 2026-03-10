use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{
    CoordinateInfo, ExposurePlacesQuery, ExposureQuery, LandCheckPayload,
    NearbyCountriesPayload, NearbyCitiesPayload, PointQuery, ReversePayload,
};
use crate::repositories::{CountryRepository, GeocodingRepository};
use crate::response::ApiResponse;

/// Find the nearest named place for a given coordinate.
#[utoipa::path(
    get,
    path = "/reverse",
    tag = "Geocoding",
    summary = "Reverse geocode",
    description = "Returns the nearest named place (city, town, village, etc.) for the given \
        coordinate using the GeoNames gazetteer. The response includes a structured address \
        with administrative hierarchy (city, state, country).",
    params(
        ("lat" = f64, Query, description = "Latitude in decimal degrees", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Longitude in decimal degrees", example = 79.8612, minimum = -180, maximum = 180)
    ),
    responses(
        (status = 200, description = "Nearest named place found", body = ReversePayload),
        (status = 400, description = "Invalid or out-of-range coordinates"),
        (status = 404, description = "No named place found near the given coordinate")
    )
)]
pub(crate) async fn reverse_geocode(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let result = GeocodingRepository::reverse_geocode(&client, query.lat, query.lon).await?;

    Ok(ApiResponse::ok(result))
}

/// Find all countries within a radius of a coordinate.
#[utoipa::path(
    get,
    path = "/geocoding/nearby-countries",
    tag = "Geocoding",
    summary = "Nearby countries",
    description = "Returns all countries whose boundary falls within the given radius. \
        Includes an `is_land` flag indicating whether the coordinate itself is on land or at sea.",
    params(
        ("lat" = f64, Query, description = "Centre latitude", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Centre longitude", example = 79.8612, minimum = -180, maximum = 180),
        ("radius" = Option<f64>, Query, description = "Search radius in km (default: 1, max: 500)", example = 50.0)
    ),
    responses(
        (status = 200, description = "Countries within radius", body = NearbyCountriesPayload),
        (status = 400, description = "Invalid parameters")
    )
)]
pub(crate) async fn nearby_countries(
    pool: web::Data<Pool>,
    query: web::Query<ExposureQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let (lat, lon, radius_km) = (query.lat, query.lon, query.radius);

    let is_land = CountryRepository::is_land(&client, lat, lon).await.unwrap_or(false);
    let countries = CountryRepository::get_nearby_countries(&client, lat, lon, radius_km).await?;

    Ok(ApiResponse::ok(NearbyCountriesPayload {
        coordinate: CoordinateInfo { lat, lon },
        radius_km,
        is_land,
        countries,
    }))
}

/// Paginated list of named places (cities) within a radius.
#[utoipa::path(
    get,
    path = "/geocoding/nearby-cities",
    tag = "Geocoding",
    summary = "Nearby cities (paginated)",
    description = "Returns a paginated list of named places from GeoNames within the given \
        radius, ordered by distance from the coordinate.",
    params(
        ("lat" = f64, Query, description = "Centre latitude", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Centre longitude", example = 79.8612, minimum = -180, maximum = 180),
        ("radius" = Option<f64>, Query, description = "Search radius in km (default: 1, max: 500)", example = 10.0),
        ("page" = Option<i64>, Query, description = "Page number (default: 1)", example = 1),
        ("per_page" = Option<i64>, Query, description = "Results per page (default: 20, max: 100)", example = 20)
    ),
    responses(
        (status = 200, description = "Paginated places list", body = NearbyCitiesPayload),
        (status = 400, description = "Invalid parameters")
    )
)]
pub(crate) async fn nearby_cities(
    pool: web::Data<Pool>,
    query: web::Query<ExposurePlacesQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let (lat, lon, radius_km) = (query.lat, query.lon, query.radius);
    let page = query.page;
    let per_page = query.per_page;
    let offset = (page - 1) * per_page;

    let total_places = GeocodingRepository::count_exposed_places(&client, lat, lon, radius_km)
        .await
        .unwrap_or(0);
    let places = GeocodingRepository::get_exposed_places(&client, lat, lon, radius_km, per_page, offset)
        .await
        .unwrap_or_default();

    Ok(ApiResponse::ok(NearbyCitiesPayload {
        coordinate: CoordinateInfo { lat, lon },
        radius_km,
        total_places,
        page,
        per_page,
        places,
    }))
}

/// Check whether a coordinate is on land or at sea.
#[utoipa::path(
    get,
    path = "/geocoding/land-check",
    tag = "Geocoding",
    summary = "Land or sea check",
    description = "Returns whether the coordinate is on land (inside a country polygon) or at sea. \
        If on land, also returns the containing country.",
    params(
        ("lat" = f64, Query, description = "Latitude in decimal degrees", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Longitude in decimal degrees", example = 79.8612, minimum = -180, maximum = 180)
    ),
    responses(
        (status = 200, description = "Land/sea check result", body = LandCheckPayload),
        (status = 400, description = "Invalid coordinates")
    )
)]
pub(crate) async fn land_check(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let (lat, lon) = (query.lat, query.lon);

    let country = CountryRepository::get_land_country(&client, lat, lon).await?;
    let is_land = country.is_some();

    Ok(ApiResponse::ok(LandCheckPayload {
        coordinate: CoordinateInfo { lat, lon },
        is_land,
        country,
    }))
}
