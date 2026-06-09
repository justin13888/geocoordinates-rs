//! Encoded/discrete location systems: Geohash, Plus Codes, Maidenhead.
//!
//! Each encodes a point into a variable-length string identifying a **cell**.
//! Encoding is exact; decoding returns the cell center wrapped in
//! [`Approx`](crate::Approx) with the cell half-extent as the error bound.

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::Result;

/// A geohash string (base-32), e.g. `dr5regy`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Geohash(pub String);

/// An Open Location Code / Plus Code, e.g. `87G7X2VV+2V`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlusCode(pub String);

/// A Maidenhead locator (amateur radio grid square), e.g. `FN20`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Maidenhead(pub String);

impl Geohash {
    /// Encode a coordinate at the given character length (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, length: usize) -> Self {
        todo!()
    }

    /// Decode to the cell center; error bound is the cell half-diagonal.
    ///
    /// # Errors
    /// Returns an error for non-base-32 input.
    pub fn decode(&self) -> Result<Approx<Coordinate>> {
        todo!()
    }
}

impl PlusCode {
    /// Encode a coordinate at the given code length (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, length: usize) -> Self {
        todo!()
    }

    /// Decode to the cell center wrapped in [`Approx`].
    ///
    /// # Errors
    /// Returns an error for malformed codes.
    pub fn decode(&self) -> Result<Approx<Coordinate>> {
        todo!()
    }
}

impl Maidenhead {
    /// Encode a coordinate at the given number of pairs (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, pairs: usize) -> Self {
        todo!()
    }

    /// Decode to the grid-square center wrapped in [`Approx`].
    ///
    /// # Errors
    /// Returns an error for malformed locators.
    pub fn decode(&self) -> Result<Approx<Coordinate>> {
        todo!()
    }
}
