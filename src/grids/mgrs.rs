//! MGRS (Military Grid Reference System) strings.
//!
//! An MGRS string (e.g. `4QFJ12345678`) is a grid-zone designator, 100 km
//! square ID, and easting/northing digits. It is validated at construction
//! ([`TryFrom<&str>`](Mgrs::try_from) / [`FromStr`]), which can fail on invalid
//! grid letters. Because the reference is then valid by construction, decoding
//! is infallible; it yields a square with extent, so [`Mgrs::to_coordinate`]
//! returns [`Approx`].

use core::str::FromStr;

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::Result;

/// A validated MGRS reference. Construct via [`TryFrom<&str>`](Mgrs::try_from),
/// [`FromStr`], or [`Mgrs::from_coordinate`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mgrs {
    text: String,
    precision_m: u32,
}

impl Mgrs {
    /// The canonical MGRS string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Precision in meters (10 km, 1 km, … 1 m) implied by the digit count.
    #[must_use]
    pub fn precision_m(&self) -> u32 {
        self.precision_m
    }

    /// Decode to a coordinate at the square's center; the error bound is half
    /// the square width. Infallible — the reference was validated at
    /// construction.
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
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] on a bad grid-zone designator
    /// or 100 km square id.
    fn try_from(s: &str) -> Result<Self> {
        todo!("validate grid-zone designator and 100 km square id")
    }
}

impl FromStr for Mgrs {
    type Err = crate::Error;

    /// Equivalent to [`TryFrom<&str>`](Mgrs::try_from).
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}
