/// WorldPop 1km population grid constants (30 arc-second resolution).
pub const NCOLS: i64 = 43200; // 360° × 120
pub const NROWS: i64 = 21600; // 180° × 120

/// Compute the integer cell_id from latitude and longitude.
///
/// Maps any coordinate to a unique grid cell using:
///   row = floor((90 - lat) × 120)
///   col = floor((lon + 180) × 120)
///   cell_id = row × 43200 + col
///
/// Returns `None` if coordinates are out of bounds.
#[inline]
pub fn cell_id(lat: f64, lon: f64) -> Option<i32> {
    if !lat.is_finite() || !lon.is_finite() {
        return None;
    }

    let row = ((90.0 - lat) * 120.0).floor() as i64;
    let col = ((lon + 180.0) * 120.0).floor() as i64;

    if row < 0 || row >= NROWS || col < 0 || col >= NCOLS {
        return None;
    }

    Some((row * NCOLS + col) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin() {
        assert_eq!(cell_id(89.999, -179.999), Some(0));
    }

    #[test]
    fn london() {
        let id = cell_id(51.5074, -0.1278).unwrap();
        assert_eq!(id, 4619 * 43200 + 21584);
    }

    #[test]
    fn out_of_bounds() {
        assert_eq!(cell_id(91.0, 0.0), None);
        assert_eq!(cell_id(-91.0, 0.0), None);
        assert_eq!(cell_id(0.0, 181.0), None);
        assert_eq!(cell_id(0.0, -181.0), None);
    }

    #[test]
    fn nan_and_infinity() {
        assert_eq!(cell_id(f64::NAN, 0.0), None);
        assert_eq!(cell_id(0.0, f64::INFINITY), None);
        assert_eq!(cell_id(f64::NEG_INFINITY, 0.0), None);
    }
}
