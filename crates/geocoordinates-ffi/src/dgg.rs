//! Discrete global grids: H3 cells and S2 cell ids.

use geocoordinates as gc;

use crate::core::approx_coordinate;
use crate::{ApproxCoordinate, Coordinate, GeoError};

// ===========================================================================
// Discrete global grids — H3 and S2
// ===========================================================================

/// An H3 cell index — mirror of [`gc::H3Cell`](gc::dgg::H3Cell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct H3Cell {
    /// The 64-bit H3 cell index.
    pub value: u64,
}

/// Encode a WGS-84 coordinate to its H3 cell index at `resolution` (0–15).
#[uniffi::export]
pub fn h3_encode(coord: Coordinate, resolution: u8) -> Result<H3Cell, GeoError> {
    gc::dgg::H3Cell::encode(coord.into(), resolution)
        .map(|cell| H3Cell { value: cell.0 })
        .map_err(Into::into)
}

/// Decode an H3 cell to its center, with the cell-radius error bound.
#[uniffi::export]
pub fn h3_decode(cell: H3Cell) -> Result<ApproxCoordinate, GeoError> {
    gc::dgg::H3Cell(cell.value)
        .decode()
        .map(approx_coordinate)
        .map_err(GeoError::from)
}

/// An S2 cell id — mirror of [`gc::S2CellId`](gc::dgg::S2CellId).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct S2CellId {
    /// The 64-bit S2 cell id.
    pub value: u64,
}

/// Encode a WGS-84 coordinate to its S2 cell id at `level` (0–30).
#[uniffi::export]
pub fn s2_encode(coord: Coordinate, level: u8) -> Result<S2CellId, GeoError> {
    gc::dgg::S2CellId::encode(coord.into(), level)
        .map(|cell| S2CellId { value: cell.0 })
        .map_err(Into::into)
}

/// Decode an S2 cell to its center, with the maximum corner-distance bound.
#[uniffi::export]
pub fn s2_decode(cell: S2CellId) -> Result<ApproxCoordinate, GeoError> {
    gc::dgg::S2CellId(cell.value)
        .decode()
        .map(approx_coordinate)
        .map_err(GeoError::from)
}
