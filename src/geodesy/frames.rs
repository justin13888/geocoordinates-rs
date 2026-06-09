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
    /// The ENU offset of `target` relative to `origin` (exact).
    #[must_use]
    pub fn from_coordinate(target: Coordinate, origin: Coordinate) -> Self {
        todo!("rotate the ECEF difference into the local tangent frame at origin")
    }

    /// Recover the absolute coordinate of this ENU offset about `origin`.
    #[must_use]
    pub fn to_coordinate(self, origin: Coordinate) -> Coordinate {
        todo!()
    }

    /// Convert to the NED convention (exact).
    #[must_use]
    pub fn to_ned(self) -> Ned {
        Ned {
            north: self.north,
            east: self.east,
            down: -self.up,
        }
    }

    /// Convert to azimuth/elevation/range (exact).
    #[must_use]
    pub fn to_aer(self) -> Aer {
        todo!("azimuth=atan2(east,north); range=hypot3; elevation=asin(up/range)")
    }
}

impl Ned {
    /// The NED offset of `target` relative to `origin` (exact).
    #[must_use]
    pub fn from_coordinate(target: Coordinate, origin: Coordinate) -> Self {
        Enu::from_coordinate(target, origin).to_ned()
    }

    /// Recover the absolute coordinate of this NED offset about `origin`.
    #[must_use]
    pub fn to_coordinate(self, origin: Coordinate) -> Coordinate {
        self.to_enu().to_coordinate(origin)
    }

    /// Convert to the ENU convention (exact).
    #[must_use]
    pub fn to_enu(self) -> Enu {
        Enu {
            east: self.east,
            north: self.north,
            up: -self.down,
        }
    }

    /// Convert to azimuth/elevation/range (exact).
    #[must_use]
    pub fn to_aer(self) -> Aer {
        self.to_enu().to_aer()
    }
}

impl Aer {
    /// The azimuth/elevation/range of `target` relative to `origin` (exact).
    #[must_use]
    pub fn from_coordinate(target: Coordinate, origin: Coordinate) -> Self {
        Enu::from_coordinate(target, origin).to_aer()
    }

    /// Recover the absolute coordinate of this AER offset about `origin`.
    #[must_use]
    pub fn to_coordinate(self, origin: Coordinate) -> Coordinate {
        self.to_enu().to_coordinate(origin)
    }

    /// Convert to the ENU convention (exact).
    #[must_use]
    pub fn to_enu(self) -> Enu {
        todo!("east=range·cos(el)·sin(az); north=range·cos(el)·cos(az); up=range·sin(el)")
    }

    /// Convert to the NED convention (exact).
    #[must_use]
    pub fn to_ned(self) -> Ned {
        self.to_enu().to_ned()
    }
}

// Frame-to-frame conversions are exact and origin-independent (pure rotation /
// repackaging), so they also implement `From`, per the conversion convention.
impl From<Enu> for Ned {
    fn from(enu: Enu) -> Ned {
        enu.to_ned()
    }
}
impl From<Ned> for Enu {
    fn from(ned: Ned) -> Enu {
        ned.to_enu()
    }
}
impl From<Enu> for Aer {
    fn from(enu: Enu) -> Aer {
        enu.to_aer()
    }
}
impl From<Aer> for Enu {
    fn from(aer: Aer) -> Enu {
        aer.to_enu()
    }
}
impl From<Ned> for Aer {
    fn from(ned: Ned) -> Aer {
        ned.to_aer()
    }
}
impl From<Aer> for Ned {
    fn from(aer: Aer) -> Ned {
        aer.to_ned()
    }
}
