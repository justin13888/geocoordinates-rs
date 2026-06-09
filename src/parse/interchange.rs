//! Structured interchange formats: GeoJSON, WKT, GPX, KML.
//!
//! **Axis-order trap:** GeoJSON and WKT positions are lon-lat (X,Y). These
//! parsers normalize to the library's lat/lon model and never silently
//! transpose.
//!
//! Each format is behind its own cargo feature; the actual backing crate is
//! wired in when implemented (currently `todo!()`).

use crate::error::Result;
use crate::fix::Fix;

/// Parse one or more positions from a GeoJSON value (lon-lat order).
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed GeoJSON.
#[cfg(feature = "geojson")]
pub fn from_geojson(input: &str) -> Result<Vec<Fix>> {
    todo!("TODO: back with serde_json / geojson crate; positions are [lon, lat]")
}

/// Parse a coordinate/geometry from a WKT string (X Y order).
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed WKT.
#[cfg(feature = "wkt")]
pub fn from_wkt(input: &str) -> Result<Vec<Fix>> {
    todo!("TODO: back with the wkt crate")
}

/// Parse track/waypoint positions from a GPX document.
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed GPX.
#[cfg(feature = "gpx")]
pub fn from_gpx(input: &str) -> Result<Vec<Fix>> {
    todo!("TODO: back with the gpx crate")
}

/// Parse placemark positions from a KML/KMZ document.
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed KML.
#[cfg(feature = "kml")]
pub fn from_kml(input: &str) -> Result<Vec<Fix>> {
    todo!("TODO: back with the kml crate")
}
