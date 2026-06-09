//! PROJ-backed transforms for the full EPSG / datum long tail.
//!
//! The references are explicit: **wrap PROJ, don't reimplement it.** This
//! module (cargo feature `proj`) exposes the high-value primitives — transform
//! between arbitrary EPSG CRS, Helmert/Molodensky/grid (NTv2) datum shifts —
//! by delegating to the C PROJ library. GCJ-02/BD-09 are *not* PROJ-supported
//! and stay in [`crate::china`].
//!
//! Adds a C build dependency; off by default.

use crate::coord::Coordinate;
use crate::error::Result;

/// A coordinate reference system identified by EPSG code or PROJ string.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CrsId {
    /// EPSG code, e.g. `4326` (WGS-84) or `3857` (Web Mercator).
    Epsg(u32),
    /// A raw PROJ pipeline/definition string.
    Proj(String),
}

/// Transform a coordinate between two CRS via PROJ.
///
/// # Errors
/// Returns an error if PROJ cannot build the transformation or the point is out
/// of the transform's domain.
pub fn transform(coord: Coordinate, from: &CrsId, to: &CrsId) -> Result<Coordinate> {
    todo!("TODO: back with the `proj` crate; cache Proj objects per (from,to)")
}
