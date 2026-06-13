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
///
/// Scope is *positional* metadata only — accuracy, time, and parse provenance.
/// Motion/telemetry that some sources carry (heading from EXIF
/// `GPSImgDirection`, NMEA course and speed) is intentionally excluded to keep
/// the type lean.
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
    // Full uncertainty propagation (covariance matrix, CEP, DOP) is out of
    // scope: accuracy stays a simple scalar radius. Revisit only if sub-meter
    // error modeling proves necessary.
}

/// The original input a coordinate was parsed from, with parse confidence and
/// the ambiguities that were resolved.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawSource {
    /// The verbatim input string.
    pub raw: String,
    /// How confidently `raw` was interpreted as this coordinate.
    pub confidence: Confidence,
    /// The axis order the parser assumed, when the format leaves it ambiguous
    /// (free text, GeoJSON/WKT). `None` for formats that fix it (NMEA, `geo:`).
    pub axis_order: Option<AxisOrder>,
    /// A flagged datum ambiguity (e.g. China-EXIF possibly GCJ-02), when the
    /// source's reference system cannot be trusted as stated.
    pub datum_ambiguity: Option<DatumAmbiguity>,
    /// Free-text notes about anything else resolved during parsing.
    pub notes: Vec<String>,
}

/// Axis ordering assumed when interpreting a textual/structured coordinate.
///
/// GeoJSON and WKT are **lon-lat (X,Y)**; humans and many EPSG CRS are
/// lat-first. Parsers record which order they assumed so the application can
/// decide whether to prompt the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AxisOrder {
    /// Latitude first (human/EPSG convention).
    LatLon,
    /// Longitude first (GeoJSON/WKT X,Y convention).
    LonLat,
}

/// A flagged uncertainty about which datum a source's coordinates are in.
///
/// Some Chinese-market devices/apps embed **GCJ-02** in metadata (e.g. EXIF)
/// rather than WGS-84, plotting ~50–500 m off. Callers should resolve this
/// before trusting the datum.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every variant,
/// so adding one here is a deliberate, compile-forcing change on both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DatumAmbiguity {
    /// Coordinate is in China's bounding box; datum may be GCJ-02, not WGS-84.
    PossiblyGcj02,
}

/// Parse confidence on a 0.0–1.0 scale.
///
/// The free-text parser must report this so the application can decide whether
/// to prompt the user (e.g. when axis order or locale was ambiguous).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Confidence(f64);

impl Confidence {
    /// Construct a confidence, clamping into the valid `0.0–1.0` range.
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// The confidence value, in `0.0–1.0`.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}
