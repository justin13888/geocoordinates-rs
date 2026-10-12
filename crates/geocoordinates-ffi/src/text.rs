//! Coordinate text: formatting options and rendering, and parsing of free
//! text and `geo:` URIs.

use geocoordinates as gc;

use crate::{AxisOrder, Coordinate, Fix, GeoError};

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

/// Whether `options` render `,` as the decimal separator (the locale's primary
/// language uses a decimal comma). Pass it as
/// [`TextParseOptions::decimal_comma`] to re-parse the output with
/// [`parse_coordinate_with`].
#[uniffi::export]
pub fn format_options_uses_decimal_comma(options: FormatOptions) -> bool {
    let options: gc::format::FormatOptions = options.into();
    options.uses_decimal_comma()
}

// --- Coordinate parsing ---

/// Best-effort parse of a single coordinate from arbitrary input (a `geo:` URI,
/// a Plus Code, else free-text DD/DMS/DDM heuristics with default options).
/// The [`Fix`] records the assumed axis order and parse confidence.
///
/// # Errors
/// Returns a [`GeoError`] when no interpretation is found.
#[uniffi::export]
pub fn parse_coordinate(input: String) -> Result<Fix, GeoError> {
    gc::parse::parse_coordinate(&input)
        .map(Into::into)
        .map_err(GeoError::from)
}

/// [`parse_coordinate`] with explicit options for the free-text fallback
/// (`geo:` URIs and Plus Codes ignore them). Re-parses decimal-comma output
/// when `decimal_comma` comes from [`format_options_uses_decimal_comma`].
///
/// # Errors
/// Returns a [`GeoError`] when no interpretation is found.
#[uniffi::export]
pub fn parse_coordinate_with(input: String, options: TextParseOptions) -> Result<Fix, GeoError> {
    let options: gc::parse::text::TextParseOptions = options.into();
    gc::parse::parse_coordinate_with(&input, &options)
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
