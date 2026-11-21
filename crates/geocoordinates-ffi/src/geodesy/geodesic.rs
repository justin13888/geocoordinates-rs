//! Geodesics and rhumb lines: distances, bearings, and the points they
//! produce, each on the ellipsoid of the inputs' own CRS.

use geocoordinates as gc;

use crate::{Coordinate, GeoError};

/// Ellipsoidal (Karney) geodesic distance between two coordinates, in
/// **meters**, measured on the ellipsoid of their shared CRS (NAD27 on Clarke
/// 1866, Tokyo on Bessel 1841, …), accurate to round-off.
///
/// # Errors
/// `GeoError::CrsMismatch` when the coordinates are in different reference
/// systems, or are GCJ-02 / BD-09, which are obfuscated renderings with no
/// ellipsoid (convert them to WGS-84 first); `GeoError::OutOfRange` /
/// `GeoError::InvalidValue` for an invalid coordinate record.
#[uniffi::export]
pub fn geodesic_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::geodesic_distance(a, b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees `[0, 360)`,
/// on the ellipsoid of their shared CRS — the FFI form of the core
/// `geodesy::initial_bearing`, suffixed `_deg` as every angle this surface
/// returns is. From a pole the bearing follows the meridian of `a`'s own
/// longitude.
///
/// # Errors
/// As [`geodesic_distance_m`], and `GeoError::InvalidValue` (field `points`)
/// when `a` and `b` are the same place, which has no bearing.
#[uniffi::export]
pub fn initial_bearing_deg(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::initial_bearing(a, b).map_err(Into::into)
}

/// Final bearing (azimuth on arrival) from `a` to `b`, in degrees `[0, 360)`,
/// on the ellipsoid of their shared CRS — the FFI form of the core
/// `geodesy::final_bearing`.
///
/// # Errors
/// As [`initial_bearing_deg`].
#[uniffi::export]
pub fn final_bearing_deg(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::final_bearing(a, b).map_err(Into::into)
}

/// The point reached from `start` along `bearing_deg` for `distance_m` meters
/// (exact, Karney), on the ellipsoid of `start`'s CRS; the result keeps that
/// CRS and has no height.
///
/// # Errors
/// `GeoError::CrsMismatch` for a GCJ-02 / BD-09 start;
/// `GeoError::InvalidValue` for a non-finite bearing (field `bearing_deg`) or a
/// non-finite or negative distance (field `distance`); `GeoError::OutOfRange`
/// / `GeoError::InvalidValue` for an invalid coordinate record.
#[uniffi::export]
pub fn destination(
    start: Coordinate,
    bearing_deg: f64,
    distance_m: f64,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::destination(
        start.try_into()?,
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .map(Into::into)
    .map_err(Into::into)
}

/// The geodesic midpoint between `a` and `b`, on the ellipsoid of their
/// shared CRS (kept, with no height).
///
/// # Errors
/// As [`geodesic_distance_m`].
#[uniffi::export]
pub fn midpoint(a: Coordinate, b: Coordinate) -> Result<Coordinate, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::midpoint(a, b)
        .map(Into::into)
        .map_err(Into::into)
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the
/// geodesic, on the ellipsoid of their shared CRS (kept, with no height).
///
/// # Errors
/// As [`geodesic_distance_m`], and `GeoError::InvalidValue` (field
/// `fraction`) when `fraction` is not finite or not in `[0, 1]`.
#[uniffi::export]
pub fn intermediate(a: Coordinate, b: Coordinate, fraction: f64) -> Result<Coordinate, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::intermediate(a, b, fraction)
        .map(Into::into)
        .map_err(Into::into)
}

/// Rhumb-line (loxodrome) distance between two coordinates, in **meters**,
/// on the ellipsoid of their shared CRS (the meridian arc when either is a
/// pole).
///
/// # Errors
/// As [`geodesic_distance_m`].
#[uniffi::export]
pub fn rhumb_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::rhumb_distance(a, b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Rhumb-line (constant) bearing from `a` to `b`, in degrees `[0, 360)`, on
/// the ellipsoid of their shared CRS — the FFI form of the core
/// `geodesy::rhumb_bearing`. To a pole it is 0° (north) or 180° (south). From
/// a pole it is taken against the meridian of `a`'s longitude and names the
/// meridian reaching `b`: `180° − (b.lon − a.lon)` from the north pole,
/// `b.lon − a.lon` from the south pole, so [`rhumb_destination`] inverts it.
///
/// # Errors
/// As [`rhumb_distance_m`], and `GeoError::InvalidValue` (field `points`) for
/// coincident points (including one pole named at two longitudes), which have
/// no bearing.
#[uniffi::export]
pub fn rhumb_bearing_deg(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::rhumb_bearing(a, b).map_err(Into::into)
}

/// The point reached from `start` along a constant `bearing_deg` rhumb line for
/// `distance_m` meters, on the ellipsoid of its CRS (kept, with no height). A
/// meridional line (0° or 180°) passes over a pole onto the far meridian; any
/// other bearing errors (`InvalidValue`) when it would reach a pole. From a
/// pole every bearing is accepted, read against the meridian of
/// `start.lon`: from the north pole bearing `β` runs down meridian
/// `start.lon + 180° − β`, from the south pole up `start.lon + β`.
///
/// # Errors
/// As [`destination`], and `GeoError::InvalidValue` (field `distance`) when a
/// line that does not start at a pole, on a bearing other than 0° or 180°,
/// would be carried to or past a pole.
#[uniffi::export]
pub fn rhumb_destination(
    start: Coordinate,
    bearing_deg: f64,
    distance_m: f64,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::rhumb_destination(
        start.try_into()?,
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .map(Into::into)
    .map_err(Into::into)
}
