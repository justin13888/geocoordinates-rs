//! China datums: the typed WGS-84 / GCJ-02 / BD-09 records, their exact
//! forward and approximate inverse conversions, and Baidu Web Mercator.
//!
//! The records are plain data and unvalidated until used: every export that
//! takes one validates it as the core `Wgs84::new` / `Gcj02::new` /
//! `Bd09::new` / `BaiduMercator::new` does, and `*_validate` checks one on
//! its own.

use geocoordinates as gc;

use crate::{Coordinate, GeoError};

/// A WGS-84 position (real GPS / OpenStreetMap), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Wgs84 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A GCJ-02 position (Google China, AutoNavi/高德, Tencent), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Gcj02 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A BD-09 position (Baidu Maps only), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Bd09 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A Baidu Web Mercator position, in projected meters.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BaiduMercator {
    /// Easting (longitude axis), in meters.
    pub x: f64,
    /// Northing (latitude axis), in meters.
    pub y: f64,
}

/// An **approximate** WGS-84 position — the FFI-flattened form of
/// `Approx<Wgs84>`. `max_error_m` is the estimated upper bound on positional
/// error, in meters.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ApproxWgs84 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
    /// Estimated maximum positional error, in meters.
    pub max_error_m: f64,
}

/// An **approximate** GCJ-02 position — the FFI-flattened form of
/// `Approx<Gcj02>`. `max_error_m` is the estimated upper bound on positional
/// error, in meters.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ApproxGcj02 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
    /// Estimated maximum positional error, in meters.
    pub max_error_m: f64,
}

/// Mirror ↔ core translations for the three datum records. A record entering
/// the core crate is validated by the core `new`, so an out-of-range record
/// fails with `GeoError::OutOfRange` before any conversion runs.
macro_rules! datum_mirror {
    ($($t:ident),*) => {$(
        impl From<gc::china::$t> for $t {
            fn from(p: gc::china::$t) -> Self {
                $t {
                    lat: p.lat(),
                    lon: p.lon(),
                }
            }
        }

        impl TryFrom<$t> for gc::china::$t {
            type Error = GeoError;

            fn try_from(p: $t) -> Result<Self, GeoError> {
                gc::china::$t::new(p.lat, p.lon).map_err(GeoError::from)
            }
        }
    )*};
}

datum_mirror!(Wgs84, Gcj02, Bd09);

impl From<gc::china::BaiduMercator> for BaiduMercator {
    fn from(m: gc::china::BaiduMercator) -> Self {
        BaiduMercator {
            x: m.x_m(),
            y: m.y_m(),
        }
    }
}

impl TryFrom<BaiduMercator> for gc::china::BaiduMercator {
    type Error = GeoError;

    fn try_from(m: BaiduMercator) -> Result<Self, GeoError> {
        gc::china::BaiduMercator::new(m.x, m.y).map_err(GeoError::from)
    }
}

impl From<gc::Approx<gc::china::Wgs84>> for ApproxWgs84 {
    fn from(a: gc::Approx<gc::china::Wgs84>) -> Self {
        ApproxWgs84 {
            lat: a.value().lat(),
            lon: a.value().lon(),
            max_error_m: a.max_error().meters(),
        }
    }
}

impl From<gc::Approx<gc::china::Gcj02>> for ApproxGcj02 {
    fn from(a: gc::Approx<gc::china::Gcj02>) -> Self {
        ApproxGcj02 {
            lat: a.value().lat(),
            lon: a.value().lon(),
            max_error_m: a.max_error().meters(),
        }
    }
}

// --- Validation and the canonical `Coordinate` bridges ---

/// Validate a (possibly hand-built) WGS-84 record: latitude in `[-90, 90]`,
/// longitude in `[-180, 180]`, both finite — the FFI form of `Wgs84::new`.
///
/// # Errors
/// `GeoError::OutOfRange` when either component is invalid.
#[uniffi::export]
pub fn wgs84_validate(p: Wgs84) -> Result<(), GeoError> {
    gc::china::Wgs84::try_from(p).map(drop)
}

/// Validate a (possibly hand-built) GCJ-02 record, as [`wgs84_validate`].
///
/// # Errors
/// `GeoError::OutOfRange` when either component is invalid.
#[uniffi::export]
pub fn gcj02_validate(p: Gcj02) -> Result<(), GeoError> {
    gc::china::Gcj02::try_from(p).map(drop)
}

/// Validate a (possibly hand-built) BD-09 record, as [`wgs84_validate`].
///
/// # Errors
/// `GeoError::OutOfRange` when either component is invalid.
#[uniffi::export]
pub fn bd09_validate(p: Bd09) -> Result<(), GeoError> {
    gc::china::Bd09::try_from(p).map(drop)
}

/// Validate a (possibly hand-built) Baidu Mercator record: both components
/// finite — the FFI form of `BaiduMercator::new`. Whether the position is
/// inside the projection is checked by [`baidu_mercator_to_bd09`].
///
/// # Errors
/// `GeoError::InvalidValue` naming `x` or `y` when that component is not
/// finite.
#[uniffi::export]
pub fn baidu_mercator_validate(m: BaiduMercator) -> Result<(), GeoError> {
    gc::china::BaiduMercator::try_from(m).map(drop)
}

/// WGS-84 record → canonical [`Coordinate`] tagged WGS-84, with no height.
///
/// # Errors
/// As [`wgs84_validate`], for an invalid record.
#[uniffi::export]
pub fn wgs84_to_coordinate(p: Wgs84) -> Result<Coordinate, GeoError> {
    Ok(gc::china::Wgs84::try_from(p)?.to_coordinate().into())
}

/// GCJ-02 record → canonical [`Coordinate`] tagged GCJ-02, with no height.
///
/// # Errors
/// As [`gcj02_validate`], for an invalid record.
#[uniffi::export]
pub fn gcj02_to_coordinate(p: Gcj02) -> Result<Coordinate, GeoError> {
    Ok(gc::china::Gcj02::try_from(p)?.to_coordinate().into())
}

/// BD-09 record → canonical [`Coordinate`] tagged BD-09, with no height.
///
/// # Errors
/// As [`bd09_validate`], for an invalid record.
#[uniffi::export]
pub fn bd09_to_coordinate(p: Bd09) -> Result<Coordinate, GeoError> {
    Ok(gc::china::Bd09::try_from(p)?.to_coordinate().into())
}

/// Canonical [`Coordinate`] → WGS-84 record. Any height is dropped.
///
/// # Errors
/// `GeoError::CrsMismatch` unless the coordinate is tagged WGS-84, so a datum
/// is never silently laundered; `GeoError::OutOfRange` for an invalid record.
#[uniffi::export]
pub fn wgs84_from_coordinate(coord: Coordinate) -> Result<Wgs84, GeoError> {
    Ok(gc::china::Wgs84::try_from_coordinate(coord.try_into()?)?.into())
}

/// Canonical [`Coordinate`] → GCJ-02 record. Any height is dropped.
///
/// # Errors
/// `GeoError::CrsMismatch` unless the coordinate is tagged GCJ-02;
/// `GeoError::OutOfRange` for an invalid record.
#[uniffi::export]
pub fn gcj02_from_coordinate(coord: Coordinate) -> Result<Gcj02, GeoError> {
    Ok(gc::china::Gcj02::try_from_coordinate(coord.try_into()?)?.into())
}

/// Canonical [`Coordinate`] → BD-09 record. Any height is dropped.
///
/// # Errors
/// `GeoError::CrsMismatch` unless the coordinate is tagged BD-09;
/// `GeoError::OutOfRange` for an invalid record.
#[uniffi::export]
pub fn bd09_from_coordinate(coord: Coordinate) -> Result<Bd09, GeoError> {
    Ok(gc::china::Bd09::try_from_coordinate(coord.try_into()?)?.into())
}

// --- China datum conversions (exact forward) ---

/// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
///
/// # Errors
/// As [`wgs84_validate`], for an invalid record; the conversion itself
/// cannot fail.
#[uniffi::export]
pub fn wgs84_to_gcj02(p: Wgs84) -> Result<Gcj02, GeoError> {
    Ok(gc::china::Wgs84::try_from(p)?.to_gcj02().into())
}

/// WGS-84 → BD-09. **Exact** composition through GCJ-02; the longitude is
/// wrapped into `[-180, 180)`.
///
/// # Errors
/// `GeoError::OutOfRange` for an invalid record; `GeoError::OutsideDomain`
/// (operation `"BD-09"`) when the nudge would carry the latitude past the
/// North Pole.
#[uniffi::export]
pub fn wgs84_to_bd09(p: Wgs84) -> Result<Bd09, GeoError> {
    Ok(gc::china::Wgs84::try_from(p)?.try_to_bd09()?.into())
}

/// GCJ-02 → BD-09. **Exact** forward nudge, applied worldwide; the longitude
/// is wrapped into `[-180, 180)`.
///
/// # Errors
/// As [`wgs84_to_bd09`].
#[uniffi::export]
pub fn gcj02_to_bd09(p: Gcj02) -> Result<Bd09, GeoError> {
    Ok(gc::china::Gcj02::try_from(p)?.try_to_bd09()?.into())
}

// --- China datum conversions (approximate inverse) ---

/// GCJ-02 → WGS-84, fast single-step inverse (within 6 m away from the China
/// box edge). **Approximate**: the returned record carries `max_error_m`,
/// measured by running the forward offset once on the result: 6 m, or 6 m
/// plus that residual where the step crosses the box edge. An input just
/// outside the box is returned unchanged; where an inside point may also map
/// onto it, `max_error_m` covers that point too.
///
/// # Errors
/// As [`gcj02_validate`], for an invalid record; the conversion itself
/// cannot fail.
#[uniffi::export]
pub fn gcj02_to_wgs84_fast(p: Gcj02) -> Result<ApproxWgs84, GeoError> {
    Ok(gc::china::Gcj02::try_from(p)?.to_wgs84_fast().into())
}

/// GCJ-02 → WGS-84, refined fixed-point inverse. **Approximate**: the
/// returned record carries `max_error_m` — 0.5 m when the iteration
/// converged, and that plus the remaining residual when it could not. An
/// input outside the box is returned unchanged with `max_error_m` 0, unless
/// an inside point also maps onto it; then `max_error_m` covers that point.
///
/// # Errors
/// As [`gcj02_validate`], for an invalid record; the conversion itself
/// cannot fail.
#[uniffi::export]
pub fn gcj02_to_wgs84_refined(p: Gcj02) -> Result<ApproxWgs84, GeoError> {
    Ok(gc::china::Gcj02::try_from(p)?.to_wgs84_refined().into())
}

/// BD-09 → GCJ-02, fast single-step inverse (within 0.5 m, or 125 m within
/// 0.02° of the antimeridian). **Approximate**: the returned record carries
/// `max_error_m`.
///
/// # Errors
/// `GeoError::OutOfRange` for an invalid record; `GeoError::OutsideDomain`
/// (operation `"BD-09 inverse"`) when the preimage would lie past the South
/// Pole.
#[uniffi::export]
pub fn bd09_to_gcj02_fast(p: Bd09) -> Result<ApproxGcj02, GeoError> {
    Ok(gc::china::Bd09::try_from(p)?.try_to_gcj02_fast()?.into())
}

/// BD-09 → GCJ-02, refined fixed-point inverse. **Approximate**: the returned
/// record carries `max_error_m` — 0.5 m when the iteration converged, and
/// that plus the remaining residual when it could not.
///
/// # Errors
/// As [`bd09_to_gcj02_fast`].
#[uniffi::export]
pub fn bd09_to_gcj02_refined(p: Bd09) -> Result<ApproxGcj02, GeoError> {
    Ok(gc::china::Bd09::try_from(p)?.try_to_gcj02_refined()?.into())
}

/// BD-09 → WGS-84, fast composition through GCJ-02. **Approximate**: the
/// returned record carries the summed `max_error_m` — 6.5 m in the China box
/// away from its edge, more where the GCJ-02 step crosses the edge, hundreds
/// of meters (about 520 m at lat 40.0 just past the east edge) within about
/// 0.011° past the edge where an inside preimage widens the GCJ-02 leg, and
/// the BD-09 leg's bound alone farther outside the box.
///
/// # Errors
/// As [`bd09_to_gcj02_fast`].
#[uniffi::export]
pub fn bd09_to_wgs84_fast(p: Bd09) -> Result<ApproxWgs84, GeoError> {
    Ok(gc::china::Bd09::try_from(p)?.try_to_wgs84_fast()?.into())
}

/// BD-09 → WGS-84, refined composition through GCJ-02. **Approximate**: the
/// returned record carries the summed `max_error_m`.
///
/// # Errors
/// As [`bd09_to_gcj02_fast`].
#[uniffi::export]
pub fn bd09_to_wgs84_refined(p: Bd09) -> Result<ApproxWgs84, GeoError> {
    Ok(gc::china::Bd09::try_from(p)?.try_to_wgs84_refined()?.into())
}

// --- Baidu Web Mercator (Baidu's published fits, both ways) ---

/// BD-09 lat/lon → Baidu Web Mercator by Baidu's `LL2MC` polynomial fit. The
/// two directions are independent fits, not exact inverses: a round trip drifts
/// by up to about 0.25 m below 60° latitude and up to about 7 m above it.
///
/// # Errors
/// `GeoError::OutOfRange` for an invalid record; `GeoError::OutsideDomain`
/// (operation `"Baidu Mercator"`) beyond ±74° of latitude, where Baidu's
/// projection is not defined.
#[uniffi::export]
pub fn baidu_mercator_from_bd09(p: Bd09) -> Result<BaiduMercator, GeoError> {
    Ok(gc::china::BaiduMercator::try_from_bd09(p.try_into()?)?.into())
}

/// Baidu Web Mercator → BD-09 lat/lon by Baidu's `MC2LL` polynomial fit. Not an
/// exact inverse of [`baidu_mercator_from_bd09`] (see its round-trip drift).
///
/// # Errors
/// `GeoError::InvalidValue` naming `x` or `y` for a non-finite component, a
/// northing beyond the image of ±74°, or an easting past ±180° of longitude.
#[uniffi::export]
pub fn baidu_mercator_to_bd09(m: BaiduMercator) -> Result<Bd09, GeoError> {
    Ok(gc::china::BaiduMercator::try_from(m)?.try_to_bd09()?.into())
}

/// Baidu Web Mercator → canonical [`Coordinate`], tagged BD-09, via
/// [`baidu_mercator_to_bd09`].
///
/// # Errors
/// As [`baidu_mercator_to_bd09`].
#[uniffi::export]
pub fn baidu_mercator_to_coordinate(m: BaiduMercator) -> Result<Coordinate, GeoError> {
    Ok(gc::china::BaiduMercator::try_from(m)?
        .try_to_coordinate()?
        .into())
}

/// Canonical [`Coordinate`] → Baidu Web Mercator.
///
/// # Errors
/// [`GeoError::CrsMismatch`] unless the coordinate is BD-09 — a non-BD-09
/// coordinate must be converted to BD-09 first, never silently reprojected —
/// and otherwise as [`baidu_mercator_from_bd09`].
#[uniffi::export]
pub fn baidu_mercator_from_coordinate(coord: Coordinate) -> Result<BaiduMercator, GeoError> {
    Ok(gc::china::BaiduMercator::try_from_coordinate(coord.try_into()?)?.into())
}

// --- Helpers ---

/// Whether `(lat, lon)` lies outside the China bounding box, where the
/// WGS-84 ↔ GCJ-02 offset is the identity. The BD-09 nudge is not gated by
/// it.
#[uniffi::export]
pub fn out_of_china(lat: f64, lon: f64) -> bool {
    gc::china::out_of_china(lat, lon)
}
