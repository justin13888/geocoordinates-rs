//! Local tangent-plane frames: ENU, NED, and AER.
//!
//! These are defined relative to a reference origin, so they are expressed as
//! methods taking the origin rather than `From` impls. The math is **exact**
//! (a rotation of the ECEF difference vector), so results are bare types.

use crate::coord::Coordinate;
use crate::units::Length;

/// East-North-Up offset from a reference origin, in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Enu {
    /// East offset (meters).
    pub east: f64,
    /// North offset (meters).
    pub north: f64,
    /// Up offset (meters).
    pub up: f64,
}

/// North-East-Down offset from a reference origin, in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ned {
    /// North offset (meters).
    pub north: f64,
    /// East offset (meters).
    pub east: f64,
    /// Down offset (meters).
    pub down: f64,
}

/// Azimuth-Elevation-Range relative to a reference origin.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aer {
    /// Azimuth (degrees clockwise from north).
    pub azimuth_deg: f64,
    /// Elevation (degrees above the local horizontal).
    pub elevation_deg: f64,
    /// Slant range.
    pub range: Length,
}

impl Enu {
    /// Compute the ENU offset of `target` relative to `origin` (exact).
    #[must_use]
    pub fn between(origin: Coordinate, target: Coordinate) -> Self {
        todo!("rotate the ECEF difference into the local tangent frame at origin")
    }

    /// Recover the absolute coordinate of this ENU offset about `origin`.
    #[must_use]
    pub fn to_coordinate(self, origin: Coordinate) -> Coordinate {
        todo!()
    }

    /// Convert to the NED convention.
    #[must_use]
    pub fn to_ned(self) -> Ned {
        todo!("north=north, east=east, down=-up")
    }

    /// Convert to azimuth/elevation/range.
    #[must_use]
    pub fn to_aer(self) -> Aer {
        todo!()
    }
}
