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

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    20
}

/// Paginated places query within an exposure radius.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({"lat": 6.9271, "lon": 79.8612, "radius": 10.0, "page": 1, "per_page": 20}))]
pub struct ExposurePlacesQuery {
    #[validate(custom(function = "crate::validation::validate_lat"))]
    #[schema(example = 6.9271, minimum = -90, maximum = 90)]
    pub lat: f64,

    #[validate(custom(function = "crate::validation::validate_lon"))]
    #[schema(example = 79.8612, minimum = -180, maximum = 180)]
    pub lon: f64,

    #[serde(default = "default_radius")]
    #[validate(custom(function = "crate::validation::validate_radius_field"))]
    #[schema(example = 10.0, minimum = 0, maximum = 500, default = 1.0)]
    pub radius: f64,

    #[serde(default = "default_page")]
    #[validate(custom(function = "crate::validation::validate_page"))]
    #[schema(example = 1, minimum = 1, default = 1)]
    pub page: i64,

    #[serde(default = "default_per_page")]
    #[validate(custom(function = "crate::validation::validate_per_page"))]
    #[schema(example = 20, minimum = 1, maximum = 100, default = 20)]
    pub per_page: i64,
}

fn default_city_limit() -> i64 {
    10
}

fn default_min_population() -> i64 {
    0
}

/// Fuzzy city search query, used by /cities/search.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({"q": "colom", "country": "LK", "limit": 10}))]
pub struct CitySearchQuery {
    /// Search term (partial name, typos tolerated). Minimum 2 characters.
    #[validate(custom(function = "crate::validation::validate_city_query"))]
    #[schema(example = "colom", min_length = 2, max_length = 80)]
    pub q: String,

    /// Optional ISO 3166-1 alpha-2 country code to scope the search (e.g. `LK`, `us`).
    #[serde(default)]
    #[validate(custom(function = "crate::validation::validate_optional_iso2"))]
    #[schema(example = "LK", min_length = 2, max_length = 2)]
    pub country: Option<String>,

    /// Maximum number of results to return (default: 10, max: 50).
    #[serde(default = "default_city_limit")]
    #[validate(custom(function = "crate::validation::validate_city_limit"))]
    #[schema(example = 10, minimum = 1, maximum = 50, default = 10)]
    pub limit: i64,

    /// Only return places with population greater than or equal to this value.
    /// Use this to filter out hamlets / farms. Default: 0 (no filter).
    #[serde(default = "default_min_population")]
    #[validate(custom(function = "crate::validation::validate_min_population"))]
    #[schema(example = 1000, minimum = 0, default = 0)]
    pub min_population: i64,
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
