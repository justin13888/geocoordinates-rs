//! China coordinate obfuscation datums: WGS-84 ↔ GCJ-02 ↔ BD-09.
//!
//! Real GPS (WGS-84) is illegal to publish on Chinese maps, so providers apply
//! a deliberate nonlinear offset (**GCJ-02**, "Mars coordinates"). Baidu adds a
//! second layer (**BD-09**).
//!
//! ## Exactness (visible in the type signatures)
//!
//! | Direction | Nature | API |
//! |---|---|---|
//! | WGS-84 → GCJ-02 | exact forward offset | [`From`] / [`Wgs84::to_gcj02`] |
//! | GCJ-02 → BD-09 | exact (empirical) forward | [`From`] / [`Gcj02::to_bd09`] |
//! | WGS-84 → BD-09 | exact composition | [`From`] / [`Wgs84::to_bd09`] |
//! | GCJ-02 → WGS-84 | **approximate** inverse | [`Gcj02::to_wgs84_refined`] → [`Approx`](crate::Approx) |
//! | BD-09 → GCJ-02 | **approximate** inverse | [`Bd09::to_gcj02_refined`] → [`Approx`](crate::Approx) |
//! | BD-09 → WGS-84 | **approximate** composition | [`Bd09::to_wgs84_refined`] → [`Approx`](crate::Approx) |
//!
//! Outside China (see [`out_of_china`]) every conversion is the identity. This
//! creates a documented discontinuity at the border.

mod bd09;
mod gcj02;

use crate::coord::{Coordinate, Crs};
use crate::error::Error;

// TODO: Validate these constants vv
/// Semi-major axis of the Krasovsky/GCJ reference ellipsoid, in meters.
pub(crate) const EARTH_R: f64 = 6_378_137.0;
/// Eccentricity squared used by the GCJ offset.
pub(crate) const EE: f64 = 0.006_693_421_622_965_943;
/// Baidu's magic angular constant, `π · 3000 / 180`.
///
/// The canonical BD-09 constant. Plain `π` (used by several stale ports) is a
/// bug that introduces a systematic ~2 m error; do not use it.
pub(crate) const X_PI: f64 = std::f64::consts::PI * 3000.0 / 180.0;
// TODO(end)

/// Bounding box gate: conversions are the identity outside China.
///
/// Box: `72.004 ≤ lon ≤ 137.8347`, `0.8293 ≤ lat ≤ 55.8271`.
#[must_use]
pub fn out_of_china(lat: f64, lon: f64) -> bool {
    todo!("hard bounding-box check; document the border discontinuity")
}

/// A WGS-84 position (real GPS / OpenStreetMap), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Wgs84 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A GCJ-02 position (Google China, AutoNavi/高德, Tencent), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gcj02 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A BD-09 position (Baidu Maps only), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bd09 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

macro_rules! impl_latlon {
    ($($t:ty),*) => {$(
        impl $t {
            /// Construct from latitude/longitude in decimal degrees.
            #[must_use]
            pub fn new(lat: f64, lon: f64) -> Self { Self { lat, lon } }
        }
        impl crate::coord::LatLon for $t {
            fn lat(&self) -> f64 { self.lat }
            fn lon(&self) -> f64 { self.lon }
        }
    )*};
}
impl_latlon!(Wgs84, Gcj02, Bd09);

// --- Bridges to the canonical `Coordinate` ---
//
// Datum newtype → `Coordinate` is exact and total: it injects the correct
// [`Crs`] tag and leaves height unset. `Coordinate` → newtype is fallible: the
// coordinate's [`Crs`] must match (so a datum is never silently laundered), and
// any height is dropped (GCJ-02 / BD-09 are 2D obfuscations with no vertical
// datum).

impl From<Wgs84> for Coordinate {
    fn from(p: Wgs84) -> Self {
        Coordinate::wgs84(p.lat, p.lon)
    }
}

impl From<Gcj02> for Coordinate {
    fn from(p: Gcj02) -> Self {
        Coordinate::gcj02(p.lat, p.lon)
    }
}

impl From<Bd09> for Coordinate {
    fn from(p: Bd09) -> Self {
        Coordinate::bd09(p.lat, p.lon)
    }
}

impl TryFrom<Coordinate> for Wgs84 {
    type Error = Error;

    /// Fails with [`Error::CrsMismatch`] unless `coord.crs` is [`Crs::Wgs84`].
    fn try_from(coord: Coordinate) -> Result<Self, Error> {
        match coord.crs {
            Crs::Wgs84 => Ok(Wgs84 {
                lat: coord.lat,
                lon: coord.lon,
            }),
            found => Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                found,
            }),
        }
    }
}

impl TryFrom<Coordinate> for Gcj02 {
    type Error = Error;

    /// Fails with [`Error::CrsMismatch`] unless `coord.crs` is [`Crs::Gcj02`].
    fn try_from(coord: Coordinate) -> Result<Self, Error> {
        match coord.crs {
            Crs::Gcj02 => Ok(Gcj02 {
                lat: coord.lat,
                lon: coord.lon,
            }),
            found => Err(Error::CrsMismatch {
                expected: Crs::Gcj02,
                found,
            }),
        }
    }
}

impl TryFrom<Coordinate> for Bd09 {
    type Error = Error;

    /// Fails with [`Error::CrsMismatch`] unless `coord.crs` is [`Crs::Bd09`].
    fn try_from(coord: Coordinate) -> Result<Self, Error> {
        match coord.crs {
            Crs::Bd09 => Ok(Bd09 {
                lat: coord.lat,
                lon: coord.lon,
            }),
            found => Err(Error::CrsMismatch {
                expected: Crs::Bd09,
                found,
            }),
        }
    }
}
