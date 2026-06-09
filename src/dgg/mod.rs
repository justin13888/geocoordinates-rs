//! Discrete global grid indexing: Uber H3 and Google S2.
//!
//! These index a coordinate to a hierarchical cell for spatial joins, binning,
//! and proximity. Backed by external crates (`h3o`, `s2`) and gated behind the
//! `h3` / `s2` cargo features. A cell covers an area, so decoding a cell to a
//! representative point returns [`Approx`](crate::Approx).

use crate::approx::Approx;
use crate::coord::Coordinate;

/// An H3 cell index at a given resolution (0–15).
#[cfg(feature = "h3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct H3Cell(pub u64);

#[cfg(feature = "h3")]
impl H3Cell {
    /// Encode (index) a coordinate to its H3 cell at `resolution` (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, resolution: u8) -> Self {
        todo!("TODO: back with the h3o crate")
    }

    /// Decode to the cell's center coordinate; error bound is the cell radius.
    #[must_use]
    pub fn decode(self) -> Approx<Coordinate> {
        todo!()
    }
}

/// An S2 cell id.
#[cfg(feature = "s2")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct S2CellId(pub u64);

#[cfg(feature = "s2")]
impl S2CellId {
    /// Encode (index) a coordinate to its S2 cell at `level` (exact).
    #[must_use]
    pub fn encode(coord: Coordinate, level: u8) -> Self {
        todo!("TODO: back with the s2 crate")
    }

    /// Decode to the cell's center coordinate; error bound is the cell radius.
    #[must_use]
    pub fn decode(self) -> Approx<Coordinate> {
        todo!()
    }
}
