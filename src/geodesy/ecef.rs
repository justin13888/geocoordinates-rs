//! Earth-Centered, Earth-Fixed (ECEF) geocentric coordinates.
//!
//! ECEF is the bridge format for almost every datum transformation. The
//! geodetic ↔ ECEF conversion is closed-form and treated as **exact** (the
//! inverse uses Bowring's well-converged formula), so it implements [`From`].

use super::ellipsoid::Ellipsoid;
use crate::coord::Coordinate;

/// A geocentric ECEF position in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ecef {
    /// X axis (meters), through the prime meridian at the equator.
    pub x: f64,
    /// Y axis (meters), 90° east at the equator.
    pub y: f64,
    /// Z axis (meters), through the north pole.
    pub z: f64,
}

impl Ecef {
    /// Construct from X/Y/Z in meters.
    #[must_use]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// ECEF → geodetic [`Coordinate`] (lat/lon/height) on the given ellipsoid
    /// (exact, Bowring closed-form inverse).
    ///
    /// The result carries no [`Crs`](crate::Crs) tag of its own — ECEF is
    /// datum-agnostic; the caller is responsible for tagging the coordinate
    /// with the reference system the `ellipsoid` belongs to.
    #[must_use]
    pub fn to_coordinate(self, ellipsoid: Ellipsoid) -> Coordinate {
        todo!("Bowring closed-form inverse; height from ellipsoid")
    }

    /// Geodetic [`Coordinate`] → ECEF on the given ellipsoid (exact, closed
    /// form).
    #[must_use]
    pub fn from_coordinate(coord: Coordinate, ellipsoid: Ellipsoid) -> Self {
        todo!("standard forward formula using N = a / sqrt(1 - e² sin²φ)")
    }
}
