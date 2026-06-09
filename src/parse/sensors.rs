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

/// Extract a [`Fix`] from an image's EXIF/XMP GPS metadata.
///
/// Reads the GPS IFD (lat/lon rationals + refs, altitude + ref, timestamp,
/// DOP/accuracy, image direction, map datum). A possible China-EXIF datum
/// ambiguity is flagged via [`DatumAmbiguity::PossiblyGcj02`] on the returned
/// [`Fix`]'s [`RawSource`](crate::fix::RawSource).
///
/// [`DatumAmbiguity::PossiblyGcj02`]: crate::fix::DatumAmbiguity::PossiblyGcj02
///
/// # Errors
/// Returns [`crate::Error::Parse`] when no usable GPS metadata is present.
#[cfg(feature = "exif")]
pub fn from_exif(bytes: &[u8]) -> Result<Fix> {
    todo!("TODO: back with kamadak-exif; read GPS IFD and XMP; flag datum_ambiguity")
}
