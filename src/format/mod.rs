//! Presentation: locale-aware, round-trip-stable coordinate formatting.
//!
//! Formatting is the inverse of parsing (the `parse` module, a later release):
//! a [`Coordinate`] plus [`FormatOptions`] renders to a string in a selectable
//! representation. The guarantee is **round-trip stability** —
//! `parse → model → format → parse` must not drift.

use crate::angle::{Axis, Dd};
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::fix::Fix;
use crate::grids::PlusCode;

/// Code length used when rendering a coordinate as a Plus Code (~14 m cells).
const PLUS_CODE_FORMAT_LENGTH: usize = 10;
/// Highest useful decimal precision for an IEEE-754 `f64`.
const MAX_PRECISION: u8 = 15;

/// Target representation for rendering a coordinate.
///
/// Grid representations (Plus Code, then UTM / MGRS / geohash) are added as
/// their grid milestones ship — see `ROADMAP.md`. Exhaustive (no
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
    coord.validate()?;
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
/// # Errors
/// As [`format()`].
pub fn format_fix(fix: &Fix, options: &FormatOptions) -> Result<String> {
    if options.precision.is_some() {
        return format(&fix.coord, options);
    }
    let derived = fix
        .accuracy
        .and_then(|a| a.horizontal_m)
        .and_then(precision_for_accuracy);
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

/// Decimal places implied by a horizontal accuracy, so digits finer than the
/// fix's resolution are not printed. `None` (unusable accuracy) → caller's
/// default. One degree of latitude ≈ 111,195 m.
fn precision_for_accuracy(horizontal_m: f64) -> Option<u8> {
    if !horizontal_m.is_finite() || horizontal_m <= 0.0 {
        return None;
    }
    let places = (-(horizontal_m / 111_195.0).log10())
        .round()
        .clamp(0.0, 8.0);
    Some(places as u8)
}

fn render(coord: &Coordinate, options: &FormatOptions) -> Result<String> {
    let comma = uses_decimal_comma(options.locale.as_deref());
    // Per-representation default precision when none is given: DD 6 (~0.11 m),
    // DMS seconds 2, DDM minutes 3. Plus Code ignores precision entirely.
    match options.representation {
        Representation::DecimalDegrees => {
            let p = options.precision.unwrap_or(6);
            let lat = render_dd(coord.lat, Axis::Latitude, options, p, comma);
            let lon = render_dd(coord.lon, Axis::Longitude, options, p, comma);
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
        Representation::PlusCode => Ok(PlusCode::encode(*coord, PLUS_CODE_FORMAT_LENGTH)?
            .as_str()
            .to_string()),
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
    let lat = render_angle(coord.lat, Axis::Latitude, options, p, comma, kind)?;
    let lon = render_angle(coord.lon, Axis::Longitude, options, p, comma, kind)?;
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
            let dms = Dd(value).try_to_dms(axis)?;
            let (deg, min, sec) = round_carry_dms(dms.degrees, dms.minutes, dms.seconds, p);
            let sec_str = decimalize(&pad_two(sec, p), comma);
            format!("{deg}{deg_sym}{min:02}{min_sym}{sec_str}{sec_sym}")
        }
        AngleKind::Ddm => {
            let ddm = Dd(value).try_to_ddm(axis)?;
            let (deg, min) = round_carry_ddm(ddm.degrees, ddm.minutes, p);
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
mod tests {
    use super::*;
    use crate::Coordinate;
    use crate::fix::{Accuracy, Fix};

    fn opts(
        representation: Representation,
        precision: Option<u8>,
        symbol_style: SymbolStyle,
        hemisphere_style: HemisphereStyle,
        locale: Option<&str>,
    ) -> FormatOptions {
        FormatOptions {
            representation,
            precision,
            symbol_style,
            hemisphere_style,
            locale: locale.map(String::from),
        }
    }

    fn fmt(coord: &Coordinate, options: &FormatOptions) -> String {
        format(coord, options).expect("DD/DMS/DDM formatting is infallible")
    }

    #[test]
    fn dd_default_matches_doc() {
        let c = Coordinate::wgs84(40.7128, -74.006);
        assert_eq!(fmt(&c, &FormatOptions::default()), "40.712800, -74.006000");
    }

    #[test]
    fn dd_cardinal_and_precision() {
        let c = Coordinate::wgs84(40.7128, -74.006);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::DecimalDegrees,
                    Some(6),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Cardinal,
                    None,
                ),
            ),
            "40.712800 N, 74.006000 W"
        );
        // Precision rounds.
        let dd = |p| {
            fmt(
                &c,
                &opts(
                    Representation::DecimalDegrees,
                    Some(p),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Signed,
                    None,
                ),
            )
        };
        assert_eq!(dd(0), "41, -74");
        assert_eq!(dd(2), "40.71, -74.01");
    }

    #[test]
    fn dms_clean_and_styles() {
        let c = Coordinate::wgs84(10.5, 20.25);
        let dms = |sym, hemi| fmt(&c, &opts(Representation::Dms, Some(0), sym, hemi, None));
        assert_eq!(
            dms(SymbolStyle::Unicode, HemisphereStyle::Cardinal),
            "10°30′00″N 20°15′00″E"
        );
        assert_eq!(
            dms(SymbolStyle::Ascii, HemisphereStyle::Cardinal),
            "10°30'00\"N 20°15'00\"E"
        );
        assert_eq!(
            dms(SymbolStyle::Letters, HemisphereStyle::Cardinal),
            "10d30m00sN 20d15m00sE"
        );
    }

    #[test]
    fn dms_signed_puts_minus_on_degrees() {
        let c = Coordinate::wgs84(10.5, -20.25);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::Dms,
                    Some(0),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Signed,
                    None,
                ),
            ),
            "10°30′00″ -20°15′00″"
        );
    }

    #[test]
    fn dms_seconds_carry_rolls_up() {
        // lat ~0.9999999 -> 0°59′59.99964″, which rounds to 60″ at p=2 and must
        // carry to 1°00′00.00″ rather than render 0°59′60.00″ or 0°60′00″.
        let c = Coordinate::wgs84(0.9999999, 0.0);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::Dms,
                    Some(2),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Cardinal,
                    None,
                ),
            ),
            "1°00′00.00″N 0°00′00.00″E"
        );
    }

    #[test]
    fn ddm_clean_and_doc_example() {
        let c = Coordinate::wgs84(40.5, -74.25);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::Ddm,
                    Some(3),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Cardinal,
                    None,
                ),
            ),
            "40°30.000′N 74°15.000′W"
        );
        // The module-doc example: 40°42.766′N.
        let doc = Coordinate::wgs84(40.0 + 42.766 / 60.0, 0.0);
        let out = fmt(
            &doc,
            &opts(
                Representation::Ddm,
                Some(3),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal,
                None,
            ),
        );
        assert!(out.starts_with("40°42.766′N"), "{out}");
    }

    #[test]
    fn locale_decimal_comma() {
        let c = Coordinate::wgs84(40.7128, -74.006);
        // Comma locale: comma decimals, whitespace list separator (round-trippable).
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::DecimalDegrees,
                    Some(6),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Signed,
                    Some("de-DE"),
                ),
            ),
            "40,712800 -74,006000"
        );
        // Dot locale is the default behaviour.
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::DecimalDegrees,
                    Some(6),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Signed,
                    Some("en-US"),
                ),
            ),
            "40.712800, -74.006000"
        );
    }

    #[test]
    fn format_fix_derives_precision_from_accuracy() {
        let coord = Coordinate::wgs84(40.7128, -74.006);
        let fix = |horizontal_m: Option<f64>| Fix {
            coord,
            accuracy: Some(Accuracy {
                horizontal_m,
                vertical_m: None,
            }),
            timestamp: None,
            source: None,
        };
        let dd_default = opts(
            Representation::DecimalDegrees,
            None,
            SymbolStyle::Unicode,
            HemisphereStyle::Signed,
            None,
        );
        // ~1 km accuracy -> 2 decimals; ~1 m -> 5 decimals.
        assert_eq!(
            format_fix(&fix(Some(1000.0)), &dd_default).unwrap(),
            "40.71, -74.01"
        );
        assert_eq!(
            format_fix(&fix(Some(1.0)), &dd_default).unwrap(),
            "40.71280, -74.00600"
        );
        // Unusable accuracy (none / zero / negative) falls back to the
        // representation default (6).
        assert_eq!(
            format_fix(&fix(None), &dd_default).unwrap(),
            "40.712800, -74.006000"
        );
        assert_eq!(
            format_fix(&fix(Some(0.0)), &dd_default).unwrap(),
            "40.712800, -74.006000"
        );
        assert_eq!(
            format_fix(&fix(Some(-5.0)), &dd_default).unwrap(),
            "40.712800, -74.006000"
        );
    }

    #[test]
    fn ddm_minutes_carry_rolls_up() {
        // Minutes round to 60.000 at p=3 and must carry into the degree.
        let c = Coordinate::wgs84(0.9999999, 0.0);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::Ddm,
                    Some(3),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Cardinal,
                    None,
                ),
            ),
            "1°00.000′N 0°00.000′E"
        );
    }

    #[test]
    fn signed_zero_has_no_minus() {
        // A zero component must not gain a leading '-' in the signed style.
        let c = Coordinate::wgs84(0.0, 10.0);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::Dms,
                    Some(0),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Signed,
                    None,
                ),
            ),
            "0°00′00″ 10°00′00″"
        );
    }

    #[test]
    fn negative_zero_longitude_is_east() {
        let c = Coordinate::wgs84(0.0, -0.0);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::Dms,
                    Some(0),
                    SymbolStyle::Unicode,
                    HemisphereStyle::Cardinal,
                    None,
                ),
            ),
            "0°00′00″N 0°00′00″E"
        );
    }

    #[test]
    fn plus_code_representation() {
        let c = Coordinate::wgs84(47.0000625, 8.0000625);
        assert_eq!(
            fmt(
                &c,
                &opts(
                    Representation::PlusCode,
                    None,
                    SymbolStyle::Unicode,
                    HemisphereStyle::Signed,
                    None,
                ),
            ),
            "8FVC2222+22"
        );
    }
}
