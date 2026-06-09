//! WGS-84 ↔ GCJ-02 offset math.
//!
//! The forward offset (WGS-84 → GCJ-02) is the canonical, de-facto-standard
//! empirical polynomial + sinusoids — exact within the published algorithm. The
//! inverse (GCJ-02 → WGS-84) has no closed form:
//!
//! - [`Gcj02::to_wgs84_fast`] subtracts the offset computed at the wrong point
//!   (~1–2 m error).
//! - [`Gcj02::to_wgs84_refined`] uses fixed-point iteration
//!   (`wgs += target − wgs2gcj(wgs)`), converging to < 0.5 m. Preferred over the
//!   30-iteration binary search used by older ports.

use super::{Gcj02, Wgs84};
use crate::approx::Approx;

/// Raw GCJ offset polynomial in latitude, evaluated on `(lon − 105, lat − 35)`.
fn transform_lat(x: f64, y: f64) -> f64 {
    todo!("canonical lat polynomial + sinusoids (see go/transform.go)")
}

/// Raw GCJ offset polynomial in longitude, evaluated on `(lon − 105, lat − 35)`.
fn transform_lon(x: f64, y: f64) -> f64 {
    todo!("canonical lon polynomial + sinusoids (see go/transform.go)")
}

/// Convert a raw offset to a (dLat, dLon) pair in degrees using the ellipsoid
/// radius of curvature at `lat`.
fn delta(lat: f64, lon: f64) -> (f64, f64) {
    todo!("apply EARTH_R / EE to scale the raw offset into degrees")
}

impl Wgs84 {
    /// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
    #[must_use]
    pub fn to_gcj02(self) -> Gcj02 {
        todo!("if out_of_china -> identity; else add delta()")
    }
}

impl From<Wgs84> for Gcj02 {
    /// Exact forward offset.
    fn from(wgs: Wgs84) -> Self {
        wgs.to_gcj02()
    }
}

impl Gcj02 {
    /// GCJ-02 → WGS-84, fast single-step inverse (~1–2 m error).
    #[must_use]
    pub fn to_wgs84_fast(self) -> Approx<Wgs84> {
        todo!("subtract delta() computed at the GCJ point; ~1-2 m bound")
    }

    /// GCJ-02 → WGS-84, refined fixed-point inverse (< 0.5 m error).
    #[must_use]
    pub fn to_wgs84_refined(self) -> Approx<Wgs84> {
        todo!("iterate wgs += target - wgs.to_gcj02() to tolerance; <0.5 m bound")
    }
}
