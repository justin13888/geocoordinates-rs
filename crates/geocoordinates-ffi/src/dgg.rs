//! Discrete global grids: H3 cells and S2 cell ids.

use geocoordinates as gc;

use crate::core::approx_coordinate;
use crate::{ApproxCoordinate, Coordinate, GeoError};

// ===========================================================================
// Discrete global grids — H3 and S2
// ===========================================================================

/// An H3 cell index — mirror of [`gc::H3Cell`](gc::dgg::H3Cell).
///
/// The record carries an unvalidated `u64`; every export that takes one
/// validates it as `H3Cell::new` does. Call [`h3_validate`] to check a
/// hand-built record up front. Every `H3Cell` this library returns is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct H3Cell {
    /// The 64-bit H3 cell index.
    pub value: u64,
}

/// Validate a (possibly hand-built) H3 cell record as `H3Cell::new` does.
///
/// # Errors
/// `GeoError::InvalidCellId` (grid `H3`) when `value` is not an H3 cell index
/// (for example `0`, or a vertex / edge index).
#[uniffi::export]
pub fn h3_validate(cell: H3Cell) -> Result<(), GeoError> {
    gc::dgg::H3Cell::new(cell.value)
        .map(drop)
        .map_err(GeoError::from)
}

/// Encode a WGS-84 coordinate to its H3 cell index at `resolution` (0–15).
///
/// # Errors
/// `GeoError::CrsMismatch` for a non-WGS-84 coordinate;
/// `GeoError::InvalidValue` for a `resolution` above 15;
/// `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid
/// coordinate record.
#[uniffi::export]
pub fn h3_encode(coord: Coordinate, resolution: u8) -> Result<H3Cell, GeoError> {
    gc::dgg::H3Cell::encode(coord.try_into()?, resolution)
        .map(|cell| H3Cell {
            value: cell.as_u64(),
        })
        .map_err(Into::into)
}

/// Decode an H3 cell to its center, with the cell-radius error bound.
///
/// # Errors
/// As [`h3_validate`]: the record is validated first.
#[uniffi::export]
pub fn h3_decode(cell: H3Cell) -> Result<ApproxCoordinate, GeoError> {
    gc::dgg::H3Cell::new(cell.value)
        .map(|cell| approx_coordinate(cell.decode()))
        .map_err(GeoError::from)
}

/// The resolution (0–15) of an H3 cell.
///
/// # Errors
/// As [`h3_validate`]: the record is validated first.
#[uniffi::export]
pub fn h3_resolution(cell: H3Cell) -> Result<u8, GeoError> {
    gc::dgg::H3Cell::new(cell.value)
        .map(gc::dgg::H3Cell::resolution)
        .map_err(GeoError::from)
}

/// An S2 cell id — mirror of [`gc::S2CellId`](gc::dgg::S2CellId).
///
/// The record carries an unvalidated `u64`; every export that takes one
/// validates it as `S2CellId::new` does. Call [`s2_validate`] to check a
/// hand-built record up front. Every `S2CellId` this library returns is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct S2CellId {
    /// The 64-bit S2 cell id.
    pub value: u64,
}

/// Validate a (possibly hand-built) S2 cell id record as `S2CellId::new`
/// does.
///
/// # Errors
/// `GeoError::InvalidCellId` (grid `S2`) when `value` is not an S2 cell id
/// (for example `0`, or an id with a face above 5 or no trailing level marker
/// bit).
#[uniffi::export]
pub fn s2_validate(cell: S2CellId) -> Result<(), GeoError> {
    gc::dgg::S2CellId::new(cell.value)
        .map(drop)
        .map_err(GeoError::from)
}

/// Encode a WGS-84 coordinate to its S2 cell id at `level` (0–30).
///
/// # Errors
/// `GeoError::CrsMismatch` for a non-WGS-84 coordinate;
/// `GeoError::InvalidValue` for a `level` above 30; `GeoError::OutOfRange` /
/// `GeoError::InvalidValue` for an invalid coordinate record.
#[uniffi::export]
pub fn s2_encode(coord: Coordinate, level: u8) -> Result<S2CellId, GeoError> {
    gc::dgg::S2CellId::encode(coord.try_into()?, level)
        .map(|cell| S2CellId {
            value: cell.as_u64(),
        })
        .map_err(Into::into)
}

/// Decode an S2 cell to its center, with the maximum corner-distance bound.
///
/// # Errors
/// As [`s2_validate`]: the record is validated first.
#[uniffi::export]
pub fn s2_decode(cell: S2CellId) -> Result<ApproxCoordinate, GeoError> {
    gc::dgg::S2CellId::new(cell.value)
        .map(|cell| approx_coordinate(cell.decode()))
        .map_err(GeoError::from)
}

/// The level (0–30) of an S2 cell.
///
/// # Errors
/// As [`s2_validate`]: the record is validated first.
#[uniffi::export]
pub fn s2_level(cell: S2CellId) -> Result<u8, GeoError> {
    gc::dgg::S2CellId::new(cell.value)
        .map(gc::dgg::S2CellId::level)
        .map_err(GeoError::from)
}
