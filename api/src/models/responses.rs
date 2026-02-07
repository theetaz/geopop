use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct HealthPayload {
    pub status: String,
}

#[derive(Serialize, ToSchema)]
pub struct PointPayload {
    pub lat: f64,
    pub lon: f64,
    pub population: f32,
    pub resolution_km: f32,
}

#[derive(Serialize, ToSchema)]
pub struct BatchPayload {
    pub results: Vec<PointPayload>,
}

#[derive(Serialize, ToSchema)]
pub struct ReversePayload {
    pub place_id: i32,
    pub lat: String,
    pub lon: String,
    pub name: String,
    pub display_name: String,
    pub address: HashMap<String, String>,
}

#[derive(Serialize, ToSchema)]
pub struct ExposedPlace {
    pub place_id: i32,
    pub lat: String,
    pub lon: String,
    pub name: String,
    pub display_name: String,
    pub address: HashMap<String, String>,
    pub distance_km: f64,
}

#[derive(Serialize, ToSchema)]
pub struct CoordinateInfo {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Serialize, ToSchema)]
pub struct ExposurePayload {
    pub coordinate: CoordinateInfo,
    pub radius_km: f64,
    pub total_population: f64,
    pub area_km2: f64,
    pub density_per_km2: f64,
    pub cell_population: f32,
    pub cell_area_km2: f64,
    pub cell_density_per_km2: f64,
    pub places: Vec<ExposedPlace>,
}

#[derive(Serialize, ToSchema)]
pub struct CountryPayload {
    pub iso_a2: Option<String>,
    pub iso_a3: Option<String>,
    pub name: String,
    pub formal_name: Option<String>,
    pub continent: String,
    pub region: Option<String>,
    pub subregion: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct CountryDetailPayload {
    pub iso_a2: Option<String>,
    pub iso_a3: Option<String>,
    pub name: String,
    pub formal_name: Option<String>,
    pub continent: String,
    pub region: Option<String>,
    pub subregion: Option<String>,
    pub pop_est: Option<i64>,
    pub bbox: [f64; 4],
}

#[derive(Serialize, ToSchema)]
pub struct CountryListPayload {
    pub continent: String,
    pub count: usize,
    pub countries: Vec<CountryPayload>,
}
