use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

/// Health check status.
#[derive(Serialize, ToSchema)]
#[schema(example = json!({"status": "ok"}))]
pub struct HealthPayload {
    /// Service status indicator
    #[schema(example = "ok")]
    pub status: String,
}

/// Population data for a single coordinate.
#[derive(Serialize, ToSchema)]
#[schema(example = json!({"lat": 6.9271, "lon": 79.8612, "population": 28534.0, "resolution_km": 1.0}))]
pub struct PointPayload {
    /// Queried latitude
    #[schema(example = 6.9271)]
    pub lat: f64,
    /// Queried longitude
    #[schema(example = 79.8612)]
    pub lon: f64,
    /// Estimated population within the grid cell
    #[schema(example = 28534.0)]
    pub population: f32,
    /// Grid cell resolution in kilometres (always 1.0 for WorldPop data)
    #[schema(example = 1.0)]
    pub resolution_km: f32,
}

/// Batch population results for multiple coordinates.
#[derive(Serialize, ToSchema)]
pub struct BatchPayload {
    /// Array of population results for each queried point
    pub results: Vec<PointPayload>,
}

/// Bounding box of a single population grid cell.
#[derive(Serialize, ToSchema)]
#[schema(example = json!({"min_lat": 20.4583, "max_lat": 20.4667, "min_lon": 93.9500, "max_lon": 93.9583}))]
pub struct CellBounds {
    /// Southern edge latitude
    #[schema(example = 20.4583)]
    pub min_lat: f64,
    /// Northern edge latitude
    #[schema(example = 20.4667)]
    pub max_lat: f64,
    /// Western edge longitude
    #[schema(example = 93.9500)]
    pub min_lon: f64,
    /// Eastern edge longitude
    #[schema(example = 93.9583)]
    pub max_lon: f64,
}

/// A single 1 km² population grid cell with its bounds for map rendering.
#[derive(Serialize, ToSchema)]
pub struct GridCell {
    /// Centre latitude of the grid cell
    #[schema(example = 20.4625)]
    pub lat: f64,
    /// Centre longitude of the grid cell
    #[schema(example = 93.9542)]
    pub lon: f64,
    /// Estimated population within this cell
    #[schema(example = 5.16)]
    pub population: f32,
    /// Geographic bounds of the cell (for rendering as a rectangle on a map)
    pub bounds: CellBounds,
}

/// Population grid data within a radius, suitable for map visualisation.
#[derive(Serialize, ToSchema)]
pub struct PopulationGridPayload {
    /// Centre coordinate of the query
    pub coordinate: CoordinateInfo,
    /// Search radius in kilometres
    #[schema(example = 5.0)]
    pub radius_km: f64,
    /// Total population across all cells within the radius
    #[schema(example = 1653.2)]
    pub total_population: f64,
    /// Number of non-empty grid cells returned
    #[schema(example = 42)]
    pub cell_count: usize,
    /// Individual grid cells with population > 0
    pub cells: Vec<GridCell>,
}

/// Reverse geocoding result — nearest named place to the queried coordinate.
#[derive(Serialize, ToSchema)]
#[schema(example = json!({
    "place_id": 1234,
    "lat": "6.9271",
    "lon": "79.8612",
    "name": "Colombo",
    "display_name": "Colombo, Western Province, Sri Lanka",
    "address": {"city": "Colombo", "state": "Western Province", "country": "Sri Lanka"}
}))]
pub struct ReversePayload {
    /// GeoNames place identifier
    #[schema(example = 1234)]
    pub place_id: i32,
    /// Latitude of the matched place
    #[schema(example = "6.9271")]
    pub lat: String,
    /// Longitude of the matched place
    #[schema(example = "79.8612")]
    pub lon: String,
    /// Place name
    #[schema(example = "Colombo")]
    pub name: String,
    /// Full display name including administrative hierarchy
    #[schema(example = "Colombo, Western Province, Sri Lanka")]
    pub display_name: String,
    /// Structured address components (city, state, country, etc.)
    pub address: HashMap<String, String>,
}

/// A named place within the exposure search radius.
#[derive(Serialize, ToSchema)]
pub struct ExposedPlace {
    /// GeoNames place identifier
    #[schema(example = 1234)]
    pub place_id: i32,
    /// Latitude of the place
    #[schema(example = "6.9271")]
    pub lat: String,
    /// Longitude of the place
    #[schema(example = "79.8612")]
    pub lon: String,
    /// Place name
    #[schema(example = "Colombo")]
    pub name: String,
    /// Full display name
    #[schema(example = "Colombo, Western Province, Sri Lanka")]
    pub display_name: String,
    /// Structured address components (city, district, state, country, country_code)
    pub address: HashMap<String, String>,
    /// Distance from the epicentre in kilometres
    #[schema(example = 3.2)]
    pub distance_km: f64,
    /// Compass direction from the epicentre (N, NE, E, SE, S, SW, W, NW)
    #[schema(example = "SW")]
    pub direction: String,
    /// Bearing from the epicentre in degrees (0 = North, 90 = East, 180 = South, 270 = West)
    #[schema(example = 225.3)]
    pub bearing_deg: f64,
}

/// Coordinate pair used in exposure results.
#[derive(Serialize, ToSchema)]
#[schema(example = json!({"lat": 6.9271, "lon": 79.8612}))]
pub struct CoordinateInfo {
    /// Latitude
    #[schema(example = 6.9271)]
    pub lat: f64,
    /// Longitude
    #[schema(example = 79.8612)]
    pub lon: f64,
}

/// Comprehensive population exposure analysis for a circular area.
#[derive(Serialize, ToSchema)]
pub struct ExposurePayload {
    /// Centre coordinate of the analysis area
    pub coordinate: CoordinateInfo,
    /// Search radius in kilometres
    #[schema(example = 10.0)]
    pub radius_km: f64,
    /// Total estimated population within the radius
    #[schema(example = 456789.0)]
    pub total_population: f64,
    /// Area of the search circle in km²
    #[schema(example = 314.16)]
    pub area_km2: f64,
    /// Average population density (people/km²) within the radius
    #[schema(example = 1454.1)]
    pub density_per_km2: f64,
    /// Population in the 1km grid cell at the centre coordinate
    #[schema(example = 28534.0)]
    pub cell_population: f32,
    /// Area of the centre grid cell in km²
    #[schema(example = 0.77)]
    pub cell_area_km2: f64,
    /// Population density of the centre grid cell (people/km²)
    #[schema(example = 37057.1)]
    pub cell_density_per_km2: f64,
    /// Named places found within the search radius
    pub places: Vec<ExposedPlace>,
}

/// Country information from Natural Earth boundaries.
#[derive(Serialize, ToSchema)]
#[schema(example = json!({
    "iso_a2": "LK", "iso_a3": "LKA", "name": "Sri Lanka",
    "formal_name": "Democratic Socialist Republic of Sri Lanka",
    "continent": "Asia", "region": "Asia", "subregion": "Southern Asia"
}))]
pub struct CountryPayload {
    /// ISO 3166-1 alpha-2 code
    #[schema(example = "LK")]
    pub iso_a2: Option<String>,
    /// ISO 3166-1 alpha-3 code
    #[schema(example = "LKA")]
    pub iso_a3: Option<String>,
    /// Country common name
    #[schema(example = "Sri Lanka")]
    pub name: String,
    /// Country formal/official name
    #[schema(example = "Democratic Socialist Republic of Sri Lanka")]
    pub formal_name: Option<String>,
    /// Continent
    #[schema(example = "Asia")]
    pub continent: String,
    /// World region
    #[schema(example = "Asia")]
    pub region: Option<String>,
    /// World sub-region
    #[schema(example = "Southern Asia")]
    pub subregion: Option<String>,
}

/// Detailed country information including population estimate and bounding box.
#[derive(Serialize, ToSchema)]
#[schema(example = json!({
    "iso_a2": "LK", "iso_a3": "LKA", "name": "Sri Lanka",
    "formal_name": "Democratic Socialist Republic of Sri Lanka",
    "continent": "Asia", "region": "Asia", "subregion": "Southern Asia",
    "pop_est": 21670000, "bbox": [79.6952, 5.9169, 81.8813, 9.8354]
}))]
pub struct CountryDetailPayload {
    /// ISO 3166-1 alpha-2 code
    #[schema(example = "LK")]
    pub iso_a2: Option<String>,
    /// ISO 3166-1 alpha-3 code
    #[schema(example = "LKA")]
    pub iso_a3: Option<String>,
    /// Country common name
    #[schema(example = "Sri Lanka")]
    pub name: String,
    /// Country formal/official name
    #[schema(example = "Democratic Socialist Republic of Sri Lanka")]
    pub formal_name: Option<String>,
    /// Continent
    #[schema(example = "Asia")]
    pub continent: String,
    /// World region
    #[schema(example = "Asia")]
    pub region: Option<String>,
    /// World sub-region
    #[schema(example = "Southern Asia")]
    pub subregion: Option<String>,
    /// Estimated population
    #[schema(example = 21670000)]
    pub pop_est: Option<i64>,
    /// Bounding box [min_lon, min_lat, max_lon, max_lat]
    #[schema(example = json!([79.6952, 5.9169, 81.8813, 9.8354]))]
    pub bbox: [f64; 4],
}

/// List of countries belonging to a continent.
#[derive(Serialize, ToSchema)]
pub struct CountryListPayload {
    /// Queried continent name
    #[schema(example = "asia")]
    pub continent: String,
    /// Number of countries returned
    #[schema(example = 49)]
    pub count: usize,
    /// Country list
    pub countries: Vec<CountryPayload>,
}
