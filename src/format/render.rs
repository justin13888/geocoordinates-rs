//! Rendering internals for the angle representations (DD / DMS / DDM).
//!
//! Every renderer rounds the magnitude first and picks the sign or hemisphere
//! from the rounded result, so a value that rounds to zero renders unsigned
//! (and N / E) and re-parses to the same rendering (C11). The sign bit alone
//! decides nothing: `-0.0` and every negative value that rounds to zero have
//! an all-zero rendering, so they are unsigned.

use super::{DecimalSeparator, HemisphereStyle, SymbolStyle};
use crate::angle::{Axis, Ddm, Dms};
use crate::coord::Coordinate;
use crate::error::Result;

/// Render a coordinate as decimal degrees with `p` decimals.
pub(super) fn dd(
    coord: &Coordinate,
    p: u8,
    hemisphere: HemisphereStyle,
    separator: DecimalSeparator,
) -> String {
    let lat = dd_component(coord.lat(), Axis::Latitude, p, hemisphere, separator);
    let lon = dd_component(coord.lon(), Axis::Longitude, p, hemisphere, separator);
    format!("{lat}{}{lon}", separator.list_separator())
}

fn dd_component(
    value: f64,
    axis: Axis,
    p: u8,
    hemisphere: HemisphereStyle,
    separator: DecimalSeparator,
) -> String {
    let prec = usize::from(p);
    let magnitude = format!("{:.prec$}", value.abs());
    let negative =
        value.is_sign_negative() && magnitude.bytes().any(|b| b.is_ascii_digit() && b != b'0');
    let digits = separator.apply(&magnitude);
    match hemisphere {
        HemisphereStyle::Signed if negative => format!("-{digits}"),
        HemisphereStyle::Signed => digits,
        HemisphereStyle::Cardinal => format!("{digits} {}", cardinal_letter(axis, negative)),
    }
}

/// Which angle encoding [`angle_pair`] emits.
#[derive(Clone, Copy)]
pub(super) enum AngleKind {
    Dms,
    Ddm,
}

/// Render a coordinate as a DMS or DDM pair, the last unit with `p` decimals.
pub(super) fn angle_pair(
    coord: &Coordinate,
    kind: AngleKind,
    p: u8,
    symbols: SymbolStyle,
    hemisphere: HemisphereStyle,
    separator: DecimalSeparator,
) -> Result<String> {
    let style = AngleStyle {
        kind,
        p,
        symbols,
        hemisphere,
        separator,
    };
    let lat = style.render(coord.lat(), Axis::Latitude)?;
    let lon = style.render(coord.lon(), Axis::Longitude)?;
    Ok(format!("{lat} {lon}"))
}

/// The settings of one DMS / DDM rendering.
struct AngleStyle {
    kind: AngleKind,
    p: u8,
    symbols: SymbolStyle,
    hemisphere: HemisphereStyle,
    separator: DecimalSeparator,
}

impl AngleStyle {
    fn render(&self, value: f64, axis: Axis) -> Result<String> {
        let (deg_sym, min_sym, sec_sym) = glyphs(self.symbols);
        let (core, is_zero) = match self.kind {
            AngleKind::Dms => {
                let dms = Dms::from_degrees(value, axis)?;
                let (deg, min, sec) =
                    round_carry_dms(dms.degrees(), dms.minutes(), dms.seconds(), self.p);
                let sec_str = self.separator.apply(&pad_two(sec, self.p));
                (
                    format!("{deg}{deg_sym}{min:02}{min_sym}{sec_str}{sec_sym}"),
                    deg == 0 && min == 0 && sec == 0.0,
                )
            }
            AngleKind::Ddm => {
                let ddm = Ddm::from_degrees(value, axis)?;
                let (deg, min) = round_carry_ddm(ddm.degrees(), ddm.minutes(), self.p);
                let min_str = self.separator.apply(&pad_two(min, self.p));
                (
                    format!("{deg}{deg_sym}{min_str}{min_sym}"),
                    deg == 0 && min == 0.0,
                )
            }
        };
        let negative = value.is_sign_negative() && !is_zero;
        Ok(match self.hemisphere {
            HemisphereStyle::Cardinal => format!("{core}{}", cardinal_letter(axis, negative)),
            HemisphereStyle::Signed if negative => format!("-{core}"),
            HemisphereStyle::Signed => core,
        })
    }
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

/// The cardinal letter for a component whose rounded rendering is (or is not)
/// negative.
fn cardinal_letter(axis: Axis, negative: bool) -> char {
    match (axis, negative) {
        (Axis::Latitude, false) => 'N',
        (Axis::Latitude, true) => 'S',
        (Axis::Longitude, false) => 'E',
        (Axis::Longitude, true) => 'W',
    }
}

/// The `(degree, minute, second)` glyphs for a symbol style. The degree sign
/// `°` is kept even for ASCII (it has no ASCII equivalent in common use).
fn glyphs(style: SymbolStyle) -> (&'static str, &'static str, &'static str) {
    match style {
        SymbolStyle::Unicode => ("°", "′", "″"),
        SymbolStyle::Ascii => ("°", "'", "\""),
        SymbolStyle::Letters => ("d", "m", "s"),
    }
}
