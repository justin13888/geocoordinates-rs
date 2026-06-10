//! UniFFI bindings for [`geocoordinates`](https://docs.rs/geocoordinates).
//!
//! This crate exposes a **deliberately curated subset** of the Rust API to
//! Python, Kotlin, Swift, and Ruby. The Rust library is intentionally
//! idiomatic, and several of its idioms cannot cross an FFI boundary, so they
//! are re-expressed here as flat, language-neutral records and free functions:
//!
//! | Rust idiom | FFI form here |
//! |---|---|
//! | `Approx<T>` (generic wrapper) | flat records [`ApproxWgs84`] / [`ApproxGcj02`] carrying `max_error_m` |
//! | `LatLon` trait / `&impl LatLon` | functions take the concrete [`Coordinate`] record |
//! | `Length` + operator overloads | distance returned as `f64` meters ([`haversine_distance_m`]) |
//! | `From` / `TryFrom` conversions | named free functions (`wgs84_to_gcj02`, …) |
//!
//! Exactness stays visible across the boundary: exact forward conversions
//! return a bare datum record, while approximate inverses return an `Approx*`
//! record and keep the `_fast` / `_refined` suffix.
//!
//! Intentionally **not** exposed in this release: the `Fix` observation family
//! (no v0.1 producer; `SystemTime` maps awkwardly), the `LatLon` trait, and
//! `Length` / `LengthUnit` arithmetic. They return once a release needs them.

use geocoordinates as gc;

uniffi::setup_scaffolding!();

// ===========================================================================
// Mirror types
//
// Hand-written mirrors (rather than `#[uniffi::remote]`) so the FFI surface is a
// curated, flattened subset — and because `Approx<T>` must be flattened anyway.
// ===========================================================================

/// Coordinate reference system / datum tag — mirror of [`gc::Crs`].
///
/// A closed snapshot of the upstream `#[non_exhaustive]` enum. A variant added
/// upstream must be mirrored here too (see `AGENTS.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Crs {
    /// WGS-84 — the global GNSS reference and library default.
    Wgs84,
    /// GCJ-02 — "Mars" coordinates used by Chinese map providers.
    Gcj02,
    /// BD-09 — Baidu's additional obfuscation atop GCJ-02.
    Bd09,
    /// NAD27 — North American Datum 1927.
    Nad27,
    /// Tokyo datum (legacy Japan / Korea).
    Tokyo,
    /// Pulkovo-1942 / SK-42.
    Pulkovo42,
}

/// A height value tagged by its reference surface — mirror of [`gc::Height`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum Height {
    /// Meters above the reference ellipsoid.
    Ellipsoidal {
        /// Height in meters.
        meters: f64,
    },
    /// Meters above the geoid (mean sea level).
    Orthometric {
        /// Height in meters.
        meters: f64,
    },
}

/// The canonical coordinate — mirror of [`gc::Coordinate`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Coordinate {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
    /// Optional height (ellipsoidal or orthometric).
    pub height: Option<Height>,
    /// The reference system the position is expressed in.
    pub crs: Crs,
}

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

/// Errors surfaced across the FFI boundary — a flattened mirror of [`gc::Error`].
///
/// Datum-bearing variants carry their reference systems as their canonical
/// short names (e.g. `"BD-09"`); everything else collapses into [`GeoError::Other`].
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum GeoError {
    /// A coordinate carried the wrong reference system for the requested operation.
    #[error("crs mismatch: expected {expected}, found {found}")]
    CrsMismatch {
        /// The reference system that was required.
        expected: String,
        /// The reference system the coordinate actually carried.
        found: String,
    },
    /// A latitude/longitude fell outside its valid domain.
    #[error("coordinate out of valid range: lat={lat}, lon={lon}")]
    OutOfRange {
        /// Offending latitude in degrees.
        lat: f64,
        /// Offending longitude in degrees.
        lon: f64,
    },
    /// Any other library error, with its message preserved.
    #[error("{message}")]
    Other {
        /// The underlying error's display message.
        message: String,
    },
}

// ===========================================================================
// Translation layer (mirror <-> core)
// ===========================================================================

impl From<gc::Crs> for Crs {
    fn from(c: gc::Crs) -> Self {
        match c {
            gc::Crs::Wgs84 => Crs::Wgs84,
            gc::Crs::Gcj02 => Crs::Gcj02,
            gc::Crs::Bd09 => Crs::Bd09,
            gc::Crs::Nad27 => Crs::Nad27,
            gc::Crs::Tokyo => Crs::Tokyo,
            gc::Crs::Pulkovo42 => Crs::Pulkovo42,
            // `gc::Crs` is `#[non_exhaustive]`. An upstream variant this mirror
            // doesn't know about falls back to WGS-84 rather than panicking;
            // keep the mirror in sync (see `AGENTS.md`) so this arm stays dead.
            _ => Crs::Wgs84,
        }
    }
}

impl From<Crs> for gc::Crs {
    fn from(c: Crs) -> Self {
        match c {
            Crs::Wgs84 => gc::Crs::Wgs84,
            Crs::Gcj02 => gc::Crs::Gcj02,
            Crs::Bd09 => gc::Crs::Bd09,
            Crs::Nad27 => gc::Crs::Nad27,
            Crs::Tokyo => gc::Crs::Tokyo,
            Crs::Pulkovo42 => gc::Crs::Pulkovo42,
        }
    }
}

impl From<gc::Height> for Height {
    fn from(h: gc::Height) -> Self {
        match h {
            gc::Height::Ellipsoidal(meters) => Height::Ellipsoidal { meters },
            gc::Height::Orthometric(meters) => Height::Orthometric { meters },
        }
    }
}

impl From<Height> for gc::Height {
    fn from(h: Height) -> Self {
        match h {
            Height::Ellipsoidal { meters } => gc::Height::Ellipsoidal(meters),
            Height::Orthometric { meters } => gc::Height::Orthometric(meters),
        }
    }
}

impl From<gc::Coordinate> for Coordinate {
    fn from(c: gc::Coordinate) -> Self {
        Coordinate {
            lat: c.lat,
            lon: c.lon,
            height: c.height.map(Into::into),
            crs: c.crs.into(),
        }
    }
}

impl From<Coordinate> for gc::Coordinate {
    fn from(c: Coordinate) -> Self {
        let base = gc::Coordinate::new(c.lat, c.lon, c.crs.into());
        match c.height {
            Some(h) => base.with_height(h.into()),
            None => base,
        }
    }
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

impl From<gc::Error> for GeoError {
    fn from(e: gc::Error) -> Self {
        match e {
            gc::Error::CrsMismatch { expected, found } => GeoError::CrsMismatch {
                expected: expected.to_string(),
                found: found.to_string(),
            },
            gc::Error::OutOfRange { lat, lon } => GeoError::OutOfRange { lat, lon },
            other => GeoError::Other {
                message: other.to_string(),
            },
        }
    }
}

// ===========================================================================
// Exported API
// ===========================================================================

// --- Coordinate constructors ---

/// Construct a WGS-84 [`Coordinate`] from latitude/longitude in degrees.
#[uniffi::export]
pub fn coordinate_wgs84(lat: f64, lon: f64) -> Coordinate {
    gc::Coordinate::wgs84(lat, lon).into()
}

/// Construct a GCJ-02 [`Coordinate`] from latitude/longitude in degrees.
#[uniffi::export]
pub fn coordinate_gcj02(lat: f64, lon: f64) -> Coordinate {
    gc::Coordinate::gcj02(lat, lon).into()
}

/// Construct a BD-09 [`Coordinate`] from latitude/longitude in degrees.
#[uniffi::export]
pub fn coordinate_bd09(lat: f64, lon: f64) -> Coordinate {
    gc::Coordinate::bd09(lat, lon).into()
}

// --- China datum conversions (exact forward) ---

/// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
#[uniffi::export]
pub fn wgs84_to_gcj02(p: Wgs84) -> Gcj02 {
    gc::Wgs84::from(p).to_gcj02().into()
}

/// WGS-84 → BD-09. **Exact** composition through GCJ-02.
#[uniffi::export]
pub fn wgs84_to_bd09(p: Wgs84) -> Bd09 {
    gc::Wgs84::from(p).to_bd09().into()
}

/// GCJ-02 → BD-09. **Exact** forward nudge.
#[uniffi::export]
pub fn gcj02_to_bd09(p: Gcj02) -> Bd09 {
    gc::Gcj02::from(p).to_bd09().into()
}

// --- China datum conversions (approximate inverse) ---

/// GCJ-02 → WGS-84, fast single-step inverse (~1–2 m). **Approximate**: the
/// returned record carries `max_error_m`.
#[uniffi::export]
pub fn gcj02_to_wgs84_fast(p: Gcj02) -> ApproxWgs84 {
    gc::Gcj02::from(p).to_wgs84_fast().into()
}

/// GCJ-02 → WGS-84, refined fixed-point inverse (< 0.5 m). **Approximate**: the
/// returned record carries `max_error_m`.
#[uniffi::export]
pub fn gcj02_to_wgs84_refined(p: Gcj02) -> ApproxWgs84 {
    gc::Gcj02::from(p).to_wgs84_refined().into()
}

/// BD-09 → GCJ-02, fast single-step inverse. **Approximate**: the returned
/// record carries `max_error_m`.
#[uniffi::export]
pub fn bd09_to_gcj02_fast(p: Bd09) -> ApproxGcj02 {
    gc::Bd09::from(p).to_gcj02_fast().into()
}

/// BD-09 → GCJ-02, refined fixed-point inverse (sub-meter). **Approximate**:
/// the returned record carries `max_error_m`.
#[uniffi::export]
pub fn bd09_to_gcj02_refined(p: Bd09) -> ApproxGcj02 {
    gc::Bd09::from(p).to_gcj02_refined().into()
}

/// BD-09 → WGS-84, refined composition through GCJ-02. **Approximate**: the
/// returned record carries the summed `max_error_m`.
#[uniffi::export]
pub fn bd09_to_wgs84_refined(p: Bd09) -> ApproxWgs84 {
    gc::Bd09::from(p).to_wgs84_refined().into()
}

// --- Baidu Web Mercator (exact, both ways) ---

/// BD-09 lat/lon → Baidu Web Mercator (exact forward projection).
#[uniffi::export]
pub fn baidu_mercator_from_bd09(p: Bd09) -> BaiduMercator {
    gc::BaiduMercator::from_bd09(gc::Bd09::from(p)).into()
}

/// Baidu Web Mercator → BD-09 lat/lon (exact inverse projection).
#[uniffi::export]
pub fn baidu_mercator_to_bd09(m: BaiduMercator) -> Bd09 {
    gc::BaiduMercator::from(m).to_bd09().into()
}

/// Baidu Web Mercator → canonical [`Coordinate`], tagged BD-09 (exact).
#[uniffi::export]
pub fn baidu_mercator_to_coordinate(m: BaiduMercator) -> Coordinate {
    gc::BaiduMercator::from(m).to_coordinate().into()
}

/// Canonical [`Coordinate`] → Baidu Web Mercator.
///
/// Errors with [`GeoError::CrsMismatch`] unless the coordinate is BD-09 — a
/// non-BD-09 coordinate must be converted to BD-09 first, never silently
/// reprojected.
#[uniffi::export]
pub fn baidu_mercator_try_from_coordinate(coord: Coordinate) -> Result<BaiduMercator, GeoError> {
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

/// Cheap spherical (haversine) distance between two coordinates, in **meters**.
///
/// `Length` and its operators do not cross the FFI boundary, so the scalar
/// meters value is returned directly.
#[uniffi::export]
pub fn haversine_distance_m(a: Coordinate, b: Coordinate) -> f64 {
    let a: gc::Coordinate = a.into();
    let b: gc::Coordinate = b.into();
    gc::geodesy::haversine_distance(&a, &b).meters()
}
