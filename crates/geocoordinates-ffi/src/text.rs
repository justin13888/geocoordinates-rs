//! Coordinate text: formatting options and rendering, and parsing of free
//! text and `geo:` URIs.

use geocoordinates as gc;

use crate::{AxisOrder, Coordinate, Fix, GeoError, MgrsPrecision};

// --- Coordinate formatting (mirror of `gc::format`) ---

/// Target representation for rendering, carrying the settings that apply to
/// it — mirror of [`gc::format::Representation`].
///
/// An unset decimal count (`None`) lets [`format_fix`] derive it from the
/// fix's accuracy, and otherwise uses the default (DD 6, DMS 2, DDM 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Representation {
    /// Decimal degrees (`40.712800, -74.006000`).
    DecimalDegrees {
        /// Decimal places of degrees, at most 15.
        decimals: Option<u8>,
        /// Sign or cardinal letter.
        hemisphere: HemisphereStyle,
    },
    /// Degrees-minutes-seconds (`40°42′46.08″N 74°00′21.60″W`).
    Dms {
        /// Decimal places of arcseconds, at most 15.
        second_decimals: Option<u8>,
        /// Degree / minute / second glyphs.
        symbols: SymbolStyle,
        /// Sign or cardinal letter.
        hemisphere: HemisphereStyle,
    },
    /// Degrees-decimal-minutes (`40°42.768′N 74°00.360′W`).
    Ddm {
        /// Decimal places of arcminutes, at most 15.
        minute_decimals: Option<u8>,
        /// Degree / minute glyphs.
        symbols: SymbolStyle,
        /// Sign or cardinal letter.
        hemisphere: HemisphereStyle,
    },
    /// Open Location Code / Plus Code (`8FVC2222+22`).
    PlusCode {
        /// Code length: 2, 4, 6, 8, or 10 through 15.
        length: u8,
    },
    /// Geohash (`ezs42`).
    Geohash {
        /// Character count, 1 through 12.
        length: u8,
    },
    /// Maidenhead locator (`FN20`).
    Maidenhead {
        /// Letter/digit pairs, 1 through 3.
        pairs: u8,
    },
    /// MGRS reference (`18TWK0000027757`).
    Mgrs {
        /// The square size.
        precision: MgrsPrecision,
    },
}

/// Decimal separator — mirror of [`gc::format::DecimalSeparator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DecimalSeparator {
    /// `.` (`40.712800, -74.006000`).
    Point,
    /// `,`; the two components are then separated by whitespace
    /// (`40,712800 -74,006000`).
    Comma,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FormatOptions {
    /// Target representation and its settings.
    pub representation: Representation,
    /// Decimal separator for the angle representations (the grids ignore
    /// it); pick it from a locale with [`decimal_separator_for_locale`].
    pub decimal_separator: DecimalSeparator,
}

/// A grid whose strings [`parse_coordinate_with`] can decode — mirror of
/// [`gc::parse::GridKind`].
///
/// Plus Code and MGRS strings are always detected; Geohash and Maidenhead
/// strings (`"42"` and `"bed"` are valid geohashes) only when
/// [`ParseOptions::expect_grid`] names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum GridKind {
    /// Open Location Code / Plus Code (`8FVC2222+22`).
    PlusCode,
    /// Geohash (`ezs42`).
    Geohash,
    /// Maidenhead locator (`FN20`).
    Maidenhead,
    /// MGRS reference (`18TWK0000027757`).
    Mgrs,
}

/// Options controlling [`parse_coordinate_with`] and [`parse_text_with`] —
/// mirror of [`gc::parse::ParseOptions`]. Take them from
/// [`format_options_parse_options`] to read back a rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ParseOptions {
    /// Axis order to assume for free text when neither a hemisphere letter
    /// nor the `|value| > 90` range rule decides it.
    pub default_axis_order: AxisOrder,
    /// Decimal separator of free text. With `Comma`, a comma between two
    /// digits is a decimal point, and the remaining commas, `;` and
    /// whitespace separate the two components.
    pub decimal_separator: DecimalSeparator,
    /// A grid whose strings to try before anything but a `geo:` URI; needed
    /// for `Geohash` and `Maidenhead`, which are never detected otherwise.
    pub expect_grid: Option<GridKind>,
}

impl From<gc::format::Representation> for Representation {
    fn from(r: gc::format::Representation) -> Self {
        use gc::format::Representation as R;
        match r {
            R::DecimalDegrees {
                decimals,
                hemisphere,
            } => Representation::DecimalDegrees {
                decimals,
                hemisphere: hemisphere.into(),
            },
            R::Dms {
                second_decimals,
                symbols,
                hemisphere,
            } => Representation::Dms {
                second_decimals,
                symbols: symbols.into(),
                hemisphere: hemisphere.into(),
            },
            R::Ddm {
                minute_decimals,
                symbols,
                hemisphere,
            } => Representation::Ddm {
                minute_decimals,
                symbols: symbols.into(),
                hemisphere: hemisphere.into(),
            },
            R::PlusCode { length } => Representation::PlusCode { length },
            R::Geohash { length } => Representation::Geohash { length },
            R::Maidenhead { pairs } => Representation::Maidenhead { pairs },
            R::Mgrs { precision } => Representation::Mgrs {
                precision: precision.into(),
            },
        }
    }
}

impl From<Representation> for gc::format::Representation {
    fn from(r: Representation) -> Self {
        use gc::format::Representation as R;
        match r {
            Representation::DecimalDegrees {
                decimals,
                hemisphere,
            } => R::DecimalDegrees {
                decimals,
                hemisphere: hemisphere.into(),
            },
            Representation::Dms {
                second_decimals,
                symbols,
                hemisphere,
            } => R::Dms {
                second_decimals,
                symbols: symbols.into(),
                hemisphere: hemisphere.into(),
            },
            Representation::Ddm {
                minute_decimals,
                symbols,
                hemisphere,
            } => R::Ddm {
                minute_decimals,
                symbols: symbols.into(),
                hemisphere: hemisphere.into(),
            },
            Representation::PlusCode { length } => R::PlusCode { length },
            Representation::Geohash { length } => R::Geohash { length },
            Representation::Maidenhead { pairs } => R::Maidenhead { pairs },
            Representation::Mgrs { precision } => R::Mgrs {
                precision: precision.into(),
            },
        }
    }
}

impl From<gc::format::DecimalSeparator> for DecimalSeparator {
    fn from(s: gc::format::DecimalSeparator) -> Self {
        match s {
            gc::format::DecimalSeparator::Point => DecimalSeparator::Point,
            gc::format::DecimalSeparator::Comma => DecimalSeparator::Comma,
        }
    }
}

impl From<DecimalSeparator> for gc::format::DecimalSeparator {
    fn from(s: DecimalSeparator) -> Self {
        match s {
            DecimalSeparator::Point => gc::format::DecimalSeparator::Point,
            DecimalSeparator::Comma => gc::format::DecimalSeparator::Comma,
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
            decimal_separator: o.decimal_separator.into(),
        }
    }
}

impl From<FormatOptions> for gc::format::FormatOptions {
    fn from(o: FormatOptions) -> Self {
        gc::format::FormatOptions {
            representation: o.representation.into(),
            decimal_separator: o.decimal_separator.into(),
        }
    }
}

impl From<gc::parse::GridKind> for GridKind {
    fn from(k: gc::parse::GridKind) -> Self {
        match k {
            gc::parse::GridKind::PlusCode => GridKind::PlusCode,
            gc::parse::GridKind::Geohash => GridKind::Geohash,
            gc::parse::GridKind::Maidenhead => GridKind::Maidenhead,
            gc::parse::GridKind::Mgrs => GridKind::Mgrs,
        }
    }
}

impl From<GridKind> for gc::parse::GridKind {
    fn from(k: GridKind) -> Self {
        match k {
            GridKind::PlusCode => gc::parse::GridKind::PlusCode,
            GridKind::Geohash => gc::parse::GridKind::Geohash,
            GridKind::Maidenhead => gc::parse::GridKind::Maidenhead,
            GridKind::Mgrs => gc::parse::GridKind::Mgrs,
        }
    }
}

impl From<gc::parse::ParseOptions> for ParseOptions {
    fn from(o: gc::parse::ParseOptions) -> Self {
        ParseOptions {
            default_axis_order: o.default_axis_order.into(),
            decimal_separator: o.decimal_separator.into(),
            expect_grid: o.expect_grid.map(Into::into),
        }
    }
}

impl From<ParseOptions> for gc::parse::ParseOptions {
    fn from(o: ParseOptions) -> Self {
        gc::parse::ParseOptions {
            default_axis_order: o.default_axis_order.into(),
            decimal_separator: o.decimal_separator.into(),
            expect_grid: o.expect_grid.map(Into::into),
        }
    }
}

// --- Coordinate formatting ---

/// The default options: signed decimal degrees with the decimal count unset
/// (6 for a bare coordinate, derived from accuracy by [`format_fix`]) and a
/// point separator.
#[uniffi::export]
pub fn format_options_default() -> FormatOptions {
    gc::format::FormatOptions::default().into()
}

/// The decimal separator a BCP-47 locale tag conventionally uses.
///
/// A **heuristic**, not locale data: a small allowlist of decimal-comma
/// languages, overridden by the regions known to use a point (`de-CH`,
/// `es-MX`, …). An unknown or malformed tag yields `Point`. The same value
/// as [`ParseOptions::decimal_separator`] re-parses the output with
/// [`parse_coordinate_with`].
#[uniffi::export]
pub fn decimal_separator_for_locale(tag: String) -> DecimalSeparator {
    gc::format::DecimalSeparator::for_locale(&tag).into()
}

/// Render a coordinate to a string using the given options.
///
/// # Errors
/// - `GeoError::OutOfRange` for an out-of-range coordinate record.
/// - `GeoError::InvalidValue` when a decimal count exceeds 15, or a grid
///   representation's size is outside the range its variant documents.
/// - `GeoError::CrsMismatch` when a grid representation is asked to render a
///   non-WGS-84 coordinate.
#[uniffi::export]
pub fn format_coordinate(coord: Coordinate, options: FormatOptions) -> Result<String, GeoError> {
    let coord: gc::Coordinate = coord.try_into()?;
    let options: gc::format::FormatOptions = options.into();
    gc::format::format(&coord, &options).map_err(GeoError::from)
}

/// Render a [`Fix`] to a string, deriving the decimal count from its accuracy
/// where the representation leaves it unset. Only the accuracy's magnitude is
/// read; every [`AccuracyKind`](crate::AccuracyKind) is treated alike.
///
/// # Errors
/// As [`format_coordinate`]; also [`GeoError::InvalidValue`] for a hand-built
/// record whose accuracy ([`accuracy_validate`](crate::accuracy_validate)) or
/// source confidence ([`confidence_validate`](crate::confidence_validate)) is
/// invalid.
#[uniffi::export]
pub fn format_fix(fix: Fix, options: FormatOptions) -> Result<String, GeoError> {
    let fix: gc::fix::Fix = fix.try_into()?;
    let options: gc::format::FormatOptions = options.into();
    gc::format::format_fix(&fix, &options).map_err(GeoError::from)
}

/// The [`ParseOptions`] that read back every string `options` renders through
/// [`parse_coordinate_with`]: lat-first, its decimal separator, and, for a
/// grid representation, that grid as `expect_grid`.
#[uniffi::export]
pub fn format_options_parse_options(options: FormatOptions) -> ParseOptions {
    let options: gc::format::FormatOptions = options.into();
    options.parse_options().into()
}

// --- Coordinate parsing ---

/// The default parse options: lat-first, point decimals, no expected grid.
#[uniffi::export]
pub fn parse_options_default() -> ParseOptions {
    gc::parse::ParseOptions::default().into()
}

/// Best-effort parse of a single coordinate from arbitrary input, with the
/// default options: a `geo:` URI, a Plus Code, an MGRS reference, else
/// free-text DD/DMS/DDM heuristics. Geohash and Maidenhead strings are not
/// detected; ask for them with [`parse_coordinate_with`]. The [`Fix`] records
/// the assumed axis order and parse confidence.
///
/// # Errors
/// Returns a [`GeoError`] when no interpretation is found.
#[uniffi::export]
pub fn parse_coordinate(input: String) -> Result<Fix, GeoError> {
    gc::parse::parse_coordinate(&input)
        .map(Into::into)
        .map_err(GeoError::from)
}

/// [`parse_coordinate`] with explicit options: after a `geo:` URI, the grid
/// `expect_grid` names is tried first, then Plus Code and MGRS, then free
/// text. Pass [`format_options_parse_options`] to read back a rendering.
///
/// # Errors
/// Returns a [`GeoError`] when no interpretation is found.
#[uniffi::export]
pub fn parse_coordinate_with(input: String, options: ParseOptions) -> Result<Fix, GeoError> {
    let options: gc::parse::ParseOptions = options.into();
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

/// Parse a free-text coordinate (no `geo:` URI or grid detection) with
/// explicit options. Free text has no grid to expect, so a set `expect_grid`
/// is rejected rather than ignored; use [`parse_coordinate_with`] for grids.
///
/// # Errors
/// `GeoError::InvalidValue` (field `expect_grid`) when `options.expect_grid`
/// is set; `GeoError::Parse` when the input cannot be interpreted.
#[uniffi::export]
pub fn parse_text_with(input: String, options: ParseOptions) -> Result<Fix, GeoError> {
    let options: gc::parse::ParseOptions = options.into();
    gc::parse::parse_text_with(&input, &options)
        .map(Into::into)
        .map_err(GeoError::from)
}
