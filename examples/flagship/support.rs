//! Shared assertion helpers and reference values for the flagship sections.

use std::error::Error as StdError;

use geocoordinates::china::BaiduMercator;
use geocoordinates::geodesy::{Aer, Ellipsoid, Enu, Ned, geodesic_distance, haversine_distance};
use geocoordinates::{Coordinate, LatLon};

/// Result of one flagship section: its display name on success.
pub(crate) type DemoResult = Result<&'static str, Box<dyn StdError>>;

/// New York City (WGS-84). Constructors validate, so this is a function rather
/// than a `const`: a `Coordinate` only exists once its range is proven.
pub(crate) fn new_york() -> Coordinate {
    Coordinate::wgs84(40.7128, -74.006).expect("valid literal")
}

pub(crate) fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

/// Asserts two positions in one CRS lie within `max_m` meters: on the CRS's
/// ellipsoid where it has one, and on the sphere between two GCJ-02 or BD-09
/// renderings, which have no ellipsoid (the geodesic rejects them).
pub(crate) fn assert_within_meters(actual: &impl LatLon, expected: &impl LatLon, max_m: f64) {
    let distance = if Ellipsoid::for_crs(actual.crs()).is_some() {
        geodesic_distance(actual, expected)
    } else {
        haversine_distance(actual, expected)
    }
    .expect("flagship compares valid positions in one CRS")
    .meters();
    assert!(
        distance <= max_m + 1e-6,
        "points are {distance} m apart, beyond {max_m} m"
    );
}

pub(crate) fn assert_mercator_close(
    actual: BaiduMercator,
    expected: BaiduMercator,
    tolerance_m: f64,
) {
    assert_close(actual.x(), expected.x(), tolerance_m);
    assert_close(actual.y(), expected.y(), tolerance_m);
}

pub(crate) fn assert_enu_close(actual: Enu, expected: Enu) {
    assert_close(actual.east, expected.east, 1e-9);
    assert_close(actual.north, expected.north, 1e-9);
    assert_close(actual.up, expected.up, 1e-9);
}

pub(crate) fn assert_ned_close(actual: Ned, expected: Ned) {
    assert_close(actual.north, expected.north, 1e-9);
    assert_close(actual.east, expected.east, 1e-9);
    assert_close(actual.down, expected.down, 1e-9);
}

pub(crate) fn assert_aer_close(actual: Aer, expected: Aer) {
    assert_close(actual.azimuth_deg, expected.azimuth_deg, 1e-9);
    assert_close(actual.elevation_deg, expected.elevation_deg, 1e-9);
    assert_close(actual.range.meters(), expected.range.meters(), 1e-9);
}
