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

/// The decoded cell of a grid code (Plus Code, geohash, Maidenhead) — the
/// WGS-84 cell **center** plus the cell half-diagonal error bound. The
/// flattened form of `Approx<Coordinate>` for the (always WGS-84) grid systems.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct GridCell {
    /// Latitude of the cell center, in decimal degrees.
    pub lat: f64,
    /// Longitude of the cell center, in decimal degrees.
    pub lon: f64,
    /// Estimated maximum positional error (cell half-diagonal), in meters.
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

// --- Coordinate formatting (mirror of `gc::format`) ---

/// Target representation for rendering — mirror of [`gc::format::Representation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Representation {
    /// Decimal degrees (`40.712800, -74.006000`).
    DecimalDegrees,
    /// Degrees-minutes-seconds (`40°42′46″N 74°00′22″W`).
    Dms,
    /// Degrees-decimal-minutes (`40°42.766′N`).
    Ddm,
    /// Open Location Code / Plus Code (`8FVC2222+22`).
    PlusCode,
}

/// Symbol style for DMS/DDM rendering — mirror of [`gc::format::SymbolStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SymbolStyle {
    /// Unicode `°′″`.
    Unicode,
    /// ASCII `°'"`.
    Ascii,
    /// Plain letters `d m s`.
    Letters,
}

/// Hemisphere sign style — mirror of [`gc::format::HemisphereStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HemisphereStyle {
    /// Signed numbers (`-74.006`).
    Signed,
    /// Cardinal letters (`74.006 W`).
    Cardinal,
}

/// Options controlling how a coordinate is rendered — mirror of
/// [`gc::FormatOptions`](gc::format::FormatOptions).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FormatOptions {
    /// Target representation.
    pub representation: Representation,
    /// Decimal places (DD) or sub-second/minute precision; `None` → a sensible
    /// per-representation default.
    pub precision: Option<u8>,
    /// Symbol style for DMS/DDM.
    pub symbol_style: SymbolStyle,
    /// Hemisphere rendering.
    pub hemisphere_style: HemisphereStyle,
    /// BCP-47 locale tag for number formatting (e.g. decimal comma).
    pub locale: Option<String>,
}

/// Options controlling tolerant text parsing — mirror of
/// [`gc::TextParseOptions`](gc::parse::text::TextParseOptions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TextParseOptions {
    /// Axis order to assume when the range heuristics are inconclusive.
    pub default_axis_order: AxisOrder,
    /// Whether to interpret `,` as a decimal separator (European locales).
    pub decimal_comma: bool,
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

impl From<gc::format::Representation> for Representation {
    fn from(r: gc::format::Representation) -> Self {
        match r {
            gc::format::Representation::DecimalDegrees => Representation::DecimalDegrees,
            gc::format::Representation::Dms => Representation::Dms,
            gc::format::Representation::Ddm => Representation::Ddm,
            gc::format::Representation::PlusCode => Representation::PlusCode,
        }
    }
}

impl From<Representation> for gc::format::Representation {
    fn from(r: Representation) -> Self {
        match r {
            Representation::DecimalDegrees => gc::format::Representation::DecimalDegrees,
            Representation::Dms => gc::format::Representation::Dms,
            Representation::Ddm => gc::format::Representation::Ddm,
            Representation::PlusCode => gc::format::Representation::PlusCode,
        }
    }
}

impl From<gc::format::SymbolStyle> for SymbolStyle {
    fn from(s: gc::format::SymbolStyle) -> Self {
        match s {
            gc::format::SymbolStyle::Unicode => SymbolStyle::Unicode,
            gc::format::SymbolStyle::Ascii => SymbolStyle::Ascii,
            gc::format::SymbolStyle::Letters => SymbolStyle::Letters,
        }
    }
}

impl From<SymbolStyle> for gc::format::SymbolStyle {
    fn from(s: SymbolStyle) -> Self {
        match s {
            SymbolStyle::Unicode => gc::format::SymbolStyle::Unicode,
            SymbolStyle::Ascii => gc::format::SymbolStyle::Ascii,
            SymbolStyle::Letters => gc::format::SymbolStyle::Letters,
        }
    }
}

impl From<gc::format::HemisphereStyle> for HemisphereStyle {
    fn from(h: gc::format::HemisphereStyle) -> Self {
        match h {
            gc::format::HemisphereStyle::Signed => HemisphereStyle::Signed,
            gc::format::HemisphereStyle::Cardinal => HemisphereStyle::Cardinal,
        }
    }
}

impl From<HemisphereStyle> for gc::format::HemisphereStyle {
    fn from(h: HemisphereStyle) -> Self {
        match h {
            HemisphereStyle::Signed => gc::format::HemisphereStyle::Signed,
            HemisphereStyle::Cardinal => gc::format::HemisphereStyle::Cardinal,
        }
    }
}

impl From<gc::format::FormatOptions> for FormatOptions {
    fn from(o: gc::format::FormatOptions) -> Self {
        FormatOptions {
            representation: o.representation.into(),
            precision: o.precision,
            symbol_style: o.symbol_style.into(),
            hemisphere_style: o.hemisphere_style.into(),
            locale: o.locale,
        }
    }
}

impl From<FormatOptions> for gc::format::FormatOptions {
    fn from(o: FormatOptions) -> Self {
        gc::format::FormatOptions {
            representation: o.representation.into(),
            precision: o.precision,
            symbol_style: o.symbol_style.into(),
            hemisphere_style: o.hemisphere_style.into(),
            locale: o.locale,
        }
    }
}

impl From<gc::parse::text::TextParseOptions> for TextParseOptions {
    fn from(o: gc::parse::text::TextParseOptions) -> Self {
        TextParseOptions {
            default_axis_order: o.default_axis_order.into(),
            decimal_comma: o.decimal_comma,
        }
    }
}

impl From<TextParseOptions> for gc::parse::text::TextParseOptions {
    fn from(o: TextParseOptions) -> Self {
        gc::parse::text::TextParseOptions {
            default_axis_order: o.default_axis_order.into(),
            decimal_comma: o.decimal_comma,
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

// --- Coordinate formatting ---

/// Render a coordinate to a string using the given options.
///
/// # Errors
/// Returns a [`GeoError`] if the representation is undefined for the coordinate
/// (the DD/DMS/DDM representations never fail).
#[uniffi::export]
pub fn format_coordinate(coord: Coordinate, options: FormatOptions) -> Result<String, GeoError> {
    let coord: gc::Coordinate = coord.into();
    let options: gc::format::FormatOptions = options.into();
    gc::format::format(&coord, &options).map_err(GeoError::from)
}

/// Render a [`Fix`] to a string, deriving display precision from its accuracy
/// when `options.precision` is `None`.
///
/// # Errors
/// As [`format_coordinate`].
#[uniffi::export]
pub fn format_fix(fix: Fix, options: FormatOptions) -> Result<String, GeoError> {
    let fix: gc::Fix = fix.into();
    let options: gc::format::FormatOptions = options.into();
    gc::format::format_fix(&fix, &options).map_err(GeoError::from)
}

// --- Coordinate parsing ---

/// Best-effort parse of a single coordinate from arbitrary input (a `geo:` URI,
/// else free-text DD/DMS/DDM heuristics). The [`Fix`] records the assumed axis
/// order and parse confidence.
///
/// # Errors
/// Returns a [`GeoError`] when no interpretation is found.
#[uniffi::export]
pub fn parse_coordinate(input: String) -> Result<Fix, GeoError> {
    gc::parse::parse_coordinate(&input)
        .map(Into::into)
        .map_err(GeoError::from)
}

/// Parse a `geo:` URI per RFC 5870 (lat-first; optional altitude; `crs`/`u`
/// parameters).
///
/// # Errors
/// Returns a [`GeoError`] when the input is not a well-formed `geo:` URI.
#[uniffi::export]
pub fn from_geo_uri(input: String) -> Result<Fix, GeoError> {
    gc::parse::from_geo_uri(&input)
        .map(Into::into)
        .map_err(GeoError::from)
}

/// Parse a free-text coordinate with explicit tolerant-parsing options.
///
/// # Errors
/// Returns a [`GeoError`] when the input cannot be interpreted.
#[uniffi::export]
pub fn parse_text_with(input: String, options: TextParseOptions) -> Result<Fix, GeoError> {
    let options: gc::parse::text::TextParseOptions = options.into();
    gc::parse::text::parse_with(&input, &options)
        .map(Into::into)
        .map_err(GeoError::from)
}

// --- Grid systems (Plus Code, geohash, Maidenhead) ---

/// Flatten a decoded `Approx<Coordinate>` cell into a [`GridCell`] record.
fn grid_cell(area: gc::Approx<gc::Coordinate>) -> GridCell {
    let max_error_m = area.max_error_m();
    let center = area.into_inner();
    GridCell {
        lat: center.lat,
        lon: center.lon,
        max_error_m,
    }
}

/// Encode a coordinate to an Open Location Code at the given length (clamped to
/// `[2, 15]`). Returns the canonical code string.
#[uniffi::export]
pub fn plus_code_encode(coord: Coordinate, length: u8) -> String {
    gc::grids::PlusCode::encode(coord.into(), usize::from(length))
        .as_str()
        .to_string()
}

/// Decode an Open Location Code to its cell center and error bound.
///
/// # Errors
/// Returns a [`GeoError`] for a malformed or short code.
#[uniffi::export]
pub fn plus_code_decode(code: String) -> Result<GridCell, GeoError> {
    let pc = gc::grids::PlusCode::try_from(code.as_str()).map_err(GeoError::from)?;
    Ok(grid_cell(pc.decode()))
}

/// Encode a coordinate to a geohash of the given character length.
#[uniffi::export]
pub fn geohash_encode(coord: Coordinate, length: u8) -> String {
    gc::grids::Geohash::encode(coord.into(), usize::from(length))
        .as_str()
        .to_string()
}

/// Decode a geohash to its cell center and error bound.
///
/// # Errors
/// Returns a [`GeoError`] for non-base-32 input.
#[uniffi::export]
pub fn geohash_decode(code: String) -> Result<GridCell, GeoError> {
    let gh = gc::grids::Geohash::try_from(code.as_str()).map_err(GeoError::from)?;
    Ok(grid_cell(gh.decode()))
}

/// Encode a coordinate to a Maidenhead locator of the given number of pairs
/// (clamped to 1–3).
#[uniffi::export]
pub fn maidenhead_encode(coord: Coordinate, pairs: u8) -> String {
    gc::grids::Maidenhead::encode(coord.into(), usize::from(pairs))
        .as_str()
        .to_string()
}

/// Decode a Maidenhead locator to its grid-square center and error bound.
///
/// # Errors
/// Returns a [`GeoError`] for a malformed locator.
#[uniffi::export]
pub fn maidenhead_decode(code: String) -> Result<GridCell, GeoError> {
    let mh = gc::grids::Maidenhead::try_from(code.as_str()).map_err(GeoError::from)?;
    Ok(grid_cell(mh.decode()))
}

// ===========================================================================
// Geodesy: ellipsoids, ECEF, and local tangent frames
// ===========================================================================

/// A reference ellipsoid — mirror of [`gc::Ellipsoid`](gc::geodesy::Ellipsoid).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ellipsoid {
    /// Semi-major axis `a`, in meters.
    pub semi_major_m: f64,
    /// Inverse flattening `1/f`.
    pub inverse_flattening: f64,
}

/// A geocentric ECEF position in meters — mirror of [`gc::Ecef`](gc::geodesy::Ecef).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ecef {
    /// X axis (meters), through the prime meridian at the equator.
    pub x: f64,
    /// Y axis (meters), 90° east at the equator.
    pub y: f64,
    /// Z axis (meters), through the north pole.
    pub z: f64,
}

/// East-North-Up offset (meters) — mirror of [`gc::Enu`](gc::geodesy::Enu).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Enu {
    /// East offset (meters).
    pub east: f64,
    /// North offset (meters).
    pub north: f64,
    /// Up offset (meters).
    pub up: f64,
}

/// North-East-Down offset (meters) — mirror of [`gc::Ned`](gc::geodesy::Ned).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ned {
    /// North offset (meters).
    pub north: f64,
    /// East offset (meters).
    pub east: f64,
    /// Down offset (meters).
    pub down: f64,
}

/// Azimuth-Elevation-Range — mirror of [`gc::Aer`](gc::geodesy::Aer), with the
/// slant range flattened to meters.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Aer {
    /// Azimuth (degrees clockwise from north).
    pub azimuth_deg: f64,
    /// Elevation (degrees above the local horizontal).
    pub elevation_deg: f64,
    /// Slant range, in meters.
    pub range_m: f64,
}

impl From<gc::geodesy::Ellipsoid> for Ellipsoid {
    fn from(e: gc::geodesy::Ellipsoid) -> Self {
        Ellipsoid {
            semi_major_m: e.semi_major_m,
            inverse_flattening: e.inverse_flattening,
        }
    }
}
impl From<Ellipsoid> for gc::geodesy::Ellipsoid {
    fn from(e: Ellipsoid) -> Self {
        gc::geodesy::Ellipsoid {
            semi_major_m: e.semi_major_m,
            inverse_flattening: e.inverse_flattening,
        }
    }
}
impl From<gc::geodesy::Ecef> for Ecef {
    fn from(c: gc::geodesy::Ecef) -> Self {
        Ecef {
            x: c.x,
            y: c.y,
            z: c.z,
        }
    }
}
impl From<Ecef> for gc::geodesy::Ecef {
    fn from(c: Ecef) -> Self {
        gc::geodesy::Ecef::new(c.x, c.y, c.z)
    }
}
impl From<gc::geodesy::Enu> for Enu {
    fn from(e: gc::geodesy::Enu) -> Self {
        Enu {
            east: e.east,
            north: e.north,
            up: e.up,
        }
    }
}
impl From<Enu> for gc::geodesy::Enu {
    fn from(e: Enu) -> Self {
        gc::geodesy::Enu {
            east: e.east,
            north: e.north,
            up: e.up,
        }
    }
}
impl From<gc::geodesy::Ned> for Ned {
    fn from(n: gc::geodesy::Ned) -> Self {
        Ned {
            north: n.north,
            east: n.east,
            down: n.down,
        }
    }
}
impl From<Ned> for gc::geodesy::Ned {
    fn from(n: Ned) -> Self {
        gc::geodesy::Ned {
            north: n.north,
            east: n.east,
            down: n.down,
        }
    }
}
impl From<gc::geodesy::Aer> for Aer {
    fn from(a: gc::geodesy::Aer) -> Self {
        Aer {
            azimuth_deg: a.azimuth_deg,
            elevation_deg: a.elevation_deg,
            range_m: a.range.meters(),
        }
    }
}
impl From<Aer> for gc::geodesy::Aer {
    fn from(a: Aer) -> Self {
        gc::geodesy::Aer {
            azimuth_deg: a.azimuth_deg,
            elevation_deg: a.elevation_deg,
            range: gc::Length::from_meters(a.range_m),
        }
    }
}

/// The WGS-84 ellipsoid.
#[uniffi::export]
pub fn ellipsoid_wgs84() -> Ellipsoid {
    gc::geodesy::Ellipsoid::WGS84.into()
}
/// The GRS80 ellipsoid (NAD83 / ETRS89).
#[uniffi::export]
pub fn ellipsoid_grs80() -> Ellipsoid {
    gc::geodesy::Ellipsoid::GRS80.into()
}
/// The Krasovsky-1940 ellipsoid (Pulkovo-1942 / SK-42).
#[uniffi::export]
pub fn ellipsoid_krasovsky_1940() -> Ellipsoid {
    gc::geodesy::Ellipsoid::KRASOVSKY_1940.into()
}
/// The Airy-1830 ellipsoid (OSGB36).
#[uniffi::export]
pub fn ellipsoid_airy_1830() -> Ellipsoid {
    gc::geodesy::Ellipsoid::AIRY_1830.into()
}
/// The Bessel-1841 ellipsoid (Tokyo datum).
#[uniffi::export]
pub fn ellipsoid_bessel_1841() -> Ellipsoid {
    gc::geodesy::Ellipsoid::BESSEL_1841.into()
}
/// The Clarke-1866 ellipsoid (NAD27).
#[uniffi::export]
pub fn ellipsoid_clarke_1866() -> Ellipsoid {
    gc::geodesy::Ellipsoid::CLARKE_1866.into()
}

/// Flattening `f` of the ellipsoid.
#[uniffi::export]
pub fn ellipsoid_flattening(ellipsoid: Ellipsoid) -> f64 {
    gc::geodesy::Ellipsoid::from(ellipsoid).flattening()
}
/// Semi-minor axis `b` (meters) of the ellipsoid.
#[uniffi::export]
pub fn ellipsoid_semi_minor_m(ellipsoid: Ellipsoid) -> f64 {
    gc::geodesy::Ellipsoid::from(ellipsoid).semi_minor_m()
}
/// First eccentricity squared `e²` of the ellipsoid.
#[uniffi::export]
pub fn ellipsoid_eccentricity_sq(ellipsoid: Ellipsoid) -> f64 {
    gc::geodesy::Ellipsoid::from(ellipsoid).eccentricity_sq()
}

/// Geodetic [`Coordinate`] → ECEF on the given ellipsoid.
#[uniffi::export]
pub fn ecef_from_coordinate(coord: Coordinate, ellipsoid: Ellipsoid) -> Ecef {
    gc::geodesy::Ecef::from_coordinate(coord.into(), ellipsoid.into()).into()
}
/// ECEF → geodetic [`Coordinate`] on the given ellipsoid (tagged WGS-84).
#[uniffi::export]
pub fn ecef_to_coordinate(ecef: Ecef, ellipsoid: Ellipsoid) -> Coordinate {
    gc::geodesy::Ecef::from(ecef)
        .to_coordinate(ellipsoid.into())
        .into()
}

/// The ENU offset of `target` relative to `origin` (WGS-84).
#[uniffi::export]
pub fn enu_from_coordinate(target: Coordinate, origin: Coordinate) -> Enu {
    gc::geodesy::Enu::from_coordinate(target.into(), origin.into()).into()
}
/// Recover the absolute coordinate of an ENU offset about `origin`.
#[uniffi::export]
pub fn enu_to_coordinate(enu: Enu, origin: Coordinate) -> Coordinate {
    gc::geodesy::Enu::from(enu)
        .to_coordinate(origin.into())
        .into()
}
/// ENU → NED.
#[uniffi::export]
pub fn enu_to_ned(enu: Enu) -> Ned {
    gc::geodesy::Enu::from(enu).to_ned().into()
}
/// ENU → azimuth/elevation/range.
#[uniffi::export]
pub fn enu_to_aer(enu: Enu) -> Aer {
    gc::geodesy::Enu::from(enu).to_aer().into()
}

/// The NED offset of `target` relative to `origin` (WGS-84).
#[uniffi::export]
pub fn ned_from_coordinate(target: Coordinate, origin: Coordinate) -> Ned {
    gc::geodesy::Ned::from_coordinate(target.into(), origin.into()).into()
}
/// Recover the absolute coordinate of a NED offset about `origin`.
#[uniffi::export]
pub fn ned_to_coordinate(ned: Ned, origin: Coordinate) -> Coordinate {
    gc::geodesy::Ned::from(ned)
        .to_coordinate(origin.into())
        .into()
}
/// NED → ENU.
#[uniffi::export]
pub fn ned_to_enu(ned: Ned) -> Enu {
    gc::geodesy::Ned::from(ned).to_enu().into()
}
/// NED → azimuth/elevation/range.
#[uniffi::export]
pub fn ned_to_aer(ned: Ned) -> Aer {
    gc::geodesy::Ned::from(ned).to_aer().into()
}

/// The azimuth/elevation/range of `target` relative to `origin` (WGS-84).
#[uniffi::export]
pub fn aer_from_coordinate(target: Coordinate, origin: Coordinate) -> Aer {
    gc::geodesy::Aer::from_coordinate(target.into(), origin.into()).into()
}
/// Recover the absolute coordinate of an AER offset about `origin`.
#[uniffi::export]
pub fn aer_to_coordinate(aer: Aer, origin: Coordinate) -> Coordinate {
    gc::geodesy::Aer::from(aer)
        .to_coordinate(origin.into())
        .into()
}
/// AER → ENU.
#[uniffi::export]
pub fn aer_to_enu(aer: Aer) -> Enu {
    gc::geodesy::Aer::from(aer).to_enu().into()
}
/// AER → NED.
#[uniffi::export]
pub fn aer_to_ned(aer: Aer) -> Ned {
    gc::geodesy::Aer::from(aer).to_ned().into()
}

// ===========================================================================
// Geodesics (distances, bearings, producers)
// ===========================================================================

/// Exact ellipsoidal (Karney) geodesic distance between two coordinates, in
/// **meters**.
#[uniffi::export]
pub fn geodesic_distance_m(a: Coordinate, b: Coordinate) -> f64 {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.into(), b.into());
    gc::geodesy::geodesic_distance(&a, &b).meters()
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees `[0, 360)`.
#[uniffi::export]
pub fn initial_bearing(a: Coordinate, b: Coordinate) -> f64 {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.into(), b.into());
    gc::geodesy::initial_bearing(&a, &b)
}

/// Final bearing (azimuth on arrival) from `a` to `b`, in degrees `[0, 360)`.
#[uniffi::export]
pub fn final_bearing(a: Coordinate, b: Coordinate) -> f64 {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.into(), b.into());
    gc::geodesy::final_bearing(&a, &b)
}

/// The point reached from `start` along `bearing_deg` for `distance_m` meters
/// (exact, Karney).
#[uniffi::export]
pub fn destination(start: Coordinate, bearing_deg: f64, distance_m: f64) -> Coordinate {
    gc::geodesy::destination(
        &start.into(),
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .into()
}

/// The geodesic midpoint between `a` and `b`.
#[uniffi::export]
pub fn midpoint(a: Coordinate, b: Coordinate) -> Coordinate {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.into(), b.into());
    gc::geodesy::midpoint(&a, &b).into()
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the geodesic.
#[uniffi::export]
pub fn intermediate(a: Coordinate, b: Coordinate, fraction: f64) -> Coordinate {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.into(), b.into());
    gc::geodesy::intermediate(&a, &b, fraction).into()
}

/// Rhumb-line (loxodrome) distance between two coordinates, in **meters**.
#[uniffi::export]
pub fn rhumb_distance_m(a: Coordinate, b: Coordinate) -> f64 {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.into(), b.into());
    gc::geodesy::rhumb_distance(&a, &b).meters()
}

/// Rhumb-line (constant) bearing from `a` to `b`, in degrees `[0, 360)`.
#[uniffi::export]
pub fn rhumb_bearing(a: Coordinate, b: Coordinate) -> f64 {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.into(), b.into());
    gc::geodesy::rhumb_bearing(&a, &b)
}

/// The point reached from `start` along a constant `bearing_deg` rhumb line for
/// `distance_m` meters.
#[uniffi::export]
pub fn rhumb_destination(start: Coordinate, bearing_deg: f64, distance_m: f64) -> Coordinate {
    gc::geodesy::rhumb_destination(
        &start.into(),
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .into()
}

/// Signed perpendicular distance (meters) from `point` to the path
/// `start` → `end` (positive to the right).
#[uniffi::export]
pub fn cross_track_distance_m(point: Coordinate, start: Coordinate, end: Coordinate) -> f64 {
    let (point, start, end): (gc::Coordinate, gc::Coordinate, gc::Coordinate) =
        (point.into(), start.into(), end.into());
    gc::geodesy::cross_track_distance(&point, &start, &end).meters()
}

/// Along-track distance (meters) from `start` to the foot of the perpendicular
/// from `point` onto `start` → `end`.
#[uniffi::export]
pub fn along_track_distance_m(point: Coordinate, start: Coordinate, end: Coordinate) -> f64 {
    let (point, start, end): (gc::Coordinate, gc::Coordinate, gc::Coordinate) =
        (point.into(), start.into(), end.into());
    gc::geodesy::along_track_distance(&point, &start, &end).meters()
}

/// Intersection of two great circles (each a point + initial bearing), or
/// `None` when they are parallel/coincident or ambiguous.
#[uniffi::export]
pub fn intersection(
    a: Coordinate,
    bearing_a_deg: f64,
    b: Coordinate,
    bearing_b_deg: f64,
) -> Option<Coordinate> {
    gc::geodesy::intersection(&a.into(), bearing_a_deg, &b.into(), bearing_b_deg).map(Into::into)
}

// ===========================================================================
// Classic datums — Helmert (Bursa-Wolf) transforms
// ===========================================================================

/// The seven Bursa-Wolf parameters of a Helmert transform — mirror of
/// [`gc::Helmert`](gc::geodesy::Helmert). Translations in meters, rotations in
/// arc-seconds (position-vector convention), scale in parts-per-million.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Helmert {
    /// X-axis translation, meters.
    pub tx_m: f64,
    /// Y-axis translation, meters.
    pub ty_m: f64,
    /// Z-axis translation, meters.
    pub tz_m: f64,
    /// X-axis rotation, arc-seconds (position-vector convention).
    pub rx_arcsec: f64,
    /// Y-axis rotation, arc-seconds (position-vector convention).
    pub ry_arcsec: f64,
    /// Z-axis rotation, arc-seconds (position-vector convention).
    pub rz_arcsec: f64,
    /// Scale difference, parts-per-million.
    pub scale_ppm: f64,
}

/// A complete datum transform (source/target ellipsoids + the Helmert shift) —
/// mirror of [`gc::DatumTransform`](gc::geodesy::DatumTransform).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct DatumTransform {
    /// Ellipsoid of the source datum.
    pub from: Ellipsoid,
    /// Ellipsoid of the target datum.
    pub to: Ellipsoid,
    /// Helmert parameters carrying the source frame to the target frame.
    pub helmert: Helmert,
}

impl From<gc::geodesy::Helmert> for Helmert {
    fn from(h: gc::geodesy::Helmert) -> Self {
        Helmert {
            tx_m: h.tx_m,
            ty_m: h.ty_m,
            tz_m: h.tz_m,
            rx_arcsec: h.rx_arcsec,
            ry_arcsec: h.ry_arcsec,
            rz_arcsec: h.rz_arcsec,
            scale_ppm: h.scale_ppm,
        }
    }
}
impl From<Helmert> for gc::geodesy::Helmert {
    fn from(h: Helmert) -> Self {
        gc::geodesy::Helmert {
            tx_m: h.tx_m,
            ty_m: h.ty_m,
            tz_m: h.tz_m,
            rx_arcsec: h.rx_arcsec,
            ry_arcsec: h.ry_arcsec,
            rz_arcsec: h.rz_arcsec,
            scale_ppm: h.scale_ppm,
        }
    }
}
impl From<gc::geodesy::DatumTransform> for DatumTransform {
    fn from(d: gc::geodesy::DatumTransform) -> Self {
        DatumTransform {
            from: d.from.into(),
            to: d.to.into(),
            helmert: d.helmert.into(),
        }
    }
}
impl From<DatumTransform> for gc::geodesy::DatumTransform {
    fn from(d: DatumTransform) -> Self {
        gc::geodesy::DatumTransform {
            from: d.from.into(),
            to: d.to.into(),
            helmert: d.helmert.into(),
        }
    }
}

/// The identity Helmert transform (no translation, rotation, or scale).
#[uniffi::export]
pub fn helmert_identity() -> Helmert {
    gc::geodesy::Helmert::IDENTITY.into()
}

/// Apply a Helmert transform to a geocentric (ECEF) position.
#[uniffi::export]
pub fn helmert_apply_ecef(helmert: Helmert, ecef: Ecef) -> Ecef {
    gc::geodesy::Helmert::from(helmert)
        .apply_ecef(ecef.into())
        .into()
}

/// The inverse Helmert transform (negated parameters).
#[uniffi::export]
pub fn helmert_inverse(helmert: Helmert) -> Helmert {
    gc::geodesy::Helmert::from(helmert).inverse().into()
}

/// The catalogued transform carrying `datum` to WGS-84, or `None` when none is
/// built in (WGS-84 itself and the China obfuscation systems).
#[uniffi::export]
pub fn datum_transform_to_wgs84(datum: Crs) -> Option<DatumTransform> {
    gc::geodesy::DatumTransform::to_wgs84(datum.into()).map(Into::into)
}

/// Transform a geodetic coordinate from the source to the target datum, tagging
/// the result with `to`.
#[uniffi::export]
pub fn datum_transform_apply(transform: DatumTransform, coord: Coordinate, to: Crs) -> Coordinate {
    gc::geodesy::DatumTransform::from(transform)
        .transform(coord.into(), to.into())
        .into()
}

/// The reverse datum transform (swaps ellipsoids, inverts the Helmert shift).
#[uniffi::export]
pub fn datum_transform_inverse(transform: DatumTransform) -> DatumTransform {
    gc::geodesy::DatumTransform::from(transform)
        .inverse()
        .into()
}

// ===========================================================================
// Runtime conversion dispatch
// ===========================================================================

/// A converted coordinate with its error bound — the flattened FFI form of
/// `Approx<Coordinate>`. `max_error_m` is `0.0` for exact routes.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ApproxCoordinate {
    /// The converted coordinate (carries its target [`Crs`]).
    pub coord: Coordinate,
    /// Estimated upper bound on positional error, in meters (`0.0` if exact).
    pub max_error_m: f64,
}

/// Convert `coord` from its own reference system to `to`, routing through the
/// WGS-84 hub (China typed conversions + classic-datum Helmert transforms).
///
/// # Errors
/// `GeoError` when no route is known between the two systems.
#[uniffi::export]
pub fn convert(coord: Coordinate, to: Crs) -> Result<ApproxCoordinate, GeoError> {
    gc::convert::convert(coord.into(), to.into())
        .map(|a| {
            let max_error_m = a.max_error_m();
            ApproxCoordinate {
                coord: a.into_inner().into(),
                max_error_m,
            }
        })
        .map_err(GeoError::from)
}

/// Whether a conversion route exists between two reference systems.
#[uniffi::export]
pub fn can_convert(from: Crs, to: Crs) -> bool {
    gc::convert::can_convert(from.into(), to.into())
}

// ===========================================================================
// UTM / UPS projections and MGRS
// ===========================================================================

/// Northern or southern band for a UTM/UPS coordinate — mirror of
/// [`gc::grids::utm::Hemisphere`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum UtmHemisphere {
    /// Northern hemisphere.
    North,
    /// Southern hemisphere.
    South,
}

impl From<gc::grids::utm::Hemisphere> for UtmHemisphere {
    fn from(h: gc::grids::utm::Hemisphere) -> Self {
        match h {
            gc::grids::utm::Hemisphere::North => UtmHemisphere::North,
            gc::grids::utm::Hemisphere::South => UtmHemisphere::South,
        }
    }
}
impl From<UtmHemisphere> for gc::grids::utm::Hemisphere {
    fn from(h: UtmHemisphere) -> Self {
        match h {
            UtmHemisphere::North => gc::grids::utm::Hemisphere::North,
            UtmHemisphere::South => gc::grids::utm::Hemisphere::South,
        }
    }
}

/// A UTM coordinate — mirror of [`gc::Utm`](gc::grids::Utm).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Utm {
    /// Longitude zone number, 1–60.
    pub zone: u8,
    /// Hemisphere band.
    pub hemisphere: UtmHemisphere,
    /// Easting in meters (false-easting applied).
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

/// A UPS (polar) coordinate — mirror of [`gc::Ups`](gc::grids::Ups).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ups {
    /// North or south polar zone.
    pub hemisphere: UtmHemisphere,
    /// Easting in meters.
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

impl From<gc::grids::Utm> for Utm {
    fn from(u: gc::grids::Utm) -> Self {
        Utm {
            zone: u.zone,
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}
impl From<Utm> for gc::grids::Utm {
    fn from(u: Utm) -> Self {
        gc::grids::Utm {
            zone: u.zone,
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}
impl From<gc::grids::Ups> for Ups {
    fn from(u: gc::grids::Ups) -> Self {
        Ups {
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}
impl From<Ups> for gc::grids::Ups {
    fn from(u: Ups) -> Self {
        gc::grids::Ups {
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}

/// Geodetic → UTM. Errors in the polar regions (use UPS there).
#[uniffi::export]
pub fn utm_from_coordinate(coord: Coordinate) -> Result<Utm, GeoError> {
    gc::grids::Utm::try_from_coordinate(coord.into())
        .map(Into::into)
        .map_err(GeoError::from)
}

/// UTM → geodetic WGS-84 coordinate (exact inverse).
#[uniffi::export]
pub fn utm_to_coordinate(utm: Utm) -> Coordinate {
    gc::grids::Utm::from(utm).to_coordinate().into()
}

/// Geodetic → UPS. Errors outside the polar regions (use UTM there).
#[uniffi::export]
pub fn ups_from_coordinate(coord: Coordinate) -> Result<Ups, GeoError> {
    gc::grids::Ups::try_from_coordinate(coord.into())
        .map(Into::into)
        .map_err(GeoError::from)
}

/// UPS → geodetic WGS-84 coordinate (exact inverse).
#[uniffi::export]
pub fn ups_to_coordinate(ups: Ups) -> Coordinate {
    gc::grids::Ups::from(ups).to_coordinate().into()
}

/// Encode a coordinate to an MGRS string at the given precision in meters
/// (1 m … 100 km, snapped to a power of ten).
#[uniffi::export]
pub fn mgrs_from_coordinate(coord: Coordinate, precision_m: u32) -> String {
    gc::grids::Mgrs::from_coordinate(coord.into(), precision_m)
        .as_str()
        .to_string()
}

/// Decode an MGRS string to the center of its square, with the half-square
/// error bound.
///
/// # Errors
/// `GeoError` when the string is not a valid MGRS reference.
#[uniffi::export]
pub fn mgrs_to_coordinate(mgrs: String) -> Result<ApproxCoordinate, GeoError> {
    let parsed = gc::grids::Mgrs::try_from(mgrs.as_str()).map_err(GeoError::from)?;
    let approx = parsed.to_coordinate();
    let max_error_m = approx.max_error_m();
    Ok(ApproxCoordinate {
        coord: approx.into_inner().into(),
        max_error_m,
    })
}

/// The precision in meters implied by an MGRS string's digit count.
///
/// # Errors
/// `GeoError` when the string is not a valid MGRS reference.
#[uniffi::export]
pub fn mgrs_precision_m(mgrs: String) -> Result<u32, GeoError> {
    gc::grids::Mgrs::try_from(mgrs.as_str())
        .map(|m| m.precision_m())
        .map_err(GeoError::from)
}

// ===========================================================================
// Structured interchange formats (GeoJSON / WKT / GPX / KML)
// ===========================================================================

/// Parse positions from a GeoJSON document (lon-lat order) — one [`Fix`] per
/// position (a line of *n* vertices yields *n* fixes).
///
/// # Errors
/// `GeoError` on malformed GeoJSON.
#[uniffi::export]
pub fn from_geojson(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_geojson(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse positions from a WKT string (X-Y order).
///
/// # Errors
/// `GeoError` on malformed WKT.
#[uniffi::export]
pub fn from_wkt(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_wkt(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse track / route / waypoint positions from a GPX document.
///
/// # Errors
/// `GeoError` on malformed GPX.
#[uniffi::export]
pub fn from_gpx(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_gpx(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

/// Parse placemark positions from a KML document (lon,lat,alt order).
///
/// # Errors
/// `GeoError` on malformed KML.
#[uniffi::export]
pub fn from_kml(input: String) -> Result<Vec<Fix>, GeoError> {
    gc::parse::interchange::from_kml(&input)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(GeoError::from)
}

// ===========================================================================
// Sensors — NMEA 0183
// ===========================================================================

/// Parse a single NMEA 0183 sentence (GGA/RMC/GLL) into a [`Fix`]. The optional
/// `*HH` checksum is verified when present.
///
/// # Errors
/// `GeoError` on an unrecognized/invalid sentence or a checksum mismatch.
#[uniffi::export]
pub fn from_nmea_sentence(sentence: String) -> Result<Fix, GeoError> {
    gc::parse::sensors::from_nmea_sentence(&sentence)
        .map(Into::into)
        .map_err(GeoError::from)
}
