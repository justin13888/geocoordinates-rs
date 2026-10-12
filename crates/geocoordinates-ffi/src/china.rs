//! China datums: the typed WGS-84 / GCJ-02 / BD-09 records, their exact
//! forward and approximate inverse conversions, and Baidu Web Mercator.

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

impl From<gc::Wgs84> for Wgs84 {
    fn from(p: gc::Wgs84) -> Self {
        Wgs84 {
            lat: p.lat,
            lon: p.lon,
        }
    }
}

impl From<Wgs84> for gc::Wgs84 {
    fn from(p: Wgs84) -> Self {
        gc::Wgs84::new(p.lat, p.lon)
    }
}

impl From<gc::Gcj02> for Gcj02 {
    fn from(p: gc::Gcj02) -> Self {
        Gcj02 {
            lat: p.lat,
            lon: p.lon,
        }
    }
}

impl From<Gcj02> for gc::Gcj02 {
    fn from(p: Gcj02) -> Self {
        gc::Gcj02::new(p.lat, p.lon)
    }
}

impl From<gc::Bd09> for Bd09 {
    fn from(p: gc::Bd09) -> Self {
        Bd09 {
            lat: p.lat,
            lon: p.lon,
        }
    }
}

impl From<Bd09> for gc::Bd09 {
    fn from(p: Bd09) -> Self {
        gc::Bd09::new(p.lat, p.lon)
    }
}

impl From<gc::BaiduMercator> for BaiduMercator {
    fn from(m: gc::BaiduMercator) -> Self {
        BaiduMercator { x: m.x, y: m.y }
    }
}

impl From<BaiduMercator> for gc::BaiduMercator {
    fn from(m: BaiduMercator) -> Self {
        gc::BaiduMercator::new(m.x, m.y)
    }
}

impl From<gc::Approx<gc::Wgs84>> for ApproxWgs84 {
    fn from(a: gc::Approx<gc::Wgs84>) -> Self {
        let max_error_m = a.max_error_m();
        let w = a.into_inner();
        ApproxWgs84 {
            lat: w.lat,
            lon: w.lon,
            max_error_m,
        }
    }
}

impl From<gc::Approx<gc::Gcj02>> for ApproxGcj02 {
    fn from(a: gc::Approx<gc::Gcj02>) -> Self {
        let max_error_m = a.max_error_m();
        let g = a.into_inner();
        ApproxGcj02 {
            lat: g.lat,
            lon: g.lon,
            max_error_m,
        }
    }
}

// --- China datum conversions (exact forward) ---

/// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
#[uniffi::export]
pub fn wgs84_to_gcj02(p: Wgs84) -> Result<Gcj02, GeoError> {
    gc::Wgs84::from(p)
        .try_to_gcj02()
        .map(Into::into)
        .map_err(Into::into)
}

/// WGS-84 → BD-09. **Exact** composition through GCJ-02.
#[uniffi::export]
pub fn wgs84_to_bd09(p: Wgs84) -> Result<Bd09, GeoError> {
    gc::Wgs84::from(p)
        .try_to_bd09()
        .map(Into::into)
        .map_err(Into::into)
}

/// GCJ-02 → BD-09. **Exact** forward nudge.
#[uniffi::export]
pub fn gcj02_to_bd09(p: Gcj02) -> Result<Bd09, GeoError> {
    gc::Gcj02::from(p)
        .try_to_bd09()
        .map(Into::into)
        .map_err(Into::into)
}

// --- China datum conversions (approximate inverse) ---

/// GCJ-02 → WGS-84, fast single-step inverse (~1–2 m). **Approximate**: the
/// returned record carries `max_error_m`.
#[uniffi::export]
pub fn gcj02_to_wgs84_fast(p: Gcj02) -> Result<ApproxWgs84, GeoError> {
    gc::Gcj02::from(p)
        .try_to_wgs84_fast()
        .map(Into::into)
        .map_err(Into::into)
}

/// GCJ-02 → WGS-84, refined fixed-point inverse (< 0.5 m). **Approximate**: the
/// returned record carries `max_error_m`.
#[uniffi::export]
pub fn gcj02_to_wgs84_refined(p: Gcj02) -> Result<ApproxWgs84, GeoError> {
    gc::Gcj02::from(p)
        .try_to_wgs84_refined()
        .map(Into::into)
        .map_err(Into::into)
}

/// BD-09 → GCJ-02, fast single-step inverse. **Approximate**: the returned
/// record carries `max_error_m`.
#[uniffi::export]
pub fn bd09_to_gcj02_fast(p: Bd09) -> Result<ApproxGcj02, GeoError> {
    gc::Bd09::from(p)
        .try_to_gcj02_fast()
        .map(Into::into)
        .map_err(Into::into)
}

/// BD-09 → GCJ-02, refined fixed-point inverse (sub-meter). **Approximate**:
/// the returned record carries `max_error_m`.
#[uniffi::export]
pub fn bd09_to_gcj02_refined(p: Bd09) -> Result<ApproxGcj02, GeoError> {
    gc::Bd09::from(p)
        .try_to_gcj02_refined()
        .map(Into::into)
        .map_err(Into::into)
}

/// BD-09 → WGS-84, refined composition through GCJ-02. **Approximate**: the
/// returned record carries the summed `max_error_m`.
#[uniffi::export]
pub fn bd09_to_wgs84_refined(p: Bd09) -> Result<ApproxWgs84, GeoError> {
    gc::Bd09::from(p)
        .try_to_wgs84_refined()
        .map(Into::into)
        .map_err(Into::into)
}

// --- Baidu Web Mercator (Baidu's published fits, both ways) ---

/// BD-09 lat/lon → Baidu Web Mercator by Baidu's `LL2MC` polynomial fit. The
/// two directions are independent fits, not exact inverses: a round trip drifts
/// by up to about 0.25 m below 60° latitude and up to about 7 m above it.
#[uniffi::export]
pub fn baidu_mercator_from_bd09(p: Bd09) -> Result<BaiduMercator, GeoError> {
    gc::BaiduMercator::try_from_bd09(gc::Bd09::from(p))
        .map(Into::into)
        .map_err(Into::into)
}

/// Baidu Web Mercator → BD-09 lat/lon by Baidu's `MC2LL` polynomial fit. Not an
/// exact inverse of [`baidu_mercator_from_bd09`] (see its round-trip drift).
#[uniffi::export]
pub fn baidu_mercator_to_bd09(m: BaiduMercator) -> Result<Bd09, GeoError> {
    gc::BaiduMercator::from(m)
        .try_to_bd09()
        .map(Into::into)
        .map_err(Into::into)
}

/// Baidu Web Mercator → canonical [`Coordinate`], tagged BD-09, via
/// [`baidu_mercator_to_bd09`].
#[uniffi::export]
pub fn baidu_mercator_to_coordinate(m: BaiduMercator) -> Result<Coordinate, GeoError> {
    gc::BaiduMercator::from(m)
        .try_to_coordinate()
        .map(Into::into)
        .map_err(Into::into)
}

/// Canonical [`Coordinate`] → Baidu Web Mercator.
///
/// Errors with [`GeoError::CrsMismatch`] unless the coordinate is BD-09 — a
/// non-BD-09 coordinate must be converted to BD-09 first, never silently
/// reprojected.
#[uniffi::export]
pub fn baidu_mercator_from_coordinate(coord: Coordinate) -> Result<BaiduMercator, GeoError> {
    gc::BaiduMercator::try_from_coordinate(coord.into())
        .map(Into::into)
        .map_err(GeoError::from)
}

// --- Helpers ---

/// Whether `(lat, lon)` lies outside the China bounding box, where every China
/// datum conversion is the identity.
#[uniffi::export]
pub fn out_of_china(lat: f64, lon: f64) -> bool {
    gc::china::out_of_china(lat, lon)
}
