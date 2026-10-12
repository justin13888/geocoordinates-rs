//! Observation metadata wrapping a [`Coordinate`].
//!
//! A [`Fix`] is what the ingestion layer (EXIF, NMEA, free-text, …) emits: a
//! position plus everything known *about* that position — accuracy, time, the
//! original raw input, and how confident the parse was. Keeping this out of
//! [`Coordinate`] means pure geodetic math never drags fix metadata around.

use std::time::SystemTime;

use crate::coord::Coordinate;

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
    /// Observation time, if known, as a [`SystemTime`] (UTC, no leap-second or
    /// GPS-week handling). Filled from GPX `<time>` and from an NMEA `RMC`
    /// sentence's date and time; `None` for sources that carry no full date.
    pub timestamp: Option<SystemTime>,
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
    /// The input this coordinate came from. For a single-coordinate input
    /// (free text, a `geo:` URI, a grid reference, an NMEA sentence) it is
    /// that input verbatim. For a structured document (GeoJSON, WKT, GPX, KML)
    /// it is this position's own fragment, re-rendered in the document's
    /// syntax — never the whole document.
    pub raw: String,
    /// How confidently `raw` was interpreted as this coordinate.
    pub confidence: Confidence,
    /// The order the source listed latitude and longitude in: `LatLon` for
    /// `geo:` URIs and NMEA, `LonLat` for GeoJSON and KML, the caller's choice
    /// for WKT, and for unlabelled free text the order the parser assumed.
    /// `None` when the axes are labelled rather than positional (a hemisphere
    /// letter in free text, GPX `lat`/`lon` attributes) or the source has no
    /// axis pair at all (a grid reference).
    pub axis_order: Option<AxisOrder>,
    /// A flagged datum ambiguity (e.g. China-EXIF possibly GCJ-02), when the
    /// source's reference system cannot be trusted as stated.
    pub datum_ambiguity: Option<DatumAmbiguity>,
    /// Free-text notes about anything else resolved during parsing.
    pub notes: Vec<String>,
}

/// The order latitude and longitude are listed in a textual/structured
/// coordinate.
///
/// GeoJSON and KML are **lon-lat (X,Y)**; WKT defines no order, so its parser
/// takes one from the caller; humans, `geo:` URIs, NMEA, and many EPSG CRS are
/// lat-first. Parsers record the listing order on [`RawSource::axis_order`] so
/// the application can decide whether to prompt the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AxisOrder {
    /// Latitude first (human/EPSG convention).
    LatLon,
    /// Longitude first (GeoJSON/KML X,Y convention, and most WKT).
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
///
/// Always in range: [`new`](Self::new) clamps, and under the `serde` feature
/// deserialization goes through the same clamp. The JSON shape is a bare
/// number.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "f64", into = "f64"))]
pub struct Confidence(f64);

/// Thin sugar over [`Confidence::new`], clamping into `0.0–1.0`.
impl From<f64> for Confidence {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

/// Thin sugar over [`Confidence::value`].
impl From<Confidence> for f64 {
    fn from(confidence: Confidence) -> Self {
        confidence.value()
    }
}

impl Confidence {
    /// Construct a confidence, clamping into the valid `0.0–1.0` range.
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self(if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, 1.0)
        })
    }

    /// The confidence value, in `0.0–1.0`.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Confidence;

    #[test]
    fn confidence_is_always_in_range() {
        assert_eq!(Confidence::new(f64::NAN).value(), 0.0);
        assert_eq!(Confidence::new(f64::NEG_INFINITY).value(), 0.0);
        assert_eq!(Confidence::new(f64::INFINITY).value(), 1.0);
        assert_eq!(Confidence::from(0.25).value(), 0.25);
        assert_eq!(f64::from(Confidence::new(2.0)), 1.0);
    }

    /// Deserialization clamps like `new`, and the wire shape is a bare number.
    #[cfg(feature = "serde")]
    #[test]
    fn confidence_serde_clamps_and_keeps_its_shape() {
        let high: Confidence = serde_json::from_str("1.5").unwrap();
        assert_eq!(high.value(), 1.0);
        let low: Confidence = serde_json::from_str("-0.5").unwrap();
        assert_eq!(low.value(), 0.0);
        assert_eq!(serde_json::to_string(&Confidence::new(0.5)).unwrap(), "0.5");
    }
}
