//! MGRS (Military Grid Reference System) strings.
//!
//! An MGRS string (e.g. `4QFJ12345678`) is a grid-zone designator, 100 km
//! square ID, and easting/northing digits. Parsing can fail (invalid grid
//! letters) → [`TryFrom`]. Decoding to a point yields a square with extent, so
//! [`Mgrs::to_coordinate`] returns [`Approx`](crate::Approx).

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::Result;

/// A parsed MGRS reference.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mgrs {
    /// The canonical MGRS string.
    pub text: String,
    /// Precision in meters (10 km, 1 km, … 1 m) implied by the digit count.
    pub precision_m: u32,
}

impl Mgrs {
    /// Decode to a coordinate at the square's center; the error bound is half
    /// the square width.
    #[must_use]
    pub fn to_coordinate(&self) -> Approx<Coordinate> {
        todo!("via UTM/UPS; bound = precision_m / 2")
    }

    /// Encode a coordinate to MGRS at the given precision in meters.
    #[must_use]
    pub fn from_coordinate(coord: Coordinate, precision_m: u32) -> Self {
        todo!()
    }
}

impl TryFrom<&str> for Mgrs {
    type Error = crate::Error;

    /// Parse and validate an MGRS string.
    fn try_from(s: &str) -> Result<Self> {
        todo!("validate grid-zone designator and 100 km square id")
    }
}
