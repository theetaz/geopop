use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{ContinentQuery, CountryDetailPayload, CountryListPayload, CountryPayload, PointQuery};
use crate::repositories::CountryRepository;
use crate::response::ApiResponse;
use crate::validation::validate_continent;

/// Identify which country contains a given coordinate.
#[utoipa::path(
    get,
    path = "/country",
    tag = "Country",
    summary = "Country by coordinate",
    description = "Returns the country that contains the given coordinate using Natural Earth \
        boundary polygons. Includes ISO codes, formal name, continent, region, and sub-region.",
    params(
        ("lat" = f64, Query, description = "Latitude in decimal degrees", example = 6.9271, minimum = -90, maximum = 90),
        ("lon" = f64, Query, description = "Longitude in decimal degrees", example = 79.8612, minimum = -180, maximum = 180)
    ),
    responses(
        (status = 200, description = "Country found at the given coordinate", body = CountryPayload),
        (status = 400, description = "Invalid or out-of-range coordinates"),
        (status = 404, description = "Coordinate is in international waters or unclaimed territory")
    )
)]
pub(crate) async fn country_lookup(
    pool: web::Data<Pool>,
    query: web::Query<PointQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let client = pool.get().await.map_err(AppError::from)?;
    let result = CountryRepository::get_by_coordinate(&client, query.lat, query.lon).await?;

    Ok(ApiResponse::ok(result))
}

/// Look up detailed country information by ISO-3166 alpha-3 code.
#[utoipa::path(
    get,
    path = "/country/{iso3}",
    tag = "Country",
    summary = "Country by ISO-3 code",
    description = "Returns detailed country information including population estimate and \
        geographic bounding box for the given ISO-3166 alpha-3 code.\n\n\
        Examples: `USA`, `GBR`, `LKA`, `IND`, `AUS`",
    params(
        ("iso3" = String, Path, description = "ISO-3166 alpha-3 country code (3 uppercase letters)", example = "LKA")
    ),
    responses(
        (status = 200, description = "Country details found", body = CountryDetailPayload),
        (status = 400, description = "Invalid ISO code format — must be exactly 3 letters"),
        (status = 404, description = "No country found for the given ISO code")
    )
)]
pub(crate) async fn country_by_iso3(
    pool: web::Data<Pool>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let iso3 = crate::validation::validate_iso3(&path.into_inner())?;

    let client = pool.get().await.map_err(AppError::from)?;
    let result = CountryRepository::get_by_iso3(&client, &iso3).await?;

    Ok(ApiResponse::ok(result))
}

/// List all countries belonging to a continent.
#[utoipa::path(
    get,
    path = "/countries",
    tag = "Country",
    summary = "Countries by continent",
    description = "Returns a list of all countries in the specified continent. \
        Valid continent values: `asia`, `europe`, `africa`, `oceania`, `americas`, \
        `north-america`, `south-america` (case-insensitive).",
    params(
        ("continent" = String, Query, description = "Continent name", example = "asia")
    ),
    responses(
        (status = 200, description = "List of countries in the continent", body = CountryListPayload),
        (status = 400, description = "Invalid continent name — see description for valid values")
    )
)]
pub(crate) async fn countries_by_continent(
    pool: web::Data<Pool>,
    query: web::Query<ContinentQuery>,
) -> ActixResult<HttpResponse> {
    query.validate().map_err(|e| {
        AppError::Validation(format!("Validation failed: {e}"))
    })?;

    let continent = validate_continent(&query.continent)?;
    let client = pool.get().await.map_err(AppError::from)?;
    let countries = CountryRepository::get_by_continent(&client, &continent).await?;

    Ok(ApiResponse::ok(CountryListPayload {
        continent: query.continent.clone(),
        count: countries.len(),
        countries,
    }))
}
