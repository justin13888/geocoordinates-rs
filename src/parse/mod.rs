//! Ingestion: turning real-world input into a [`Fix`](crate::Fix).
//!
//! - [`text`] — tolerant free-text / DMS / DDM parsing (always available).
//! - [`interchange`] — GeoJSON, WKT, GPX, KML (each behind a cargo feature).
//! - [`sensors`] — NMEA 0183 and EXIF/XMP image metadata (feature-gated).
//!
//! ## Axis order is first-class
//!
//! GeoJSON and WKT are **lon-lat (X,Y)**; humans and many EPSG CRS are
//! lat-first. Every parser records the [`AxisOrder`] it assumed and reports a
//! confidence so the application can decide whether to prompt the user.

pub mod text;

#[cfg(any(feature = "geojson", feature = "wkt", feature = "gpx", feature = "kml"))]
pub mod interchange;

#[cfg(any(feature = "nmea", feature = "exif"))]
pub mod sensors;

use crate::error::Result;
use crate::fix::Fix;

/// Axis ordering of a textual/structured coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AxisOrder {
    /// Latitude first (human/EPSG convention).
    LatLon,
    /// Longitude first (GeoJSON/WKT X,Y convention).
    LonLat,
}

/// The outcome of a parse, including ambiguities that were resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseReport {
    /// The parsed fix (coordinate + metadata + confidence).
    pub fix: Fix,
    /// The axis order that was assumed.
    pub axis_order: AxisOrder,
}

/// Best-effort parse of a single coordinate from arbitrary input.
///
/// Tries free-text heuristics first, then recognizes UTM/MGRS/Plus Code/geohash
/// tokens. Reports parse confidence and the assumed axis order.
///
/// # Errors
/// Returns [`crate::Error::Parse`] when no interpretation is found.
pub fn parse_coordinate(input: &str) -> Result<ParseReport> {
    todo!("delegate to text::parse with range/locale heuristics")
}
