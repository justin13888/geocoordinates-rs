//! Encoded/discrete location systems: Geohash, Plus Codes, Maidenhead.
//!
//! Each encodes a point into a variable-length string identifying a **cell**.
//! Strings are validated at construction ([`TryFrom<&str>`] / [`FromStr`]), so
//! [`encode`](Geohash::encode) is exact and [`decode`](Geohash::decode) is
//! infallible — the latter returns the cell center wrapped in [`Approx`] with
//! the cell half-extent as the error bound.
//!
//! [`TryFrom<&str>`]: Geohash::try_from
//! [`FromStr`]: std::str::FromStr

use core::str::FromStr;

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::Result;

/// A validated geohash string (base-32), e.g. `dr5regy`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Geohash(String);

/// A validated Open Location Code / Plus Code, e.g. `87G7X2VV+2V`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlusCode(String);

/// A validated Maidenhead locator (amateur radio grid square), e.g. `FN20`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Maidenhead(String);

impl Geohash {
    /// Encode a coordinate at the given character length (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, length: usize) -> Self {
        todo!()
    }

    /// The canonical geohash string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the cell center; error bound is the cell half-diagonal.
    /// Infallible — validated at construction.
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        todo!()
    }
}

impl TryFrom<&str> for Geohash {
    type Error = crate::Error;

    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for non-base-32 input.
    fn try_from(s: &str) -> Result<Self> {
        todo!("validate base-32 alphabet")
    }
}

impl FromStr for Geohash {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

impl PlusCode {
    /// Encode a coordinate at the given code length (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, length: usize) -> Self {
        todo!()
    }

    /// The canonical Plus Code string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the cell center wrapped in [`Approx`]. Infallible — validated
    /// at construction.
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        todo!()
    }
}

impl TryFrom<&str> for PlusCode {
    type Error = crate::Error;

    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for malformed codes.
    fn try_from(s: &str) -> Result<Self> {
        todo!("validate Open Location Code format")
    }
}

impl FromStr for PlusCode {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

impl Maidenhead {
    /// Encode a coordinate at the given number of pairs (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, pairs: usize) -> Self {
        todo!()
    }

    /// The canonical Maidenhead locator string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the grid-square center wrapped in [`Approx`]. Infallible —
    /// validated at construction.
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        todo!()
    }
}

impl TryFrom<&str> for Maidenhead {
    type Error = crate::Error;

    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for malformed locators.
    fn try_from(s: &str) -> Result<Self> {
        todo!("validate Maidenhead locator format")
    }
}

impl FromStr for Maidenhead {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}
