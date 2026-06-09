//! Ingestion: turning real-world input into a [`Fix`].
//!
//! - [`text`] — tolerant free-text / DMS / DDM parsing (always available).
//! - [`from_geo_uri`] — `geo:` URIs per RFC 5870 (always available).
//! - [`interchange`] — GeoJSON, WKT, GPX, KML (each behind a cargo feature).
//! - [`sensors`] — NMEA 0183 and EXIF/XMP image metadata (feature-gated).
//!
//! ## Axis order is first-class
//!
//! GeoJSON and WKT are **lon-lat (X,Y)**; humans and many EPSG CRS are
//! lat-first. Every parser records the [`AxisOrder`] it assumed and reports a
//! confidence so the application can decide whether to prompt the user.
//!
//! ## Out of scope
//!
//! Map-service URLs (Google `@lat,lon`, OSM, Apple), WKB, and GML are
//! intentionally **not** parsed here — the structured interchange surface is
//! limited to the text formats above plus `geo:` URIs.

pub mod text;

#[cfg(any(feature = "geojson", feature = "wkt", feature = "gpx", feature = "kml"))]
pub mod interchange;

#[cfg(any(feature = "nmea", feature = "exif"))]
pub mod sensors;

use crate::error::Result;
use crate::fix::Fix;

/// Axis ordering of a textual/structured coordinate. Re-exported from
/// [`fix`](crate::fix), where it lives so parsers can record it on a [`Fix`]'s
/// [`RawSource`](crate::fix::RawSource).
pub use crate::fix::AxisOrder;

/// Best-effort parse of a single coordinate from arbitrary input.
///
/// Recognizes, in order: a `geo:` URI (see [`from_geo_uri`]); a UTM / MGRS /
/// Plus Code / geohash token (via the [`grids`](crate::grids) decoders); then
/// falls back to free-text DD/DMS/DDM heuristics (see [`text`]). The returned
/// [`Fix`] records parse confidence and the assumed [`AxisOrder`] in its
/// [`RawSource`](crate::fix::RawSource).
///
/// # Errors
/// Returns [`crate::Error::Parse`] when no interpretation is found.
pub fn parse_coordinate(input: &str) -> Result<Fix> {
    todo!(
        "detect geo: URI, then UTM/MGRS/PlusCode/geohash tokens via grids \
         decoders, else text::parse with range/locale heuristics"
    )
}

/// Parse a `geo:` URI per [RFC 5870](https://www.rfc-editor.org/rfc/rfc5870),
/// e.g. `geo:13.4125,103.8667` or `geo:48.2,16.3,183;crs=wgs84;u=40`.
///
/// Latitude comes first (the RFC fixes the axis order), an optional third
/// number is the altitude in meters, and the `crs`/`u` parameters set the
/// reference system and the horizontal accuracy (meters) on the returned
/// [`Fix`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input is not a well-formed `geo:`
/// URI.
pub fn from_geo_uri(input: &str) -> Result<Fix> {
    todo!("strip 'geo:' scheme; parse lat,lon[,alt]; apply crs= and u= params")
}
