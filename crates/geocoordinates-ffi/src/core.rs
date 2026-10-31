//! Core mirrors: the reference-system tag, heights, the canonical coordinate
//! and its approximate form, length units, and the `Fix` observation family,
//! with their constructors, validators, and the runtime conversion dispatch.

use geocoordinates as gc;

use crate::GeoError;

// ===========================================================================
// Mirror types
// ===========================================================================

/// Coordinate reference system / datum tag — mirror of [`gc::Crs`].
///
/// `gc::Crs` is exhaustive, so the `From` impls below are exhaustive matches:
/// adding a datum upstream fails this crate's build until mirrored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Crs {
    /// WGS-84 — the global GNSS reference and library default.
    Wgs84,
    /// GCJ-02 — "Mars" coordinates used by Chinese map providers.
    Gcj02,
    /// BD-09 — Baidu's additional obfuscation atop GCJ-02.
    Bd09,
    /// NAD27 — North American Datum 1927.
    Nad27,
    /// Tokyo datum (legacy Japan / Korea).
    Tokyo,
    /// Pulkovo-1942 / SK-42.
    Pulkovo42,
}

/// A height value tagged by its reference surface — mirror of [`gc::Height`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum Height {
    /// Meters above the reference ellipsoid.
    Ellipsoidal {
        /// Height in meters.
        meters: f64,
    },
    /// Meters above the geoid (mean sea level).
    Orthometric {
        /// Height in meters.
        meters: f64,
    },
}

/// The canonical coordinate — mirror of [`gc::Coordinate`].
///
/// A record can be built field by field in foreign code, so it is checked at
/// the boundary instead: every function that takes one rejects a latitude
/// outside `[-90, 90]`, a longitude outside `[-180, 180]`, a non-finite
/// component, or a non-finite height with [`GeoError::OutOfRange`] /
/// [`GeoError::InvalidValue`] before doing anything else. Build one with
/// [`coordinate_new`] to fail at construction, or call [`coordinate_validate`]
/// on a hand-built record. Every `Coordinate` this library returns is valid.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Coordinate {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
    /// Optional height (ellipsoidal or orthometric).
    pub height: Option<Height>,
    /// The reference system the position is expressed in.
    pub crs: Crs,
}

/// A length unit — mirror of [`gc::LengthUnit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LengthUnit {
    /// Meter (SI).
    Meter,
    /// Kilometer.
    Kilometer,
    /// International foot (exactly 0.3048 m).
    Foot,
    /// US survey foot (1200/3937 m).
    UsSurveyFoot,
    /// Nautical mile (1852 m).
    NauticalMile,
    /// International statute mile (1609.344 m).
    Mile,
    /// International yard (0.9144 m).
    Yard,
}

// --- Fix observation family (mirror of `gc::fix`) ---

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
    pub timestamp: Option<std::time::SystemTime>,
    /// The raw input and how confidently it was interpreted.
    pub source: Option<RawSource>,
}

// ===========================================================================
// Translation layer (mirror <-> core)
// ===========================================================================

impl From<gc::Crs> for Crs {
    fn from(c: gc::Crs) -> Self {
        match c {
            gc::Crs::Wgs84 => Crs::Wgs84,
            gc::Crs::Gcj02 => Crs::Gcj02,
            gc::Crs::Bd09 => Crs::Bd09,
            gc::Crs::Nad27 => Crs::Nad27,
            gc::Crs::Tokyo => Crs::Tokyo,
            gc::Crs::Pulkovo42 => Crs::Pulkovo42,
        }
    }
}

impl From<Crs> for gc::Crs {
    fn from(c: Crs) -> Self {
        match c {
            Crs::Wgs84 => gc::Crs::Wgs84,
            Crs::Gcj02 => gc::Crs::Gcj02,
            Crs::Bd09 => gc::Crs::Bd09,
            Crs::Nad27 => gc::Crs::Nad27,
            Crs::Tokyo => gc::Crs::Tokyo,
            Crs::Pulkovo42 => gc::Crs::Pulkovo42,
        }
    }
}

impl From<gc::Height> for Height {
    fn from(h: gc::Height) -> Self {
        match h {
            gc::Height::Ellipsoidal(meters) => Height::Ellipsoidal { meters },
            gc::Height::Orthometric(meters) => Height::Orthometric { meters },
        }
    }
}

impl From<Height> for gc::Height {
    fn from(h: Height) -> Self {
        match h {
            Height::Ellipsoidal { meters } => gc::Height::Ellipsoidal(meters),
            Height::Orthometric { meters } => gc::Height::Orthometric(meters),
        }
    }
}

impl From<gc::Coordinate> for Coordinate {
    fn from(c: gc::Coordinate) -> Self {
        Coordinate {
            lat: c.lat(),
            lon: c.lon(),
            height: c.height().map(Into::into),
            crs: c.crs().into(),
        }
    }
}

/// Every mirror record entering the core crate is validated here, so a
/// hand-built out-of-range record is rejected rather than computed on.
impl TryFrom<Coordinate> for gc::Coordinate {
    type Error = gc::Error;

    fn try_from(c: Coordinate) -> Result<Self, gc::Error> {
        let base = gc::Coordinate::new(c.lat, c.lon, c.crs.into())?;
        match c.height {
            Some(h) => base.with_height(h.into()),
            None => Ok(base),
        }
    }
}

impl From<gc::LengthUnit> for LengthUnit {
    fn from(u: gc::LengthUnit) -> Self {
        match u {
            gc::LengthUnit::Meter => LengthUnit::Meter,
            gc::LengthUnit::Kilometer => LengthUnit::Kilometer,
            gc::LengthUnit::Foot => LengthUnit::Foot,
            gc::LengthUnit::UsSurveyFoot => LengthUnit::UsSurveyFoot,
            gc::LengthUnit::NauticalMile => LengthUnit::NauticalMile,
            gc::LengthUnit::Mile => LengthUnit::Mile,
            gc::LengthUnit::Yard => LengthUnit::Yard,
        }
    }
}

impl From<LengthUnit> for gc::LengthUnit {
    fn from(u: LengthUnit) -> Self {
        match u {
            LengthUnit::Meter => gc::LengthUnit::Meter,
            LengthUnit::Kilometer => gc::LengthUnit::Kilometer,
            LengthUnit::Foot => gc::LengthUnit::Foot,
            LengthUnit::UsSurveyFoot => gc::LengthUnit::UsSurveyFoot,
            LengthUnit::NauticalMile => gc::LengthUnit::NauticalMile,
            LengthUnit::Mile => gc::LengthUnit::Mile,
            LengthUnit::Yard => gc::LengthUnit::Yard,
        }
    }
}

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

// --- Coordinate constructor ---

/// Construct a [`Coordinate`] from latitude and longitude in decimal degrees
/// (latitude first) in `crs`, with no height. Attach a height by setting the
/// record's `height` field; it is validated wherever the record is used.
///
/// # Errors
/// [`GeoError::OutOfRange`] when latitude is outside `[-90, 90]`, longitude is
/// outside `[-180, 180]`, or either is not finite.
#[uniffi::export]
pub fn coordinate_new(lat: f64, lon: f64, crs: Crs) -> Result<Coordinate, GeoError> {
    Ok(gc::Coordinate::new(lat, lon, crs.into())?.into())
}

/// Construct a [`Coordinate`] from longitude and latitude in decimal degrees
/// (**longitude first**, the GeoJSON / WKT X,Y order) in `crs`, with no
/// height — the FFI form of `Coordinate::from_lon_lat`. Use it where the
/// source is X,Y, so the swap is spelled by the function name rather than by
/// argument order at the call site.
///
/// # Errors
/// As [`coordinate_new`].
#[uniffi::export]
pub fn coordinate_from_lon_lat(lon: f64, lat: f64, crs: Crs) -> Result<Coordinate, GeoError> {
    Ok(gc::Coordinate::from_lon_lat(lon, lat, crs.into())?.into())
}

// --- Length unit conversions (`Length` is flattened to `f64` meters) ---

/// Convert `value` expressed in `unit` to meters.
#[uniffi::export]
pub fn length_from_unit(value: f64, unit: LengthUnit) -> f64 {
    gc::Length::from_unit(value, unit.into()).meters()
}

/// Convert `meters` to `unit`.
#[uniffi::export]
pub fn length_to_unit(meters: f64, unit: LengthUnit) -> f64 {
    gc::Length::from_meters(meters).to_unit(unit.into())
}

/// Meters in one of `unit` — mirror of [`gc::LengthUnit::meters_per_unit`].
#[uniffi::export]
pub fn length_unit_meters_per_unit(unit: LengthUnit) -> f64 {
    gc::LengthUnit::from(unit).meters_per_unit()
}

/// The height in meters, whichever surface it is measured from — mirror of
/// [`gc::Height::meters`] (`Length` is flattened to `f64` meters).
#[uniffi::export]
pub fn height_meters(height: Height) -> f64 {
    gc::Height::from(height).meters().meters()
}

// --- Coordinate validation ---

/// Validate a (possibly hand-built) coordinate record: latitude in
/// `[-90, 90]`, longitude in `[-180, 180]`, both finite, and any height finite.
///
/// # Errors
/// [`GeoError::OutOfRange`] when either horizontal component is invalid, or
/// [`GeoError::InvalidValue`] when the height is not finite.
#[uniffi::export]
pub fn coordinate_validate(coord: Coordinate) -> Result<(), GeoError> {
    gc::Coordinate::try_from(coord)?;
    Ok(())
}

/// Whether the coordinate is "Null Island" — both components within ~0.11 m of
/// zero, the telltale of a missing or defaulted fix.
///
/// # Errors
/// As [`coordinate_validate`], for an invalid record.
#[uniffi::export]
pub fn coordinate_is_null_island(coord: Coordinate) -> Result<bool, GeoError> {
    Ok(gc::Coordinate::try_from(coord)?.is_null_island())
}

// --- Fix observation family constructors ---

/// Wrap a bare [`Coordinate`] as a [`Fix`] with no metadata.
///
/// # Errors
/// As [`coordinate_validate`], for an invalid record.
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

// ===========================================================================
// Runtime conversion dispatch
// ===========================================================================

/// A coordinate with its error bound — the single FFI form of
/// `Approx<Coordinate>`, returned by [`convert`], by `centroid` and by every grid /
/// cell decoder (Plus Code, geohash, Maidenhead, MGRS, H3, S2).
///
/// What `max_error_m` bounds depends on the producer:
///
/// - **`centroid`** bounds the geodesic distance from the result to the farthest
///   input.
/// - **Grid / cell decoders** return the WGS-84 cell center; the bound is the
///   distance from that center to the farthest point of the cell, as each
///   decoder documents it: the maximum geodesic corner distance (Plus Code,
///   geohash, Maidenhead, S2), the half diagonal in ground meters (MGRS), or
///   the cell radius (H3).
/// - **[`convert`]** sums its legs' estimates: the iterative GCJ-02 / BD-09
///   inverses report their own bound, the forward China offsets `0.0`, and
///   each classic-datum (Helmert) leg its parameters' published accuracy
///   (`DatumTransform.accuracy_m`: about 9.27 m for NAD27 and 28.72 m for
///   Tokyo, and infinity for Pulkovo-1942, which has no published estimate).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ApproxCoordinate {
    /// The coordinate (carries its [`Crs`]: the conversion target, or WGS-84
    /// for a decoded cell center).
    pub coord: Coordinate,
    /// Estimated upper bound on the positional error, in meters (see the
    /// record docs for what each producer bounds).
    pub max_error_m: f64,
}

/// Flatten an `Approx<Coordinate>` into its [`ApproxCoordinate`] record.
pub(crate) fn approx_coordinate(approx: gc::Approx<gc::Coordinate>) -> ApproxCoordinate {
    let max_error_m = approx.max_error().meters();
    ApproxCoordinate {
        coord: approx.into_inner().into(),
        max_error_m,
    }
}

/// Convert `coord` from its own reference system to `to`, routing through the
/// WGS-84 hub (China typed conversions + classic-datum Helmert transforms).
///
/// Every pair of reference systems has a route.
///
/// # Errors
/// `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid record;
/// `GeoError::OutsideDomain` (operation `"BD-09"` or `"BD-09 inverse"`) when
/// a BD-09 leg would carry a latitude past a pole; `GeoError::InvalidValue`
/// (field `height`) when the coordinate carries an orthometric height and a
/// leg is a classic-datum Helmert shift.
#[uniffi::export]
pub fn convert(coord: Coordinate, to: Crs) -> Result<ApproxCoordinate, GeoError> {
    Ok(approx_coordinate(gc::convert::convert(
        coord.try_into()?,
        to.into(),
    )?))
}
