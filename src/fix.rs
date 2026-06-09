//! Observation metadata wrapping a [`Coordinate`].
//!
//! A [`Fix`] is what the ingestion layer (EXIF, NMEA, free-text, …) emits: a
//! position plus everything known *about* that position — accuracy, time, the
//! original raw input, and how confident the parse was. Keeping this out of
//! [`Coordinate`] means pure geodetic math never drags fix metadata around.

use crate::coord::Coordinate;

/// A timestamp associated with an observation.
///
/// Uses [`std::time::SystemTime`] to stay dependency-free for now.
// TODO(impl): consider a richer `time::OffsetDateTime` behind a feature for
// GPS week/UTC handling and leap seconds.
pub type Timestamp = std::time::SystemTime;

/// A coordinate plus all known observation metadata.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fix {
    /// The observed position.
    pub coord: Coordinate,
    /// Positional accuracy, if reported.
    pub accuracy: Option<Accuracy>,
    /// Observation time, if known.
    pub timestamp: Option<Timestamp>,
    /// The raw input and how confidently it was interpreted.
    pub source: Option<RawSource>,
}

impl Fix {
    /// Wrap a bare coordinate with no metadata.
    #[must_use]
    pub fn from_coord(coord: Coordinate) -> Self {
        Self {
            coord,
            accuracy: None,
            timestamp: None,
            source: None,
        }
    }
}

/// A positional accuracy estimate.
///
/// Real fixes usually carry an error estimate; carrying and propagating it lets
/// the presentation layer avoid printing spurious precision.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Accuracy {
    /// Horizontal accuracy radius in meters (e.g. CEP or reported radius).
    pub horizontal_m: Option<f64>,
    /// Vertical accuracy in meters.
    pub vertical_m: Option<f64>,
    // TODO(impl): full covariance matrix and DOP fields for sub-meter work.
}

/// The original input a coordinate was parsed from, with parse confidence.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawSource {
    /// The verbatim input string.
    pub raw: String,
    /// How confidently `raw` was interpreted as this coordinate.
    pub confidence: Confidence,
    /// Notes about ambiguities resolved during parsing (e.g. assumed axis
    /// order, assumed datum).
    pub notes: Vec<String>,
}

/// Parse confidence on a 0.0–1.0 scale.
///
/// The free-text parser must report this so the application can decide whether
/// to prompt the user (e.g. when axis order or locale was ambiguous).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Confidence(pub f64);
