//! Sensor/device ingestion: NMEA 0183 sentences.
//!
//! EXIF/XMP image GPS metadata is **out of scope** — it is handled by a
//! separate library that consumes this crate's primitives (the angle
//! conversions for GPS rationals, [`Fix`] with its
//! [`RawSource`](crate::fix::RawSource), and
//! [`DatumAmbiguity::PossiblyGcj02`](crate::fix::DatumAmbiguity::PossiblyGcj02)
//! for China-EXIF datum ambiguity).

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
