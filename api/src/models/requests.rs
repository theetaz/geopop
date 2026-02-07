use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct PointQuery {
    #[validate(custom(function = "crate::validation::validate_lat"))]
    pub lat: f64,
    #[validate(custom(function = "crate::validation::validate_lon"))]
    pub lon: f64,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct BatchQuery {
    #[validate(length(min = 1, max = 1000, message = "Must contain between 1 and 1000 points"))]
    pub points: Vec<PointQuery>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ExposureQuery {
    #[validate(custom(function = "crate::validation::validate_lat"))]
    pub lat: f64,
    #[validate(custom(function = "crate::validation::validate_lon"))]
    pub lon: f64,
    #[serde(default = "default_radius")]
    #[validate(custom(function = "crate::validation::validate_radius_field"))]
    pub radius: f64,
}

fn default_radius() -> f64 {
    1.0
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ContinentQuery {
    #[validate(custom(function = "crate::validation::validate_continent_field"))]
    pub continent: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Iso3Path {
    #[serde(rename = "iso3")]
    pub value: String,
}
