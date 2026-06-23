//! UniFFI bindings for [`geocoordinates`](https://docs.rs/geocoordinates).
//!
//! This crate exposes the Rust API with **full capability parity** to Python,
//! Kotlin, Swift, and TypeScript/WASM (and Java, via the Kotlin/JVM artifact):
//! every public capability of the released `geocoordinates` surface is
//! reachable here, and the mirror gates each release (see `ROADMAP.md`).
//! Several Rust idioms cannot cross an FFI boundary, so they are re-expressed
//! as flat, language-neutral records and free functions:
//!
//! | Rust idiom | FFI form here |
//! |---|---|
//! | `Approx<T>` (generic wrapper) | flat records [`ApproxWgs84`] / [`ApproxGcj02`] carrying `max_error_m` |
//! | `LatLon` trait / `&impl LatLon` | functions take the concrete [`Coordinate`] record |
//! | `Length` + operator overloads | distance returned as `f64` meters ([`haversine_distance_m`]) |
//! | `From` / `TryFrom` conversions | named free functions (`wgs84_to_gcj02`, …) |
//!
//! Exactness stays visible across the boundary: exact forward conversions
//! return a bare datum record, while approximate inverses return an `Approx*`
//! record and keep the `_fast` / `_refined` suffix.
//!
//! The angle encodings (`Dd` / `Dms` / `Ddm`), their conversions and the
//! normalization helpers, the `Length` / `LengthUnit` unit helpers, coordinate
//! validation, and the `Fix` observation family (`Fix` / `Accuracy` /
//! `RawSource` / `Confidence`, with `SystemTime` mapped natively to UniFFI's
//! builtin `Timestamp`) all cross the boundary as flat records / enums and free
//! functions.

use geocoordinates as gc;

uniffi::setup_scaffolding!();

// ===========================================================================
// Mirror types
//
// Hand-written mirrors (rather than `#[uniffi::remote]`) so the FFI surface is
// flattened into language-neutral forms — and because `Approx<T>` must be
// flattened anyway.
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

/// A WGS-84 position (real GPS / OpenStreetMap), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Wgs84 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A GCJ-02 position (Google China, AutoNavi/高德, Tencent), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Gcj02 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A BD-09 position (Baidu Maps only), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Bd09 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A Baidu Web Mercator position, in projected meters.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BaiduMercator {
    /// Easting (longitude axis), in meters.
    pub x: f64,
    /// Northing (latitude axis), in meters.
    pub y: f64,
}

/// An **approximate** WGS-84 position — the FFI-flattened form of
/// `Approx<Wgs84>`. `max_error_m` is the estimated upper bound on positional
/// error, in meters.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ApproxWgs84 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
    /// Estimated maximum positional error, in meters.
    pub max_error_m: f64,
}

/// An **approximate** GCJ-02 position — the FFI-flattened form of
/// `Approx<Gcj02>`. `max_error_m` is the estimated upper bound on positional
/// error, in meters.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ApproxGcj02 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
    /// Estimated maximum positional error, in meters.
    pub max_error_m: f64,
}

/// Errors surfaced across the FFI boundary — a flattened mirror of [`gc::Error`].
///
/// Datum-bearing variants carry their reference systems as their canonical
/// short names (e.g. `"BD-09"`); everything else collapses into [`GeoError::Other`].
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum GeoError {
    /// A coordinate carried the wrong reference system for the requested operation.
    #[error("crs mismatch: expected {expected}, found {found}")]
    CrsMismatch {
        /// The reference system that was required.
        expected: String,
        /// The reference system the coordinate actually carried.
        found: String,
    },
    /// A latitude/longitude fell outside its valid domain.
    #[error("coordinate out of valid range: lat={lat}, lon={lon}")]
    OutOfRange {
        /// Offending latitude in degrees.
        lat: f64,
        /// Offending longitude in degrees.
        lon: f64,
    },
    /// Any other library error, with its message preserved.
    // NB: the field is `detail`, not `message`: UniFFI's Kotlin backend emits an
    // `override val message` getter on every error variant, which would collide
    // with a field named `message` and break Kotlin/Java compilation.
    #[error("{detail}")]
    Other {
        /// The underlying error's display message.
        detail: String,
    },
}

// --- Angle encodings (mirror of `gc::angle`) ---

/// North/South or East/West hemisphere sign — mirror of [`gc::angle::Hemisphere`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Hemisphere {
    /// North (latitude, positive).
    North,
    /// South (latitude, negative).
    South,
    /// East (longitude, positive).
    East,
    /// West (longitude, negative).
    West,
}

/// Which axis an angle represents — mirror of [`gc::angle::Axis`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Axis {
    /// Latitude (selects N/S).
    Latitude,
    /// Longitude (selects E/W).
    Longitude,
}

/// Decimal degrees — mirror of the `gc::angle::Dd` newtype.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Dd {
    /// The signed angle in decimal degrees.
    pub value: f64,
}

/// Degrees / minutes / seconds — mirror of [`gc::angle::Dms`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Dms {
    /// Whole degrees (non-negative; sign carried by `hemisphere`).
    pub degrees: u16,
    /// Whole minutes `[0, 60)`.
    pub minutes: u8,
    /// Seconds `[0, 60)`.
    pub seconds: f64,
    /// Hemisphere providing the sign.
    pub hemisphere: Hemisphere,
}

/// Degrees / decimal minutes — mirror of [`gc::angle::Ddm`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ddm {
    /// Whole degrees (non-negative; sign carried by `hemisphere`).
    pub degrees: u16,
    /// Decimal minutes `[0, 60)`.
    pub minutes: f64,
    /// Hemisphere providing the sign.
    pub hemisphere: Hemisphere,
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
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Confidence {
    /// The confidence value, clamped into `[0.0, 1.0]`.
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

impl From<gc::Wgs84> for Wgs84 {
    fn from(p: gc::Wgs84) -> Self {
        Wgs84 {
            lat: p.lat,
            lon: p.lon,
        }
    }
}

impl From<Wgs84> for gc::Wgs84 {
    fn from(p: Wgs84) -> Self {
        gc::Wgs84::new(p.lat, p.lon)
    }
}

impl From<gc::Gcj02> for Gcj02 {
    fn from(p: gc::Gcj02) -> Self {
        Gcj02 {
            lat: p.lat,
            lon: p.lon,
        }
    }
}

impl From<Gcj02> for gc::Gcj02 {
    fn from(p: Gcj02) -> Self {
        gc::Gcj02::new(p.lat, p.lon)
    }
}

impl From<gc::Bd09> for Bd09 {
    fn from(p: gc::Bd09) -> Self {
        Bd09 {
            lat: p.lat,
            lon: p.lon,
        }
    }
}

impl From<Bd09> for gc::Bd09 {
    fn from(p: Bd09) -> Self {
        gc::Bd09::new(p.lat, p.lon)
    }
}

impl From<gc::BaiduMercator> for BaiduMercator {
    fn from(m: gc::BaiduMercator) -> Self {
        BaiduMercator { x: m.x, y: m.y }
    }
}

impl From<BaiduMercator> for gc::BaiduMercator {
    fn from(m: BaiduMercator) -> Self {
        gc::BaiduMercator::new(m.x, m.y)
    }
}

impl From<gc::Approx<gc::Wgs84>> for ApproxWgs84 {
    fn from(a: gc::Approx<gc::Wgs84>) -> Self {
        let max_error_m = a.max_error_m();
        let w = a.into_inner();
        ApproxWgs84 {
            lat: w.lat,
            lon: w.lon,
            max_error_m,
        }
    }
}

impl From<gc::Approx<gc::Gcj02>> for ApproxGcj02 {
    fn from(a: gc::Approx<gc::Gcj02>) -> Self {
        let max_error_m = a.max_error_m();
        let g = a.into_inner();
        ApproxGcj02 {
            lat: g.lat,
            lon: g.lon,
            max_error_m,
        }
    }
}

impl From<gc::Error> for GeoError {
    fn from(e: gc::Error) -> Self {
        match e {
            gc::Error::CrsMismatch { expected, found } => GeoError::CrsMismatch {
                expected: expected.to_string(),
                found: found.to_string(),
            },
            gc::Error::OutOfRange { lat, lon } => GeoError::OutOfRange { lat, lon },
            other => GeoError::Other {
                detail: other.to_string(),
            },
        }
    }
}

impl From<gc::angle::Hemisphere> for Hemisphere {
    fn from(h: gc::angle::Hemisphere) -> Self {
        match h {
            gc::angle::Hemisphere::North => Hemisphere::North,
            gc::angle::Hemisphere::South => Hemisphere::South,
            gc::angle::Hemisphere::East => Hemisphere::East,
            gc::angle::Hemisphere::West => Hemisphere::West,
        }
    }
}

impl From<Hemisphere> for gc::angle::Hemisphere {
    fn from(h: Hemisphere) -> Self {
        match h {
            Hemisphere::North => gc::angle::Hemisphere::North,
            Hemisphere::South => gc::angle::Hemisphere::South,
            Hemisphere::East => gc::angle::Hemisphere::East,
            Hemisphere::West => gc::angle::Hemisphere::West,
        }
    }
}

impl From<gc::angle::Axis> for Axis {
    fn from(a: gc::angle::Axis) -> Self {
        match a {
            gc::angle::Axis::Latitude => Axis::Latitude,
            gc::angle::Axis::Longitude => Axis::Longitude,
        }
    }
}

impl From<Axis> for gc::angle::Axis {
    fn from(a: Axis) -> Self {
        match a {
            Axis::Latitude => gc::angle::Axis::Latitude,
            Axis::Longitude => gc::angle::Axis::Longitude,
        }
    }
}

impl From<gc::angle::Dd> for Dd {
    fn from(d: gc::angle::Dd) -> Self {
        Dd { value: d.0 }
    }
}

impl From<Dd> for gc::angle::Dd {
    fn from(d: Dd) -> Self {
        gc::angle::Dd(d.value)
    }
}

impl From<gc::angle::Dms> for Dms {
    fn from(d: gc::angle::Dms) -> Self {
        Dms {
            degrees: d.degrees,
            minutes: d.minutes,
            seconds: d.seconds,
            hemisphere: d.hemisphere.into(),
        }
    }
}

impl From<Dms> for gc::angle::Dms {
    fn from(d: Dms) -> Self {
        gc::angle::Dms {
            degrees: d.degrees,
            minutes: d.minutes,
            seconds: d.seconds,
            hemisphere: d.hemisphere.into(),
        }
    }
}

impl From<gc::angle::Ddm> for Ddm {
    fn from(d: gc::angle::Ddm) -> Self {
        Ddm {
            degrees: d.degrees,
            minutes: d.minutes,
            hemisphere: d.hemisphere.into(),
        }
    }
}

impl From<Ddm> for gc::angle::Ddm {
    fn from(d: Ddm) -> Self {
        gc::angle::Ddm {
            degrees: d.degrees,
            minutes: d.minutes,
            hemisphere: d.hemisphere.into(),
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

// --- China datum conversions (exact forward) ---

/// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
#[uniffi::export]
pub fn wgs84_to_gcj02(p: Wgs84) -> Gcj02 {
    gc::Wgs84::from(p).to_gcj02().into()
}

/// WGS-84 → BD-09. **Exact** composition through GCJ-02.
#[uniffi::export]
pub fn wgs84_to_bd09(p: Wgs84) -> Bd09 {
    gc::Wgs84::from(p).to_bd09().into()
}

/// GCJ-02 → BD-09. **Exact** forward nudge.
#[uniffi::export]
pub fn gcj02_to_bd09(p: Gcj02) -> Bd09 {
    gc::Gcj02::from(p).to_bd09().into()
}

// --- China datum conversions (approximate inverse) ---

/// GCJ-02 → WGS-84, fast single-step inverse (~1–2 m). **Approximate**: the
/// returned record carries `max_error_m`.
#[uniffi::export]
pub fn gcj02_to_wgs84_fast(p: Gcj02) -> ApproxWgs84 {
    gc::Gcj02::from(p).to_wgs84_fast().into()
}

/// GCJ-02 → WGS-84, refined fixed-point inverse (< 0.5 m). **Approximate**: the
/// returned record carries `max_error_m`.
#[uniffi::export]
pub fn gcj02_to_wgs84_refined(p: Gcj02) -> ApproxWgs84 {
    gc::Gcj02::from(p).to_wgs84_refined().into()
}

/// BD-09 → GCJ-02, fast single-step inverse. **Approximate**: the returned
/// record carries `max_error_m`.
#[uniffi::export]
pub fn bd09_to_gcj02_fast(p: Bd09) -> ApproxGcj02 {
    gc::Bd09::from(p).to_gcj02_fast().into()
}

/// BD-09 → GCJ-02, refined fixed-point inverse (sub-meter). **Approximate**:
/// the returned record carries `max_error_m`.
#[uniffi::export]
pub fn bd09_to_gcj02_refined(p: Bd09) -> ApproxGcj02 {
    gc::Bd09::from(p).to_gcj02_refined().into()
}

/// BD-09 → WGS-84, refined composition through GCJ-02. **Approximate**: the
/// returned record carries the summed `max_error_m`.
#[uniffi::export]
pub fn bd09_to_wgs84_refined(p: Bd09) -> ApproxWgs84 {
    gc::Bd09::from(p).to_wgs84_refined().into()
}

// --- Baidu Web Mercator (exact, both ways) ---

/// BD-09 lat/lon → Baidu Web Mercator (exact forward projection).
#[uniffi::export]
pub fn baidu_mercator_from_bd09(p: Bd09) -> BaiduMercator {
    gc::BaiduMercator::from_bd09(gc::Bd09::from(p)).into()
}

/// Baidu Web Mercator → BD-09 lat/lon (exact inverse projection).
#[uniffi::export]
pub fn baidu_mercator_to_bd09(m: BaiduMercator) -> Bd09 {
    gc::BaiduMercator::from(m).to_bd09().into()
}

/// Baidu Web Mercator → canonical [`Coordinate`], tagged BD-09 (exact).
#[uniffi::export]
pub fn baidu_mercator_to_coordinate(m: BaiduMercator) -> Coordinate {
    gc::BaiduMercator::from(m).to_coordinate().into()
}

/// Canonical [`Coordinate`] → Baidu Web Mercator.
///
/// Errors with [`GeoError::CrsMismatch`] unless the coordinate is BD-09 — a
/// non-BD-09 coordinate must be converted to BD-09 first, never silently
/// reprojected.
#[uniffi::export]
pub fn baidu_mercator_try_from_coordinate(coord: Coordinate) -> Result<BaiduMercator, GeoError> {
    gc::BaiduMercator::try_from_coordinate(coord.into())
        .map(Into::into)
        .map_err(GeoError::from)
}

// --- Helpers ---

/// Whether `(lat, lon)` lies outside the China bounding box, where every China
/// datum conversion is the identity.
#[uniffi::export]
pub fn out_of_china(lat: f64, lon: f64) -> bool {
    gc::china::out_of_china(lat, lon)
}

/// Cheap spherical (haversine) distance between two coordinates, in **meters**.
///
/// `Length` and its operators do not cross the FFI boundary, so the scalar
/// meters value is returned directly.
#[uniffi::export]
pub fn haversine_distance_m(a: Coordinate, b: Coordinate) -> f64 {
    let a: gc::Coordinate = a.into();
    let b: gc::Coordinate = b.into();
    gc::geodesy::haversine_distance(&a, &b).meters()
}

// --- Angle conversions (mirror of `From`/`to_*` on the angle types) ---

/// Decimal degrees → degrees/minutes/seconds for the given axis.
#[uniffi::export]
pub fn dd_to_dms(dd: Dd, axis: Axis) -> Dms {
    gc::angle::Dd::from(dd).to_dms(axis.into()).into()
}

/// Decimal degrees → degrees/decimal-minutes for the given axis.
#[uniffi::export]
pub fn dd_to_ddm(dd: Dd, axis: Axis) -> Ddm {
    gc::angle::Dd::from(dd).to_ddm(axis.into()).into()
}

/// Degrees/minutes/seconds → degrees/decimal-minutes (preserves hemisphere).
#[uniffi::export]
pub fn dms_to_ddm(dms: Dms) -> Ddm {
    gc::angle::Dms::from(dms).to_ddm().into()
}

/// Degrees/decimal-minutes → degrees/minutes/seconds (preserves hemisphere).
#[uniffi::export]
pub fn ddm_to_dms(ddm: Ddm) -> Dms {
    gc::angle::Ddm::from(ddm).to_dms().into()
}

/// Degrees/minutes/seconds → decimal degrees (signed by hemisphere).
#[uniffi::export]
pub fn dms_to_dd(dms: Dms) -> Dd {
    gc::angle::Dd::from(gc::angle::Dms::from(dms)).into()
}

/// Degrees/decimal-minutes → decimal degrees (signed by hemisphere).
#[uniffi::export]
pub fn ddm_to_dd(ddm: Ddm) -> Dd {
    gc::angle::Dd::from(gc::angle::Ddm::from(ddm)).into()
}

/// The numeric sign a hemisphere applies (`-1.0` for South/West, else `+1.0`).
#[uniffi::export]
pub fn hemisphere_sign(hemisphere: Hemisphere) -> f64 {
    gc::angle::Hemisphere::from(hemisphere).sign()
}

// --- Angle normalization helpers ---

/// Wrap a longitude into the half-open range `[-180, 180)` (so `180` → `-180`).
#[uniffi::export]
pub fn wrap_longitude(lon_deg: f64) -> f64 {
    gc::angle::wrap_longitude(lon_deg)
}

/// Clamp a latitude into the closed range `[-90, 90]`.
#[uniffi::export]
pub fn clamp_latitude(lat_deg: f64) -> f64 {
    gc::angle::clamp_latitude(lat_deg)
}

/// Normalize an angle (degrees) into `[0, 360)`.
#[uniffi::export]
pub fn normalize_degrees(deg: f64) -> f64 {
    gc::angle::normalize_degrees(deg)
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
