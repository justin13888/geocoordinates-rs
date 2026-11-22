//! The `Fix` observation family (mirror of `gc::fix`): axis order, datum
//! ambiguity, confidence, accuracy, the raw source and the fix itself, with
//! their constructors and validators.

use geocoordinates as gc;

use crate::{Coordinate, GeoError};

// ===========================================================================
// Mirror types
// ===========================================================================

/// Axis ordering assumed when interpreting a coordinate — mirror of
/// [`gc::AxisOrder`](gc::fix::AxisOrder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AxisOrder {
    /// Latitude first (human / EPSG convention).
    LatLon,
    /// Longitude first (GeoJSON / WKT X,Y convention).
    LonLat,
}

/// A flagged uncertainty about a source's datum — mirror of
/// [`gc::DatumAmbiguity`](gc::fix::DatumAmbiguity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DatumAmbiguity {
    /// In China's bounding box; datum may be GCJ-02 rather than WGS-84.
    PossiblyGcj02,
}

/// Parse confidence on a 0.0–1.0 scale — mirror of [`gc::fix::Confidence`].
///
/// A record can be built field by field in foreign code, so it is checked at
/// the boundary instead: every function that takes one (alone or inside a
/// [`RawSource`] / [`Fix`]) rejects a NaN or out-of-range `value` with
/// [`GeoError::InvalidValue`] (field `confidence`). Build one with
/// [`confidence_new`] to fail at construction, or call
/// [`confidence_validate`] on a hand-built record. Every `Confidence` this
/// library returns is valid.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Confidence {
    /// The confidence value, in `[0.0, 1.0]`.
    pub value: f64,
}

/// What an [`Accuracy`] magnitude means statistically — mirror of
/// [`gc::fix::AccuracyKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccuracyKind {
    /// A hard maximum (e.g. a decoded grid cell's center-to-corner distance).
    Bound,
    /// A one-sigma estimate, such as NMEA / GPX `HDOP × 5 m`.
    OneSigma,
    /// A circular error probable: the radius holding the position 50% of the
    /// time.
    Cep50,
    /// A magnitude whose confidence level the source does not state (e.g. a
    /// `geo:` URI's `u=`).
    Unspecified,
}

/// A positional accuracy estimate — mirror of [`gc::fix::Accuracy`].
///
/// Checked at the boundary like [`Confidence`]: every function that takes one
/// (inside a [`Fix`]) rejects a negative, NaN or infinite magnitude, or a
/// record with neither magnitude, with [`GeoError::InvalidValue`] (field
/// `horizontal`, `vertical` or `accuracy`). Build one with [`accuracy_new`],
/// or call [`accuracy_validate`] on a hand-built record.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Accuracy {
    /// What the magnitudes mean statistically.
    pub kind: AccuracyKind,
    /// Horizontal accuracy radius in meters, if reported.
    pub horizontal_m: Option<f64>,
    /// Vertical accuracy in meters, if reported.
    pub vertical_m: Option<f64>,
}

/// The original input a coordinate was parsed from — mirror of [`gc::fix::RawSource`].
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RawSource {
    /// The input this coordinate came from: a single-coordinate input
    /// verbatim, or for a structured document (GeoJSON, WKT, GPX, KML) this
    /// position's own fragment in the document's syntax.
    pub raw: String,
    /// How confidently `raw` was interpreted as this coordinate.
    pub confidence: Confidence,
    /// The order the source listed latitude and longitude in; absent when the
    /// axes are labelled (hemisphere letters, GPX attributes) or there is no
    /// axis pair (a grid reference).
    pub axis_order: Option<AxisOrder>,
    /// A flagged datum ambiguity, when the source's reference system is suspect.
    pub datum_ambiguity: Option<DatumAmbiguity>,
    /// The geoid separation N the source reported (NMEA `GGA`), in meters:
    /// the ellipsoidal height is the orthometric height plus N.
    pub geoid_separation_m: Option<f64>,
    /// Free-text notes about anything else resolved during parsing.
    pub notes: Vec<String>,
}

/// A coordinate plus all known observation metadata — mirror of [`gc::fix::Fix`].
///
/// `timestamp` crosses natively via UniFFI's builtin `Timestamp`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Fix {
    /// The observed position.
    pub coord: Coordinate,
    /// Positional accuracy, if reported.
    pub accuracy: Option<Accuracy>,
    /// Observation time, if known.
    ///
    /// An instant in the last second before 1970-01-01T00:00:00Z (strictly
    /// between -1 s and 0 s) crosses the boundary with its sign lost, as its
    /// mirror image after the epoch: UniFFI's `Timestamp` keeps the sign on the
    /// whole seconds, which are zero there. Every other instant, before or after
    /// the epoch, crosses unchanged. See the FFI README's "Timestamps" section.
    pub timestamp: Option<std::time::SystemTime>,
    /// The raw input and how confidently it was interpreted.
    pub source: Option<RawSource>,
}

// ===========================================================================
// Translation layer (mirror <-> core)
// ===========================================================================

impl From<gc::fix::AxisOrder> for AxisOrder {
    fn from(a: gc::fix::AxisOrder) -> Self {
        match a {
            gc::fix::AxisOrder::LatLon => AxisOrder::LatLon,
            gc::fix::AxisOrder::LonLat => AxisOrder::LonLat,
        }
    }
}

impl From<AxisOrder> for gc::fix::AxisOrder {
    fn from(a: AxisOrder) -> Self {
        match a {
            AxisOrder::LatLon => gc::fix::AxisOrder::LatLon,
            AxisOrder::LonLat => gc::fix::AxisOrder::LonLat,
        }
    }
}

impl From<gc::fix::DatumAmbiguity> for DatumAmbiguity {
    fn from(d: gc::fix::DatumAmbiguity) -> Self {
        match d {
            gc::fix::DatumAmbiguity::PossiblyGcj02 => DatumAmbiguity::PossiblyGcj02,
        }
    }
}

impl From<DatumAmbiguity> for gc::fix::DatumAmbiguity {
    fn from(d: DatumAmbiguity) -> Self {
        match d {
            DatumAmbiguity::PossiblyGcj02 => gc::fix::DatumAmbiguity::PossiblyGcj02,
        }
    }
}

impl From<gc::fix::Confidence> for Confidence {
    fn from(c: gc::fix::Confidence) -> Self {
        Confidence { value: c.value() }
    }
}

impl TryFrom<Confidence> for gc::fix::Confidence {
    type Error = gc::Error;

    fn try_from(c: Confidence) -> Result<Self, gc::Error> {
        gc::fix::Confidence::new(c.value)
    }
}

impl From<gc::fix::AccuracyKind> for AccuracyKind {
    fn from(k: gc::fix::AccuracyKind) -> Self {
        match k {
            gc::fix::AccuracyKind::Bound => AccuracyKind::Bound,
            gc::fix::AccuracyKind::OneSigma => AccuracyKind::OneSigma,
            gc::fix::AccuracyKind::Cep50 => AccuracyKind::Cep50,
            gc::fix::AccuracyKind::Unspecified => AccuracyKind::Unspecified,
        }
    }
}

impl From<AccuracyKind> for gc::fix::AccuracyKind {
    fn from(k: AccuracyKind) -> Self {
        match k {
            AccuracyKind::Bound => gc::fix::AccuracyKind::Bound,
            AccuracyKind::OneSigma => gc::fix::AccuracyKind::OneSigma,
            AccuracyKind::Cep50 => gc::fix::AccuracyKind::Cep50,
            AccuracyKind::Unspecified => gc::fix::AccuracyKind::Unspecified,
        }
    }
}

impl From<gc::fix::Accuracy> for Accuracy {
    fn from(a: gc::fix::Accuracy) -> Self {
        Accuracy {
            kind: a.kind().into(),
            horizontal_m: a.horizontal().map(|h| h.meters()),
            vertical_m: a.vertical().map(|v| v.meters()),
        }
    }
}

impl TryFrom<Accuracy> for gc::fix::Accuracy {
    type Error = gc::Error;

    fn try_from(a: Accuracy) -> Result<Self, gc::Error> {
        gc::fix::Accuracy::new(
            a.kind.into(),
            a.horizontal_m.map(gc::Length::from_meters),
            a.vertical_m.map(gc::Length::from_meters),
        )
    }
}

impl From<gc::fix::RawSource> for RawSource {
    fn from(s: gc::fix::RawSource) -> Self {
        RawSource {
            raw: s.raw,
            confidence: s.confidence.into(),
            axis_order: s.axis_order.map(Into::into),
            datum_ambiguity: s.datum_ambiguity.map(Into::into),
            geoid_separation_m: s.geoid_separation.map(|n| n.meters()),
            notes: s.notes,
        }
    }
}

/// Validates the embedded [`Confidence`].
impl TryFrom<RawSource> for gc::fix::RawSource {
    type Error = gc::Error;

    fn try_from(s: RawSource) -> Result<Self, gc::Error> {
        let mut source = gc::fix::RawSource::new(s.raw, s.confidence.try_into()?);
        source.axis_order = s.axis_order.map(Into::into);
        source.datum_ambiguity = s.datum_ambiguity.map(Into::into);
        source.geoid_separation = s.geoid_separation_m.map(gc::Length::from_meters);
        source.notes = s.notes;
        Ok(source)
    }
}

impl From<gc::fix::Fix> for Fix {
    fn from(f: gc::fix::Fix) -> Self {
        Fix {
            coord: f.coord.into(),
            accuracy: f.accuracy.map(Into::into),
            timestamp: f.timestamp,
            source: f.source.map(Into::into),
        }
    }
}

/// Validates the embedded [`Coordinate`], [`Accuracy`] and [`Confidence`], as
/// their own `TryFrom` impls do.
impl TryFrom<Fix> for gc::fix::Fix {
    type Error = gc::Error;

    fn try_from(f: Fix) -> Result<Self, gc::Error> {
        let mut fix = gc::fix::Fix::from_coord(f.coord.try_into()?);
        fix.accuracy = f.accuracy.map(TryInto::try_into).transpose()?;
        fix.timestamp = f.timestamp;
        fix.source = f.source.map(TryInto::try_into).transpose()?;
        Ok(fix)
    }
}

// ===========================================================================
// Exported API
// ===========================================================================

/// Wrap a bare [`Coordinate`] as a [`Fix`] with no metadata.
///
/// # Errors
/// As [`coordinate_validate`](crate::coordinate_validate), for an invalid record.
#[uniffi::export]
pub fn fix_from_coord(coord: Coordinate) -> Result<Fix, GeoError> {
    Ok(gc::fix::Fix::from_coord(coord.try_into()?).into())
}

/// Construct a [`Confidence`] from a value in `[0.0, 1.0]`.
///
/// # Errors
/// [`GeoError::InvalidValue`] (field `confidence`) when `value` is NaN or
/// outside `[0.0, 1.0]`.
#[uniffi::export]
pub fn confidence_new(value: f64) -> Result<Confidence, GeoError> {
    Ok(gc::fix::Confidence::new(value)?.into())
}

/// Validate a (possibly hand-built) [`Confidence`] record.
///
/// # Errors
/// As [`confidence_new`].
#[uniffi::export]
pub fn confidence_validate(confidence: Confidence) -> Result<(), GeoError> {
    gc::fix::Confidence::try_from(confidence)?;
    Ok(())
}

/// Construct an [`Accuracy`] of `kind` from its horizontal and vertical
/// magnitudes in meters; either may be absent, but not both.
///
/// # Errors
/// [`GeoError::InvalidValue`] with field `horizontal` / `vertical` when that
/// magnitude is negative, NaN or infinite, or with field `accuracy` when both
/// are absent.
#[uniffi::export]
pub fn accuracy_new(
    kind: AccuracyKind,
    horizontal_m: Option<f64>,
    vertical_m: Option<f64>,
) -> Result<Accuracy, GeoError> {
    let accuracy = gc::fix::Accuracy::new(
        kind.into(),
        horizontal_m.map(gc::Length::from_meters),
        vertical_m.map(gc::Length::from_meters),
    )?;
    Ok(accuracy.into())
}

/// Validate a (possibly hand-built) [`Accuracy`] record.
///
/// # Errors
/// As [`accuracy_new`].
#[uniffi::export]
pub fn accuracy_validate(accuracy: Accuracy) -> Result<(), GeoError> {
    gc::fix::Accuracy::try_from(accuracy)?;
    Ok(())
}

/// Construct a [`RawSource`] recording `raw` as interpreted with `confidence`,
/// with no axis order, datum ambiguity, geoid separation or notes; set those
/// fields afterwards where the source supplies them.
///
/// # Errors
/// As [`confidence_validate`], for an invalid `confidence`.
#[uniffi::export]
pub fn raw_source_new(raw: String, confidence: Confidence) -> Result<RawSource, GeoError> {
    Ok(gc::fix::RawSource::new(raw, confidence.try_into()?).into())
}
