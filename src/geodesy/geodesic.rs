//! Distance, bearing, and geodesic problems.
//!
//! Ellipsoidal (Karney, on WGS-84) [`geodesic_distance`], [`initial_bearing`] /
//! [`final_bearing`], and the position producers ([`destination`], [`midpoint`],
//! [`intermediate`]) delegate to
//! [`geographiclib-rs`](https://docs.rs/geographiclib-rs). The spherical
//! [`haversine_distance`], cross/along-track ([`cross_track_distance`] /
//! [`along_track_distance`]), and great-circle [`intersection`] routines, and
//! the rhumb/loxodrome ([`rhumb_distance`] / [`rhumb_bearing`] /
//! [`rhumb_destination`]) routines, are implemented directly in sibling
//! modules on the IUGG mean sphere and re-exported here, so every
//! point-to-point operation stays reachable at this path.
//!
//! The model is explicit in the function name, so the accuracy is obvious
//! without reading docs. Measurement functions (distances, bearings) take any
//! [`LatLon`], so they work on `Coordinate` and the per-datum newtypes alike;
//! every multi-point operation rejects mixed CRS values. Producer functions
//! take `&Coordinate`, require WGS-84 inputs, and return WGS-84 coordinates, so
//! an obfuscated GCJ-02 / BD-09 position cannot silently enter true-datum math.

use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

pub use super::rhumb::{rhumb_bearing, rhumb_destination, rhumb_distance};
pub use super::spherical::{
    along_track_distance, cross_track_distance, haversine_distance, intersection,
};
use crate::angle::normalize_degrees;
use crate::coord::{Coordinate, Crs, LatLon};
use crate::error::{Error, Result};
use crate::units::Length;

/// Ellipsoidal (Karney geodesic) distance between two points, measured on the
/// **WGS-84** ellipsoid whatever their shared [`Crs`].
///
/// Preferred over Vincenty, which fails to converge for near-antipodal points.
/// Karney's algorithm is accurate to round-off (nanometers) on WGS-84, so for
/// two WGS-84 points the result is exact for practical purposes. For other
/// systems it is not:
///
/// - **Classic datums** (NAD27, Tokyo, Pulkovo-1942) sit on other ellipsoids,
///   and measuring them on WGS-84 is off by up to about 1.3e-4 of the distance
///   (≈ 0.13 m per km, for Tokyo's Bessel 1841).
/// - **GCJ-02 / BD-09** are offset from true positions by an amount that
///   varies across the map, so the result carries the difference of the two
///   points' offsets. Convert both to WGS-84 first for a true distance.
pub fn geodesic_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    validate_pair(a, b)?;
    let s12: f64 = Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Ok(Length::from_meters(s12))
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees `[0, 360)`.
pub fn initial_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    validate_pair(a, b)?;
    // geographiclib-rs returns the 3-tuple as (azi1, azi2, a12).
    let (azi1, _, _): (f64, f64, f64) =
        Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Ok(normalize_degrees(azi1))
}

/// Final bearing (azimuth on arrival) of the geodesic from `a` to `b`, in
/// degrees `[0, 360)`. Differs from [`initial_bearing`] on the ellipsoid.
pub fn final_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    validate_pair(a, b)?;
    // The 3-tuple is (azi1, azi2, a12); the final azimuth is the second element.
    let (_, azi2, _): (f64, f64, f64) =
        Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Ok(normalize_degrees(azi2))
}

/// Direct geodesic problem: the point reached from `start` by traveling
/// `distance` along `bearing_deg` (exact, Karney). Carries `start.crs()`.
pub fn destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Result<Coordinate> {
    validate_wgs84(start)?;
    validate_bearing_and_distance(bearing_deg, distance)?;
    let (lat2, lon2): (f64, f64) =
        Geodesic::wgs84().direct(start.lat(), start.lon(), bearing_deg, distance.meters());
    Coordinate::new(lat2, lon2, start.crs())
}

/// The geodesic midpoint between `a` and `b`. Carries `a.crs()`.
pub fn midpoint(a: &Coordinate, b: &Coordinate) -> Result<Coordinate> {
    intermediate(a, b, 0.5)
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the geodesic
/// from `a` to `b`. Carries `a.crs()`.
pub fn intermediate(a: &Coordinate, b: &Coordinate, fraction: f64) -> Result<Coordinate> {
    validate_pair(a, b)?;
    validate_wgs84(a)?;
    if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
        return Err(Error::InvalidValue {
            field: "fraction",
            detail: "must be finite and in [0, 1]".into(),
        });
    }
    let g = Geodesic::wgs84();
    // The 4-tuple is (s12, azi1, azi2, a12); travel `fraction · s12` along azi1.
    let (s12, azi1, _, _): (f64, f64, f64, f64) = g.inverse(a.lat(), a.lon(), b.lat(), b.lon());
    let (lat, lon): (f64, f64) = g.direct(a.lat(), a.lon(), azi1, s12 * fraction);
    Coordinate::new(lat, lon, a.crs())
}

fn validate_position(position: &impl LatLon) -> Result<()> {
    Coordinate::new(position.lat(), position.lon(), position.crs()).map(|_| ())
}

pub(super) fn validate_pair(a: &impl LatLon, b: &impl LatLon) -> Result<()> {
    validate_position(a)?;
    validate_position(b)?;
    if a.crs() != b.crs() {
        return Err(Error::CrsMismatch {
            expected: a.crs(),
            found: b.crs(),
        });
    }
    Ok(())
}

pub(super) fn validate_wgs84(position: &Coordinate) -> Result<()> {
    if position.crs() != Crs::Wgs84 {
        return Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: position.crs(),
        });
    }
    Ok(())
}

pub(super) fn validate_bearing_and_distance(bearing_deg: f64, distance: Length) -> Result<()> {
    if !bearing_deg.is_finite() {
        return Err(Error::InvalidValue {
            field: "bearing",
            detail: "must be finite".into(),
        });
    }
    if !distance.meters().is_finite() || distance.meters() < 0.0 {
        return Err(Error::InvalidValue {
            field: "distance",
            detail: "must be finite and nonnegative".into(),
        });
    }
    Ok(())
}

// Polygon and line ops — ellipsoidal area/perimeter, point-in-polygon, centroid,
// convex hull, buffers, bounding boxes, line densification, and simplification —
// are out of scope for this crate: use the `geo` crate directly for them. This
// module and its `spherical` / `rhumb` siblings own only point-to-point ops.

#[cfg(test)]
mod tests;
