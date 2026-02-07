use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Single coordinate query for population or geocoding lookups.
#[derive(Debug, Deserialize, Serialize, Validate, ToSchema)]
#[schema(example = json!({"lat": 6.9271, "lon": 79.8612}))]
pub struct PointQuery {
    /// Latitude in decimal degrees (-90 to 90)
    #[validate(custom(function = "crate::validation::validate_lat"))]
    #[schema(example = 6.9271, minimum = -90, maximum = 90)]
    pub lat: f64,

    /// Longitude in decimal degrees (-180 to 180)
    #[validate(custom(function = "crate::validation::validate_lon"))]
    #[schema(example = 79.8612, minimum = -180, maximum = 180)]
    pub lon: f64,
}

/// Population query with optional radius for grid cell retrieval.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({"lat": 6.9271, "lon": 79.8612, "radius": 5.0}))]
pub struct PopulationQuery {
    /// Latitude in decimal degrees (-90 to 90)
    #[validate(custom(function = "crate::validation::validate_lat"))]
    #[schema(example = 6.9271, minimum = -90, maximum = 90)]
    pub lat: f64,

    /// Longitude in decimal degrees (-180 to 180)
    #[validate(custom(function = "crate::validation::validate_lon"))]
    #[schema(example = 79.8612, minimum = -180, maximum = 180)]
    pub lon: f64,

    /// Optional search radius in kilometres. When omitted, returns a single grid cell. When provided, returns all non-empty grid cells within the radius (max: 10 km).
    #[validate(custom(function = "crate::validation::validate_population_radius"))]
    #[schema(example = 5.0, minimum = 0, maximum = 10)]
    pub radius: Option<f64>,
}

/// Batch request containing multiple coordinate points (max 1000).
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({"points": [{"lat": 6.9271, "lon": 79.8612}, {"lat": 7.2906, "lon": 80.6337}]}))]
pub struct BatchQuery {
    /// Array of coordinate points to query (1–1000 points)
    #[validate(length(min = 1, max = 1000, message = "Must contain between 1 and 1000 points"))]
    pub points: Vec<PointQuery>,
}

/// Population exposure query with configurable search radius.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({"lat": 6.9271, "lon": 79.8612, "radius": 10.0}))]
pub struct ExposureQuery {
    /// Latitude in decimal degrees (-90 to 90)
    #[validate(custom(function = "crate::validation::validate_lat"))]
    #[schema(example = 6.9271, minimum = -90, maximum = 90)]
    pub lat: f64,

    /// Longitude in decimal degrees (-180 to 180)
    #[validate(custom(function = "crate::validation::validate_lon"))]
    #[schema(example = 79.8612, minimum = -180, maximum = 180)]
    pub lon: f64,

    /// Search radius in kilometres (default: 1, max: 500)
    #[serde(default = "default_radius")]
    #[validate(custom(function = "crate::validation::validate_radius_field"))]
    #[schema(example = 10.0, minimum = 0, maximum = 500, default = 1.0)]
    pub radius: f64,
}

fn default_radius() -> f64 {
    1.0
}

/// Query filter for listing countries by continent.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({"continent": "asia"}))]
pub struct ContinentQuery {
    /// Continent name (asia, europe, africa, oceania, americas, north-america, south-america)
    #[validate(custom(function = "crate::validation::validate_continent_field"))]
    #[schema(example = "asia")]
    pub continent: String,
}
