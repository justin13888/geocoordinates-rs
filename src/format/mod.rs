//! Presentation: locale-aware, round-trip-stable coordinate formatting.
//!
//! Formatting is the inverse of parsing (the `parse` module):
//! a [`Coordinate`] plus [`FormatOptions`] renders to a string in a selectable
//! representation. The guarantee is **round-trip stability** —
//! `parse → model → format → parse` must not drift beyond the rendered
//! precision.
//!
//! Every DD / DMS / DDM output, in every [`SymbolStyle`] and
//! [`HemisphereStyle`], re-parses through
//! [`parse_coordinate_with`](crate::parse::parse_coordinate_with) given
//! `decimal_comma` = [`FormatOptions::uses_decimal_comma`] and the default
//! (lat-first) axis order. For a dot-decimal locale (the default) that is
//! exactly [`parse_coordinate`](crate::parse::parse_coordinate). Plus Code
//! output re-parses through either to the decoded cell's center.

use crate::angle::{Axis, Ddm, Dms};
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::fix::Fix;
use crate::grids::PlusCode;

/// Code length used when rendering a coordinate as a Plus Code (~14 m cells).
const PLUS_CODE_FORMAT_LENGTH: u8 = 10;
/// Highest useful decimal precision for an IEEE-754 `f64`.
const MAX_PRECISION: u8 = 15;

/// Target representation for rendering a coordinate.
///
/// Covers decimal degrees, DMS, DDM and Plus Code. The remaining grid
/// representations (UTM / MGRS / geohash / Maidenhead) are not renderable yet
/// even though those types ship — see [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md). Exhaustive (no
/// `#[non_exhaustive]`): adding a variant compile-forces both the `format`
/// dispatch and the FFI mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Representation {
    /// Decimal degrees (`40.712800, -74.006000`).
    DecimalDegrees,
    /// Degrees-minutes-seconds (`40°42′46″N 74°00′22″W`).
    Dms,
    /// Degrees-decimal-minutes (`40°42.766′N`).
    Ddm,
    /// Open Location Code / Plus Code (`8FVC2222+22`), at a fixed length-10
    /// resolution. Precision/symbol/hemisphere options do not apply.
    PlusCode,
}

/// Symbol style for DMS/DDM rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SymbolStyle {
    /// Unicode `°′″`.
    Unicode,
    /// ASCII `°'"`.
    Ascii,
    /// Plain letters `d m s`.
    Letters,
}

/// Sign style for hemispheres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HemisphereStyle {
    /// Signed numbers (`-74.006`).
    Signed,
    /// Cardinal letters (`74.006 W`).
    Cardinal,
}

/// Options controlling how a coordinate is rendered.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormatOptions {
    /// Target representation.
    pub representation: Representation,
    /// Decimal places (DD) or sub-second/minute precision. When `None`, a
    /// sensible default is used for a bare coordinate; [`format_fix`] instead
    /// derives precision from the fix's accuracy to avoid spurious digits.
    pub precision: Option<u8>,
    /// Symbol style for DMS/DDM.
    pub symbol_style: SymbolStyle,
    /// Hemisphere rendering.
    pub hemisphere_style: HemisphereStyle,
    /// BCP-47 locale tag for number formatting (e.g. decimal comma).
    pub locale: Option<String>,
}

impl FormatOptions {
    /// Whether these options render `,` as the decimal separator, i.e. whether
    /// [`locale`](Self::locale)'s primary language conventionally uses a
    /// decimal comma (`de`, `fr`, `es`, …). Such output separates the two
    /// components with whitespace instead of `, `.
    ///
    /// Pass this as
    /// [`TextParseOptions::decimal_comma`](crate::parse::text::TextParseOptions::decimal_comma)
    /// to re-parse the output.
    #[must_use]
    pub fn uses_decimal_comma(&self) -> bool {
        uses_decimal_comma(self.locale.as_deref())
    }
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            representation: Representation::DecimalDegrees,
            precision: Some(6), // ~0.11 m; printing 9 would be lying
            symbol_style: SymbolStyle::Unicode,
            hemisphere_style: HemisphereStyle::Signed,
            locale: None,
        }
    }
}

/// Render a coordinate to a string using the given options.
///
/// # Errors
/// Returns an error when the requested [`Representation`] is undefined for the
/// coordinate — e.g. a future UTM representation at the poles (see
/// [`crate::Error::InvalidGridRef`]). The DD/DMS/DDM representations never fail.
pub fn format(coord: &Coordinate, options: &FormatOptions) -> Result<String> {
    if options
        .precision
        .is_some_and(|precision| precision > MAX_PRECISION)
    {
        return Err(Error::InvalidValue {
            field: "format precision",
            detail: format!("must be no greater than {MAX_PRECISION}"),
        });
    }
    render(coord, options)
}

/// Render a [`Fix`] to a string, deriving display precision from its
/// [`accuracy`](crate::fix::Fix::accuracy) when `options.precision` is `None`
/// (so spurious digits beyond the fix's resolution are not printed).
///
/// The derived precision is in the representation's last rendered unit:
/// decimals of degrees for DD, of arcseconds for DMS (1″ ≈ 30.9 m), and of
/// arcminutes for DDM (1′ ≈ 1.85 km) — e.g. a 1 m fix renders as 5 DD
/// decimals, 1 DMS decimal, or 3 DDM decimals.
///
/// # Errors
/// As [`format()`].
pub fn format_fix(fix: &Fix, options: &FormatOptions) -> Result<String> {
    if options.precision.is_some() {
        return format(&fix.coord, options);
    }
    let derived = fix
        .accuracy
        .and_then(|a| a.horizontal_m)
        .and_then(|m| precision_for_accuracy(m, options.representation));
    let Some(p) = derived else {
        return format(&fix.coord, options);
    };
    let opts = FormatOptions {
        precision: Some(p),
        ..options.clone()
    };
    format(&fix.coord, &opts)
}

// ===========================================================================
// Rendering internals
// ===========================================================================

/// Which angle encoding [`render_angle`] should emit.
#[derive(Clone, Copy)]
enum AngleKind {
    Dms,
    Ddm,
}

/// Meters per degree of arc on the mean-radius sphere (≈ 111,195 m), a
/// heuristic used only to turn a horizontal accuracy into displayed decimal
/// places. Real ground distance per degree varies with latitude and azimuth;
/// grid decoders measure their bounds geodesically instead.
const MEAN_METERS_PER_DEGREE: f64 = 111_195.0;

/// Decimal places implied by a horizontal accuracy, so digits finer than the
/// fix's resolution are not printed. The places count the representation's
/// last rendered unit: degrees for DD, arcseconds for DMS, arcminutes for DDM.
/// `None` (unusable accuracy, or a representation without a precision knob
/// such as Plus Code) → caller's default.
fn precision_for_accuracy(horizontal_m: f64, representation: Representation) -> Option<u8> {
    if !horizontal_m.is_finite() || horizontal_m <= 0.0 {
        return None;
    }
    let meters_per_unit = match representation {
        Representation::DecimalDegrees => MEAN_METERS_PER_DEGREE,
        Representation::Dms => MEAN_METERS_PER_DEGREE / 3600.0,
        Representation::Ddm => MEAN_METERS_PER_DEGREE / 60.0,
        Representation::PlusCode => return None,
    };
    let places = (-(horizontal_m / meters_per_unit).log10())
        .round()
        .clamp(0.0, 8.0);
    Some(places as u8)
}

fn render(coord: &Coordinate, options: &FormatOptions) -> Result<String> {
    let comma = options.uses_decimal_comma();
    // Per-representation default precision when none is given: DD 6 (~0.11 m),
    // DMS seconds 2, DDM minutes 3. Plus Code ignores precision entirely.
    match options.representation {
        Representation::DecimalDegrees => {
            let p = options.precision.unwrap_or(6);
            let lat = render_dd(coord.lat(), Axis::Latitude, options, p, comma);
            let lon = render_dd(coord.lon(), Axis::Longitude, options, p, comma);
            // A comma decimal separator collides with a ", " list separator, so
            // switch to whitespace (which the text parser splits on) there.
            let sep = if comma { " " } else { ", " };
            Ok(format!("{lat}{sep}{lon}"))
        }
        Representation::Dms => {
            let p = options.precision.unwrap_or(2);
            render_angle_pair(coord, options, p, comma, AngleKind::Dms)
        }
        Representation::Ddm => {
            let p = options.precision.unwrap_or(3);
            render_angle_pair(coord, options, p, comma, AngleKind::Ddm)
        }
        Representation::PlusCode => {
            Ok(PlusCode::encode(*coord, PLUS_CODE_FORMAT_LENGTH)?.to_string())
        }
    }
}

fn render_dd(value: f64, axis: Axis, options: &FormatOptions, p: u8, comma: bool) -> String {
    let prec = usize::from(p);
    match options.hemisphere_style {
        HemisphereStyle::Signed => decimalize(&format!("{value:.prec$}"), comma),
        HemisphereStyle::Cardinal => {
            let magnitude = decimalize(&format!("{:.prec$}", value.abs()), comma);
            format!("{magnitude} {}", cardinal_letter(axis, value))
        }
    }
}

fn render_angle_pair(
    coord: &Coordinate,
    options: &FormatOptions,
    p: u8,
    comma: bool,
    kind: AngleKind,
) -> Result<String> {
    let lat = render_angle(coord.lat(), Axis::Latitude, options, p, comma, kind)?;
    let lon = render_angle(coord.lon(), Axis::Longitude, options, p, comma, kind)?;
    Ok(format!("{lat} {lon}"))
}

fn render_angle(
    value: f64,
    axis: Axis,
    options: &FormatOptions,
    p: u8,
    comma: bool,
    kind: AngleKind,
) -> Result<String> {
    let (deg_sym, min_sym, sec_sym) = symbols(options.symbol_style);
    let core = match kind {
        AngleKind::Dms => {
            let dms = Dms::from_degrees(value, axis)?;
            let (deg, min, sec) = round_carry_dms(dms.degrees(), dms.minutes(), dms.seconds(), p);
            let sec_str = decimalize(&pad_two(sec, p), comma);
            format!("{deg}{deg_sym}{min:02}{min_sym}{sec_str}{sec_sym}")
        }
        AngleKind::Ddm => {
            let ddm = Ddm::from_degrees(value, axis)?;
            let (deg, min) = round_carry_ddm(ddm.degrees(), ddm.minutes(), p);
            let min_str = decimalize(&pad_two(min, p), comma);
            format!("{deg}{deg_sym}{min_str}{min_sym}")
        }
    };
    Ok(apply_sign(core, axis, value, options.hemisphere_style))
}

/// Round DMS seconds to `p` places, carrying 60″ → minute and 60′ → degree so a
/// boundary value renders `1°00′00″`, never `0°59′60″` or `0°60′00″`.
fn round_carry_dms(degrees: u16, minutes: u8, seconds: f64, p: u8) -> (u16, u8, f64) {
    let factor = 10f64.powi(i32::from(p));
    let mut sec = (seconds * factor).round() / factor;
    let mut min = minutes;
    let mut deg = degrees;
    if sec >= 60.0 {
        sec -= 60.0;
        min += 1;
    }
    if min >= 60 {
        min -= 60;
        deg += 1;
    }
    (deg, min, sec)
}

/// Round DDM minutes to `p` places, carrying 60′ → degree.
fn round_carry_ddm(degrees: u16, minutes: f64, p: u8) -> (u16, f64) {
    let factor = 10f64.powi(i32::from(p));
    let mut min = (minutes * factor).round() / factor;
    let mut deg = degrees;
    if min >= 60.0 {
        min -= 60.0;
        deg += 1;
    }
    (deg, min)
}

/// Format a non-negative value with a 2-digit integer part and `p` decimals
/// (`5.5` at `p=3` → `05.500`; `46` at `p=0` → `46`).
fn pad_two(value: f64, p: u8) -> String {
    let prec = usize::from(p);
    let width = if p > 0 { prec + 3 } else { 2 };
    format!("{value:0width$.prec$}")
}

/// Apply the hemisphere: a trailing cardinal letter, or a leading `-` for the
/// signed style. `-0.0` is treated as positive (no sign / N or E).
fn apply_sign(core: String, axis: Axis, value: f64, style: HemisphereStyle) -> String {
    match style {
        HemisphereStyle::Cardinal => format!("{core}{}", cardinal_letter(axis, value)),
        HemisphereStyle::Signed if value < 0.0 => format!("-{core}"),
        HemisphereStyle::Signed => core,
    }
}

/// The cardinal letter for a signed value on an axis (`-0.0` → N / E).
fn cardinal_letter(axis: Axis, value: f64) -> char {
    match (axis, value >= 0.0) {
        (Axis::Latitude, true) => 'N',
        (Axis::Latitude, false) => 'S',
        (Axis::Longitude, true) => 'E',
        (Axis::Longitude, false) => 'W',
    }
}

/// The `(degree, minute, second)` glyphs for a symbol style. The degree sign
/// `°` is kept even for ASCII (it has no ASCII equivalent in common use).
fn symbols(style: SymbolStyle) -> (&'static str, &'static str, &'static str) {
    match style {
        SymbolStyle::Unicode => ("°", "′", "″"),
        SymbolStyle::Ascii => ("°", "'", "\""),
        SymbolStyle::Letters => ("d", "m", "s"),
    }
}

/// Whether a BCP-47 locale's primary language conventionally uses a decimal
/// comma. A small allowlist — full locale-aware number formatting is out of
/// scope (it would pull in a heavy i18n dependency).
fn uses_decimal_comma(locale: Option<&str>) -> bool {
    let Some(tag) = locale else { return false };
    let primary = tag.split(['-', '_']).next().unwrap_or_default();
    matches!(
        primary.to_ascii_lowercase().as_str(),
        "de" | "fr"
            | "es"
            | "it"
            | "ru"
            | "nl"
            | "pl"
            | "pt"
            | "sv"
            | "da"
            | "fi"
            | "cs"
            | "tr"
            | "hu"
            | "ro"
            | "nb"
            | "nn"
            | "uk"
            | "bg"
            | "hr"
            | "sk"
            | "sl"
            | "lt"
            | "lv"
            | "ca"
            | "is"
            | "et"
    )
}

/// Replace the decimal point with a comma in an already-formatted number, when
/// the locale calls for it. Safe because the input contains a single `.`.
fn decimalize(numeric: &str, comma: bool) -> String {
    if comma {
        numeric.replace('.', ",")
    } else {
        numeric.to_owned()
    }
}

#[cfg(test)]
mod tests;
