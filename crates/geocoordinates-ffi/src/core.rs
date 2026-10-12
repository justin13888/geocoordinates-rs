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
/// Unvalidated until used: constructing one does not check that `lat` is in
/// `[-90, 90]`, `lon` in `[-180, 180]`, or a height finite. Call
/// [`coordinate_validate`] to reject such values up front.
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

/// Parse confidence on a 0.0–1.0 scale — mirror of [`gc::Confidence`].
///
/// Unvalidated until used: a record built in foreign code keeps whatever
/// `value` it was given. When it is passed into a function, the value is
/// clamped into `[0.0, 1.0]` (NaN becomes `0.0`), as `gc::Confidence::new`
/// does. Every `Confidence` this library returns is already in range.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Confidence {
    /// The confidence value. In `[0.0, 1.0]` when returned by this library;
    /// clamped into that range when passed into a function.
    pub value: f64,
}

/// A positional accuracy estimate — mirror of [`gc::Accuracy`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Accuracy {
    /// Horizontal accuracy radius in meters, if reported.
    pub horizontal_m: Option<f64>,
    /// Vertical accuracy in meters, if reported.
    pub vertical_m: Option<f64>,
}

/// The original input a coordinate was parsed from — mirror of [`gc::RawSource`].
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RawSource {
    /// The verbatim input string.
    pub raw: String,
    /// How confidently `raw` was interpreted as this coordinate.
    pub confidence: Confidence,
    /// The axis order the parser assumed, when the format leaves it ambiguous.
    pub axis_order: Option<AxisOrder>,
    /// A flagged datum ambiguity, when the source's reference system is suspect.
    pub datum_ambiguity: Option<DatumAmbiguity>,
    /// Free-text notes about anything else resolved during parsing.
    pub notes: Vec<String>,
}

/// A coordinate plus all known observation metadata — mirror of [`gc::Fix`].
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
            lat: c.lat,
            lon: c.lon,
            height: c.height.map(Into::into),
            crs: c.crs.into(),
        }
    }
}

impl From<Coordinate> for gc::Coordinate {
    fn from(c: Coordinate) -> Self {
        let base = gc::Coordinate::new(c.lat, c.lon, c.crs.into());
        match c.height {
            Some(h) => base.with_height(h.into()),
            None => base,
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

impl From<gc::Confidence> for Confidence {
    fn from(c: gc::Confidence) -> Self {
        Confidence { value: c.value() }
    }
}

impl From<Confidence> for gc::Confidence {
    fn from(c: Confidence) -> Self {
        gc::Confidence::new(c.value)
    }
}

impl From<gc::Accuracy> for Accuracy {
    fn from(a: gc::Accuracy) -> Self {
        Accuracy {
            horizontal_m: a.horizontal_m,
            vertical_m: a.vertical_m,
        }
    }
}

impl From<Accuracy> for gc::Accuracy {
    fn from(a: Accuracy) -> Self {
        gc::Accuracy {
            horizontal_m: a.horizontal_m,
            vertical_m: a.vertical_m,
        }
    }
}

impl From<gc::RawSource> for RawSource {
    fn from(s: gc::RawSource) -> Self {
        RawSource {
            raw: s.raw,
            confidence: s.confidence.into(),
            axis_order: s.axis_order.map(Into::into),
            datum_ambiguity: s.datum_ambiguity.map(Into::into),
            notes: s.notes,
        }
    }
}

impl From<RawSource> for gc::RawSource {
    fn from(s: RawSource) -> Self {
        gc::RawSource {
            raw: s.raw,
            confidence: s.confidence.into(),
            axis_order: s.axis_order.map(Into::into),
            datum_ambiguity: s.datum_ambiguity.map(Into::into),
            notes: s.notes,
        }
    }
}

impl From<gc::Fix> for Fix {
    fn from(f: gc::Fix) -> Self {
        Fix {
            coord: f.coord.into(),
            accuracy: f.accuracy.map(Into::into),
            timestamp: f.timestamp,
            source: f.source.map(Into::into),
        }
    }
}

impl From<Fix> for gc::Fix {
    fn from(f: Fix) -> Self {
        gc::Fix {
            coord: f.coord.into(),
            accuracy: f.accuracy.map(Into::into),
            timestamp: f.timestamp,
            source: f.source.map(Into::into),
        }
    }
}

// ===========================================================================
// Exported API
// ===========================================================================

// --- Coordinate constructors ---

/// Construct a WGS-84 [`Coordinate`] from latitude/longitude in degrees.
#[uniffi::export]
pub fn coordinate_wgs84(lat: f64, lon: f64) -> Coordinate {
    gc::Coordinate::wgs84(lat, lon).into()
}

/// Construct a GCJ-02 [`Coordinate`] from latitude/longitude in degrees.
#[uniffi::export]
pub fn coordinate_gcj02(lat: f64, lon: f64) -> Coordinate {
    gc::Coordinate::gcj02(lat, lon).into()
}

/// Construct a BD-09 [`Coordinate`] from latitude/longitude in degrees.
#[uniffi::export]
pub fn coordinate_bd09(lat: f64, lon: f64) -> Coordinate {
    gc::Coordinate::bd09(lat, lon).into()
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

// --- Coordinate validation ---

/// Validate that the coordinate's latitude/longitude are in range.
///
/// # Errors
/// Returns [`GeoError::OutOfRange`] when either component is out of range.
#[uniffi::export]
pub fn coordinate_validate(coord: Coordinate) -> Result<(), GeoError> {
    gc::Coordinate::from(coord)
        .validate()
        .map_err(GeoError::from)
}

/// Whether the coordinate is "Null Island" — both components within ~0.11 m of
/// zero, the telltale of a missing or defaulted fix.
#[uniffi::export]
pub fn coordinate_is_null_island(coord: Coordinate) -> bool {
    gc::Coordinate::from(coord).is_null_island()
}

// --- Fix observation family constructors ---

/// Wrap a bare [`Coordinate`] as a [`Fix`] with no metadata.
#[uniffi::export]
pub fn fix_from_coord(coord: Coordinate) -> Fix {
    gc::Fix::from_coord(coord.into()).into()
}

/// Construct a [`Confidence`], clamping `value` into `[0.0, 1.0]`.
#[uniffi::export]
pub fn confidence_new(value: f64) -> Confidence {
    gc::Confidence::new(value).into()
}

// ===========================================================================
// Runtime conversion dispatch
// ===========================================================================

/// A coordinate with its error bound — the single FFI form of
/// `Approx<Coordinate>`, returned by [`convert`] and by every grid / cell
/// decoder (Plus Code, geohash, Maidenhead, MGRS, H3, S2).
///
/// What `max_error_m` bounds depends on the producer:
///
/// - **Grid / cell decoders** return the WGS-84 cell center; the bound is the
///   distance from that center to the farthest point of the cell, as each
///   decoder documents it: the half-diagonal (Plus Code, geohash,
///   Maidenhead), the half-square (MGRS), the cell radius (H3), or the
///   maximum corner distance (S2).
/// - **[`convert`]** bounds only the error of the iterative GCJ-02 / BD-09
///   inverses; every other leg contributes `0.0`. For classic-datum (Helmert)
///   legs that `0.0` is not a claim of exactness: the catalogued mean shifts
///   are good to only several meters, which the bound leaves out.
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
    let max_error_m = approx.max_error_m();
    ApproxCoordinate {
        coord: approx.into_inner().into(),
        max_error_m,
    }
}

/// Convert `coord` from its own reference system to `to`, routing through the
/// WGS-84 hub (China typed conversions + classic-datum Helmert transforms).
///
/// # Errors
/// `GeoError` when no route is known between the two systems.
#[uniffi::export]
pub fn convert(coord: Coordinate, to: Crs) -> Result<ApproxCoordinate, GeoError> {
    gc::convert::convert(coord.into(), to.into())
        .map(approx_coordinate)
        .map_err(GeoError::from)
}

/// Whether a conversion route exists between two reference systems.
#[uniffi::export]
pub fn can_convert(from: Crs, to: Crs) -> bool {
    gc::convert::can_convert(from.into(), to.into())
}
