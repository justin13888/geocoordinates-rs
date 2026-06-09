//! UTM (Universal Transverse Mercator) and UPS (Universal Polar Stereographic).
//!
//! The projection is deterministic and invertible to high precision, so it is
//! **exact**. Constructing from a coordinate can fail (UTM is undefined at the
//! poles, where UPS is used instead), so the lat/lon → grid direction uses
//! [`TryFrom`].

use crate::coord::Coordinate;
use crate::error::Result;

/// Northern or southern hemisphere band for a UTM coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hemisphere {
    /// Northern hemisphere.
    North,
    /// Southern hemisphere.
    South,
}

/// A UTM coordinate: zone, hemisphere, easting, and northing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Utm {
    /// Longitude zone number, 1–60.
    pub zone: u8,
    /// Hemisphere band.
    pub hemisphere: Hemisphere,
    /// Easting in meters (false-easting applied).
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

/// A UPS coordinate for the polar regions where UTM is undefined.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ups {
    /// Whether this is the north or south polar zone.
    pub hemisphere: Hemisphere,
    /// Easting in meters.
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

impl Utm {
    /// UTM → geodetic WGS-84 coordinate (exact inverse projection).
    #[must_use]
    pub fn to_coordinate(self) -> Coordinate {
        todo!("inverse transverse Mercator")
    }
}

impl TryFrom<Coordinate> for Utm {
    type Error = crate::Error;

    /// Geodetic → UTM. Fails in the polar regions (use [`Ups`] there).
    fn try_from(coord: Coordinate) -> Result<Self> {
        todo!("pick zone from lon; forward transverse Mercator; error near poles")
    }
}

impl Ups {
    /// UPS → geodetic WGS-84 coordinate (exact inverse polar stereographic).
    #[must_use]
    pub fn to_coordinate(self) -> Coordinate {
        todo!("inverse polar stereographic")
    }
}

impl TryFrom<Coordinate> for Ups {
    type Error = crate::Error;

    /// Geodetic → UPS. Fails outside the polar zones (north of ~84°N or south
    /// of ~80°S is UPS territory; use [`Utm`] elsewhere).
    fn try_from(coord: Coordinate) -> Result<Self> {
        todo!("forward polar stereographic; error outside the polar zones")
    }
}
