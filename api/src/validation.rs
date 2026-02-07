use crate::errors::AppError;
use validator::ValidationError;

pub const MAX_BATCH_SIZE: usize = 1000;
pub const MAX_RADIUS_KM: f64 = 500.0;
pub const VALID_CONTINENTS: &[&str] = &[
    "asia", "europe", "africa", "oceania", "americas",
    "north-america", "south-america",
];

pub fn validate_lat(lat: &f64) -> Result<(), ValidationError> {
    if !lat.is_finite() || *lat < -90.0 || *lat > 90.0 {
        return Err(ValidationError::new("latitude"));
    }
    Ok(())
}

pub fn validate_lon(lon: &f64) -> Result<(), ValidationError> {
    if !lon.is_finite() || *lon < -180.0 || *lon >= 180.0 {
        return Err(ValidationError::new("longitude"));
    }
    Ok(())
}

pub fn validate_radius_field(radius: &f64) -> Result<(), ValidationError> {
    if !radius.is_finite() || *radius <= 0.0 || *radius > MAX_RADIUS_KM {
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

pub fn validate_coordinates(lat: f64, lon: f64) -> Result<(), AppError> {
    if !lat.is_finite() || !lon.is_finite() {
        return Err(AppError::Validation(
            "Coordinates must be finite numbers".to_string(),
        ));
    }
    if lat < -90.0 || lat > 90.0 {
        return Err(AppError::Validation(
            "Latitude must be between -90 and 90".to_string(),
        ));
    }
    if lon < -180.0 || lon >= 180.0 {
        return Err(AppError::Validation(
            "Longitude must be between -180 and 180".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_radius(radius: f64) -> Result<(), AppError> {
    if !radius.is_finite() || radius <= 0.0 || radius > MAX_RADIUS_KM {
        return Err(AppError::Validation(format!(
            "Radius must be between 0 and {} km",
            MAX_RADIUS_KM
        )));
    }
    Ok(())
}

pub fn validate_continent(input: &str) -> Result<String, AppError> {
    let normalized = input.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!(
            "Missing required parameter: continent. Valid values: {}",
            VALID_CONTINENTS.join(", ")
        )));
    }
    if !VALID_CONTINENTS.contains(&normalized.as_str()) {
        return Err(AppError::Validation(format!(
            "Invalid continent '{}'. Valid values: {}",
            input,
            VALID_CONTINENTS.join(", ")
        )));
    }
    Ok(normalized)
}

pub fn validate_iso3(iso3: &str) -> Result<String, AppError> {
    let normalized = iso3.to_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::Validation(
            "ISO-3166 alpha-3 code must be exactly 3 letters (e.g. USA, IND, GBR)".to_string(),
        ));
    }
    Ok(normalized)
}

pub fn validate_batch_size(size: usize) -> Result<(), AppError> {
    if size == 0 {
        return Err(AppError::Validation(
            "Request must contain at least one point".to_string(),
        ));
    }
    if size > MAX_BATCH_SIZE {
        return Err(AppError::Validation(format!(
            "Maximum {} points per batch request",
            MAX_BATCH_SIZE
        )));
    }
    Ok(())
}
