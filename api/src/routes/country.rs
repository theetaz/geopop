use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;
use validator::Validate;

use crate::errors::AppError;
use crate::models::{ContinentQuery, CountryListPayload, PointQuery};
use crate::repositories::CountryRepository;
use crate::response::ApiResponse;
use crate::validation::validate_continent;

#[utoipa::path(
    get,
    path = "/country",
    tag = "Country",
    params(("lat" = f64, Query), ("lon" = f64, Query)),
    responses(
        (status = 200, description = "Country at coordinate"),
        (status = 400, description = "Invalid coordinates"),
        (status = 404, description = "No country found")
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

#[utoipa::path(
    get,
    path = "/country/{iso3}",
    tag = "Country",
    params(("iso3" = String, Path)),
    responses(
        (status = 200, description = "Country details"),
        (status = 400, description = "Invalid ISO code"),
        (status = 404, description = "Country not found")
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

#[utoipa::path(
    get,
    path = "/countries",
    tag = "Country",
    params(("continent" = String, Query)),
    responses(
        (status = 200, description = "Countries in continent"),
        (status = 400, description = "Invalid continent")
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
