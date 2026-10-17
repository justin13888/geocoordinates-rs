//! Structured interchange formats (GeoJSON / WKT / GPX / KML) and NMEA 0183
//! sensor sentences, each parsed into [`Fix`] records.

use geocoordinates as gc;

use crate::{AxisOrder, Fix, GeoError};

// ===========================================================================
// Structured interchange formats (GeoJSON / WKT / GPX / KML)
// ===========================================================================

/// Parse positions from a GeoJSON document (lon-lat order) — one [`Fix`] per
/// position (a line of *n* vertices yields *n* fixes), altitude ellipsoidal.
/// A document with no positions yields an empty list.
///
/// # Errors
/// `GeoError::Parse` on malformed GeoJSON (kind `TooDeep` past 128 levels of
/// nesting), or naming the index of the first position with fewer than two
/// numbers; `GeoError::OutOfRange` for the first position out of range.
#[uniffi::export]
pub fn from_geojson(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_geojson(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse positions from a WKT string, reading X and Y in `axis_order`.
///
/// WKT carries no CRS and so no axis order: CRS84 and most GIS software write
/// longitude first, EPSG:4326 taken strictly is latitude first. Z is read as an
/// ellipsoidal height, with a note on the fix that WKT leaves it unspecified.
///
/// # Errors
/// `GeoError::Parse` on malformed WKT (kind `TooDeep` on parentheses nested
/// deeper than 128 levels); `GeoError::OutOfRange` for the first position out
/// of range.
#[uniffi::export]
pub fn from_wkt(input: String, axis_order: AxisOrder) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_wkt(&input, axis_order.into())
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse track / route / waypoint positions from a GPX document. `<ele>` is an
/// orthometric (sea-level) height, `<time>` the fix timestamp, and `<hdop>` a
/// horizontal accuracy of `HDOP × 5 m`.
///
/// # Errors
/// `GeoError::Parse` (kind `Syntax`) on malformed GPX or a `lat` / `lon` out
/// of range (the GPX reader range-checks both before any point is built), or
/// naming the index of the first point with an invalid `<hdop>`.
#[uniffi::export]
pub fn from_gpx(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_gpx(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse placemark positions from a KML document (lon,lat,alt order), altitude
/// orthometric (above sea level).
///
/// # Errors
/// `GeoError::Parse` on malformed KML (kind `TooDeep` on elements nested
/// deeper than 32 levels, `Syntax` otherwise, including a truncated document
/// with an element left unclosed or a `Point` with no coordinates);
/// `GeoError::OutOfRange` for the first position out
/// of range; `GeoError::InvalidValue`, naming the position's index, for a
/// non-finite altitude.
#[uniffi::export]
pub fn from_kml(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_kml(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

// ===========================================================================
// Sensors — NMEA 0183
// ===========================================================================

/// Parse a single NMEA 0183 sentence (GGA/RMC/GLL) into a [`Fix`]. The optional
/// `*HH` checksum is verified when present.
///
/// Only a sentence that reports a fix yields one. `RMC` fills the fix's
/// `timestamp` from its UTC date and time (two-digit year read as 1980–2079);
/// `GGA` and `GLL` carry no date, so theirs is `None`.
///
/// # Errors
/// `GeoError::Parse`, whose `kind` is `Checksum` on a malformed or mismatched
/// checksum; `Unsupported` on a sentence other than `GGA` / `RMC` / `GLL`;
/// `NoFix` on a sentence reporting no fix (`RMC`/`GLL` status `V` or mode
/// `N`, `GGA` quality `0`); and `Syntax` on any other invalid field, such as
/// a coordinate whose minutes are not two integer digits or a `GGA` altitude
/// whose unit is not `M`.
#[uniffi::export]
pub fn from_nmea_sentence(sentence: String) -> Result<Fix, GeoError> {
    gc::parse::sensors::from_nmea_sentence(&sentence)
        .map(Into::into)
        .map_err(GeoError::from)
}
