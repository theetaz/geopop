use crate::errors::AppError;
use validator::ValidationError;

pub(crate) const MAX_BATCH_SIZE: usize = 1000;
pub(crate) const MAX_RADIUS_KM: f64 = 500.0;
pub(crate) const MAX_POPULATION_RADIUS_KM: f64 = 10.0;
pub(crate) const VALID_CONTINENTS: &[&str] = &[
    "asia", "europe", "africa", "oceania", "americas",
    "north-america", "south-america",
];

pub fn validate_lat(lat: f64) -> Result<(), ValidationError> {
    if !lat.is_finite() || lat < -90.0 || lat > 90.0 {
        return Err(ValidationError::new("latitude"));
    }
    Ok(())
}

pub fn validate_lon(lon: f64) -> Result<(), ValidationError> {
    if !lon.is_finite() || lon < -180.0 || lon >= 180.0 {
        return Err(ValidationError::new("longitude"));
    }
    Ok(())
}

pub fn validate_population_radius(radius: f64) -> Result<(), ValidationError> {
    if !radius.is_finite() || radius <= 0.0 || radius > MAX_POPULATION_RADIUS_KM {
        return Err(ValidationError::new("radius"));
    }
    Ok(())
}

pub fn validate_radius_field(radius: f64) -> Result<(), ValidationError> {
    if !radius.is_finite() || radius <= 0.0 || radius > MAX_RADIUS_KM {
        return Err(ValidationError::new("radius"));
    }
    Ok(())
}

pub fn validate_continent_field(continent: &str) -> Result<(), ValidationError> {
    let normalized = continent.trim().to_lowercase();
    if normalized.is_empty() || !VALID_CONTINENTS.contains(&normalized.as_str()) {
        return Err(ValidationError::new("continent"));
    }
    Ok(())
}

pub(crate) fn validate_continent(input: &str) -> Result<String, AppError> {
    let normalized = input.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!(
            "Missing required parameter: continent. Valid values: {}",
            VALID_CONTINENTS.join(", ")
        )));
    }
    if !VALID_CONTINENTS.contains(&normalized.as_str()) {
        return Err(AppError::Validation(format!(
            "Invalid continent '{input}'. Valid values: {}",
            VALID_CONTINENTS.join(", ")
        )));
    }
    Ok(normalized)
}

pub(crate) fn validate_iso3(iso3: &str) -> Result<String, AppError> {
    let normalized = iso3.to_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::Validation(
            "ISO-3166 alpha-3 code must be exactly 3 letters (e.g. USA, IND, GBR)".into(),
        ));
    }
    Ok(normalized)
}

pub(crate) fn validate_batch_size(size: usize) -> Result<(), AppError> {
    if size == 0 {
        return Err(AppError::Validation(
            "Request must contain at least one point".into(),
        ));
    }
    if size > MAX_BATCH_SIZE {
        return Err(AppError::Validation(format!(
            "Maximum {MAX_BATCH_SIZE} points per batch request"
        )));
    }
    Ok(())
}
