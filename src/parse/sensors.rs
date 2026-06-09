//! Sensor/device ingestion: NMEA 0183 sentences and EXIF/XMP image metadata.

use crate::error::Result;
use crate::fix::Fix;

/// Parse a single NMEA 0183 sentence (GGA/RMC/GLL) into a [`Fix`].
///
/// NMEA uses degrees-decimal-minutes (DDM) and carries fix quality, HDOP,
/// altitude, and geoid separation — mapped onto [`Fix`] metadata.
///
/// # Errors
/// Returns [`crate::Error::Parse`] on an unrecognized/invalid sentence.
#[cfg(feature = "nmea")]
pub fn from_nmea_sentence(sentence: &str) -> Result<Fix> {
    todo!("TODO: back with an nmea parser crate")
}

/// Whether an EXIF GPS block may be ambiguous between WGS-84 and GCJ-02.
///
/// Some Chinese-market devices/apps embed **GCJ-02** in EXIF rather than
/// WGS-84, plotting photos ~50–500 m off. Callers should resolve this before
/// trusting the datum.
#[cfg(feature = "exif")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChinaExifAmbiguity {
    /// Coordinate is in China's bounding box; datum may be GCJ-02, not WGS-84.
    PossiblyGcj02,
    /// Outside China or otherwise unambiguous.
    None,
}

/// Extract a [`Fix`] from an image's EXIF/XMP GPS metadata.
///
/// Reads the GPS IFD (lat/lon rationals + refs, altitude + ref, timestamp,
/// DOP/accuracy, image direction, map datum) and flags possible China-EXIF
/// datum ambiguity.
///
/// # Errors
/// Returns [`crate::Error::Parse`] when no usable GPS metadata is present.
#[cfg(feature = "exif")]
pub fn from_exif(bytes: &[u8]) -> Result<(Fix, ChinaExifAmbiguity)> {
    todo!("TODO: back with kamadak-exif; read GPS IFD and XMP")
}
