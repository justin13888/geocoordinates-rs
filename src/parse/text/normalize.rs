//! Typography normalization, component splitting, and per-component lexing for
//! the free-text parser.

use super::resolve::Component;
use crate::angle::{Axis, Hemisphere};
use crate::error::ParseErrorKind;

/// Canonicalize typography: Unicode primes → `'`/`"`, masculine ordinal → `°`,
/// exotic spaces → ASCII space; apply the decimal-comma rewrite when requested,
/// then the letter-marker rewrite (`10d30m00s` → `10°30'00"`).
pub(super) fn normalize(input: &str, decimal_comma: bool) -> String {
    let mapped: String = input
        .trim()
        .chars()
        .map(|c| match c {
            '\u{2032}' | '\u{2019}' => '\'',             // ′ ’ → '
            '\u{2033}' | '\u{201D}' => '"',              // ″ ” → "
            '\u{00BA}' => '\u{00B0}',                    // º → °
            '\u{00A0}' | '\u{2007}' | '\u{202F}' => ' ', // NBSP / figure / narrow → space
            other => other,
        })
        .collect();
    let decimals = if decimal_comma {
        rewrite_decimal_comma(&mapped)
    } else {
        mapped
    };
    rewrite_unit_letters(&decimals)
}

/// Rewrite letter unit markers (`d` / `D`, `m` / `M`, and lowercase `s`) into
/// `°`, `'`, `"`, so letter-style DMS / DDM (`10d30m00.00sN`, `-20d15.000m`)
/// parses like the symbol forms.
///
/// Only the anchored sequence `<digits>d<number>[m[<number>s]]` is rewritten: a
/// degree `d` must sit between two digits, a minute `m` must directly follow
/// the minutes number, and a second `s` must directly follow a seconds number.
/// That anchoring keeps a hemisphere suffix intact — `S` in `10d30mS` (no
/// seconds number) or in `33.9s` (no degree marker) stays a hemisphere letter.
///
/// The seconds marker is lowercase `s`. An uppercase `S` after a seconds number
/// is the marker only when an uppercase hemisphere letter follows it directly
/// (`10D30M15SN`, `10D30M15SS`); otherwise (`10d30m15S`) it is the South
/// hemisphere, never a unit. Reading a trailing `S` as a unit would drop the
/// hemisphere and silently resolve the value as North.
pub(super) fn rewrite_unit_letters(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = chars.clone();
    for i in 1..chars.len() {
        let degree_marker = matches!(chars[i], 'd' | 'D')
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if !degree_marker {
            continue;
        }
        out[i] = '°';
        let minutes_end = number_end(&chars, i + 1);
        if !matches!(chars.get(minutes_end), Some('m' | 'M')) {
            continue;
        }
        out[minutes_end] = '\'';
        let seconds_end = number_end(&chars, minutes_end + 1);
        let seconds_marker = match chars.get(seconds_end) {
            Some('s') => true,
            Some('S') => matches!(chars.get(seconds_end + 1), Some('N' | 'S' | 'E' | 'W')),
            _ => false,
        };
        if seconds_end > minutes_end + 1 && seconds_marker {
            out[seconds_end] = '"';
        }
    }
    out.into_iter().collect()
}

/// Index one past the run of digits and `.` starting at `start`.
fn number_end(chars: &[char], start: usize) -> usize {
    chars[start..]
        .iter()
        .position(|c| !(c.is_ascii_digit() || *c == '.'))
        .map_or(chars.len(), |len| start + len)
}

/// Replace a comma that sits between two digits with a decimal point, so the
/// remaining commas / whitespace act as list separators.
pub(super) fn rewrite_decimal_comma(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        let between_digits = c == ','
            && i > 0
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        out.push(if between_digits { '.' } else { c });
    }
    out
}

/// Split a normalized string into two component substrings: by an explicit
/// list separator, else by hemisphere-letter boundary, else by whitespace into
/// two.
///
/// The list separator is `,`; with a decimal comma (already rewritten to `.`
/// between digits by [`rewrite_decimal_comma`]) it is the first remaining `,`
/// or `;`.
pub(super) fn split_two(s: &str, decimal_comma: bool) -> Option<(String, String)> {
    let list_separator = |c: char| c == ',' || (decimal_comma && c == ';');
    if let Some((a, b)) = s.split_once(list_separator) {
        if !a.trim().is_empty() && !b.trim().is_empty() {
            return Some((a.trim().to_string(), b.trim().to_string()));
        }
    }
    if let Some(pair) = split_on_hemisphere(s) {
        return Some(pair);
    }
    let tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.len() == 2 {
        return Some((tokens[0].to_string(), tokens[1].to_string()));
    }
    None
}

/// Split around two isolated hemisphere letters (prefix or suffix style).
fn split_on_hemisphere(s: &str) -> Option<(String, String)> {
    let chars: Vec<char> = s.chars().collect();
    let positions: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|&(i, &c)| hemi_from_char(c).is_some() && is_isolated(&chars, i))
        .map(|(i, _)| i)
        .collect();
    if positions.len() != 2 {
        return None;
    }
    // Suffix style if the first letter trails a value (digit / symbol); else it
    // is a prefix leading the next value. Checking what precedes the letter
    // avoids mistaking the *second* component's leading digit for a prefix.
    let first_is_suffix = chars[..positions[0]]
        .iter()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, '°' | '\'' | '"' | '.'));
    let cut = if first_is_suffix {
        positions[0] + 1 // split after the first (suffix) letter
    } else {
        positions[1] // split before the second (prefix) letter
    };
    let first: String = chars[..cut].iter().collect();
    let second: String = chars[cut..].iter().collect();
    // An empty component (after trimming) is rejected downstream by
    // `parse_component`, so no defensive check is needed here.
    Some((first.trim().to_string(), second.trim().to_string()))
}

/// Whether the char at `i` is not flanked by other ASCII letters (so `N` in
/// `46″N` counts, but not the `N` in `North`).
pub(super) fn is_isolated(chars: &[char], i: usize) -> bool {
    let before = i == 0 || !chars[i - 1].is_ascii_alphabetic();
    let after = i + 1 >= chars.len() || !chars[i + 1].is_ascii_alphabetic();
    before && after
}

/// Parse one component into a magnitude, sign, and (optional) axis, or the
/// kind of its rejection.
pub(super) fn parse_component(component: &str) -> Result<Component, ParseErrorKind> {
    let (body, hemi) = extract_hemisphere(component.trim());
    let mut negative = false;
    let mut axis = None;
    if let Some(h) = hemi {
        axis = Some(hemi_axis(h));
        negative = matches!(h, Hemisphere::South | Hemisphere::West);
    }
    let body = body.trim();
    let (body, signed_negative) = match body.strip_prefix('-') {
        Some(rest) => (rest.trim_start(), true),
        None => (body.strip_prefix('+').map_or(body, str::trim_start), false),
    };
    if signed_negative {
        if matches!(hemi, Some(Hemisphere::North | Hemisphere::East)) {
            return Err(ParseErrorKind::Syntax);
        }
        negative = true;
    }
    let magnitude = parse_magnitude(body, hemi.is_some())?;
    Ok(Component {
        magnitude,
        negative,
        axis,
    })
}

/// Strip a single isolated hemisphere letter (suffix preferred, then prefix).
fn extract_hemisphere(s: &str) -> (&str, Option<Hemisphere>) {
    let s = s.trim();
    let chars: Vec<char> = s.chars().collect();
    if let Some(&last) = chars.last() {
        if let Some(h) = hemi_from_char(last) {
            if is_isolated(&chars, chars.len() - 1) {
                return (s[..s.len() - last.len_utf8()].trim(), Some(h));
            }
        }
    }
    if let Some(&first) = chars.first() {
        if let Some(h) = hemi_from_char(first) {
            if is_isolated(&chars, 0) {
                return (s[first.len_utf8()..].trim(), Some(h));
            }
        }
    }
    (s, None)
}

/// Parse an unsigned magnitude in degrees: plain decimal, DDM (`deg min`), DMS
/// (`deg min sec`), or — only beside a hemisphere letter — concatenated NMEA
/// (`DDMM.mmm` / `DDDMM.mmm`).
///
/// Every field must be a plain unsigned decimal (no sign, exponent, `inf` or
/// `NaN`), and only the last field may carry a fraction: `40.5 30` has no
/// single reading, so it is rejected rather than summed to 41°.
///
/// A rejection is [`ParseErrorKind::Ambiguous`] where the text has no single
/// reading (a leading fraction, exponent notation, bare `DDMM`), and
/// [`ParseErrorKind::Syntax`] otherwise.
fn parse_magnitude(body: &str, has_hemisphere: bool) -> Result<f64, ParseErrorKind> {
    let fields: Vec<&str> = body
        .split(|c: char| matches!(c, '°' | '\'' | '"') || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect();
    let (last, leading) = fields.split_last().ok_or(ParseErrorKind::Syntax)?;
    if fields.iter().any(|f| is_exponent(f)) {
        return Err(ParseErrorKind::Ambiguous);
    }
    if !fields.iter().all(|f| is_plain_decimal(f)) {
        return Err(ParseErrorKind::Syntax);
    }
    if leading.iter().any(|f| f.contains('.')) {
        return Err(ParseErrorKind::Ambiguous);
    }
    match fields.as_slice() {
        [single] => {
            let value: f64 = single.parse().map_err(|_| ParseErrorKind::Syntax)?;
            if integer_digits(single) < 4 {
                Ok(value)
            } else if has_hemisphere {
                nmea_decode(value).ok_or(ParseErrorKind::Syntax) // concatenated DDMM.mmm
            } else {
                // No degree value has four integer digits, and DDMM needs a
                // hemisphere letter to be read as one.
                Err(ParseErrorKind::Ambiguous)
            }
        }
        [deg, min] => components_to_degrees(deg, min, None).ok_or(ParseErrorKind::Syntax),
        [deg, min, _] => components_to_degrees(deg, min, Some(last)).ok_or(ParseErrorKind::Syntax),
        _ => Err(ParseErrorKind::Syntax),
    }
}

/// Whether `s` is a number in exponent notation (`4e1`, `2.5E-3`): a plain
/// decimal mantissa, an `e` / `E`, and an optionally signed integer exponent.
fn is_exponent(s: &str) -> bool {
    s.split_once(['e', 'E'])
        .is_some_and(|(mantissa, exponent)| {
            let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            is_plain_decimal(mantissa)
                && !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
        })
}

/// Whether `s` is an unsigned decimal of ASCII digits with at most one `.`.
fn is_plain_decimal(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit())
        && s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && s.matches('.').count() <= 1
}

/// Sum validated plain-decimal fields; minutes and seconds must be below 60.
fn components_to_degrees(deg: &str, min: &str, sec: Option<&str>) -> Option<f64> {
    let degrees = deg.parse::<f64>().ok()?;
    let minutes = min.parse::<f64>().ok()?;
    let seconds = sec.map_or(Some(0.0), |s| s.parse::<f64>().ok())?;
    if !(0.0..60.0).contains(&minutes) || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some(degrees + minutes / 60.0 + seconds / 3600.0)
}

/// Count the digits before the decimal point of a plain decimal.
fn integer_digits(s: &str) -> usize {
    s.split('.').next().unwrap_or_default().len()
}

/// Decode a concatenated NMEA value: `DDMM.mmm` → decimal degrees.
fn nmea_decode(value: f64) -> Option<f64> {
    let degrees = (value / 100.0).trunc();
    let minutes = value - degrees * 100.0;
    (minutes < 60.0).then_some(degrees + minutes / 60.0)
}

fn hemi_from_char(c: char) -> Option<Hemisphere> {
    match c.to_ascii_uppercase() {
        'N' => Some(Hemisphere::North),
        'S' => Some(Hemisphere::South),
        'E' => Some(Hemisphere::East),
        'W' => Some(Hemisphere::West),
        _ => None,
    }
}

fn hemi_axis(h: Hemisphere) -> Axis {
    match h {
        Hemisphere::North | Hemisphere::South => Axis::Latitude,
        Hemisphere::East | Hemisphere::West => Axis::Longitude,
    }
}
