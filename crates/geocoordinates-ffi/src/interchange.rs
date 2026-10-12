//! Structured interchange formats (GeoJSON / WKT / GPX / KML) and NMEA 0183
//! sensor sentences, each parsed into [`Fix`] records.

use geocoordinates as gc;

use crate::{Fix, GeoError};

// ===========================================================================
// Structured interchange formats (GeoJSON / WKT / GPX / KML)
// ===========================================================================

/// Parse positions from a GeoJSON document (lon-lat order) — one [`Fix`] per
/// position (a line of *n* vertices yields *n* fixes).
///
/// # Errors
/// `GeoError` on malformed GeoJSON.
#[uniffi::export]
pub fn from_geojson(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_geojson(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse positions from a WKT string (X-Y order).
///
/// # Errors
/// `GeoError` on malformed WKT.
#[uniffi::export]
pub fn from_wkt(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_wkt(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse track / route / waypoint positions from a GPX document.
///
/// # Errors
/// `GeoError` on malformed GPX.
#[uniffi::export]
pub fn from_gpx(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_gpx(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse placemark positions from a KML document (lon,lat,alt order).
///
/// # Errors
/// `GeoError` on malformed KML.
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
/// # Errors
/// `GeoError` on an unrecognized/invalid sentence or a checksum mismatch.
#[uniffi::export]
pub fn from_nmea_sentence(sentence: String) -> Result<Fix, GeoError> {
    gc::parse::sensors::from_nmea_sentence(&sentence)
        .map(Into::into)
        .map_err(GeoError::from)
}
