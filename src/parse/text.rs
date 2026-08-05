//! Tolerant free-text and DMS/DDM coordinate parsing.
//!
//! Handles the hairiest input: signed decimal (`40.7128, -74.006`), DMS with
//! assorted symbols (`°'"`, Unicode primes `′″`, bare spaces), hemisphere as
//! prefix *or* suffix, DDM, and concatenated forms (`4042.766N`).
//!
//! Hard problems handled explicitly:
//! - **Axis-order ambiguity** (`40, -74` — lat,lon or lon,lat?): resolved with
//!   range heuristics plus a configurable default, reporting confidence.
//! - **Locale**: a European decimal comma (`40,7128`) collides with the list
//!   separator.

use super::AxisOrder;
use crate::angle::{Axis, Hemisphere};
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::fix::{Confidence, Fix, RawSource};

/// Options controlling tolerant parsing.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextParseOptions {
    /// Axis order to assume when range heuristics are inconclusive.
    pub default_axis_order: AxisOrder,
    /// Whether to interpret `,` as a decimal separator (European locales).
    pub decimal_comma: bool,
}

impl Default for TextParseOptions {
    fn default() -> Self {
        Self {
            default_axis_order: AxisOrder::LatLon,
            decimal_comma: false,
        }
    }
}

/// Parse a free-text coordinate with default options.
///
/// The returned [`Fix`] records the assumed axis order and parse confidence in
/// its [`RawSource`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input cannot be interpreted.
pub fn parse(input: &str) -> Result<Fix> {
    parse_with(input, &TextParseOptions::default())
}

/// Parse a free-text coordinate with explicit options.
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input cannot be interpreted.
pub fn parse_with(input: &str, options: &TextParseOptions) -> Result<Fix> {
    let normalized = normalize(input, options);
    let (first, second) = split_two(&normalized, options.decimal_comma)
        .ok_or_else(|| Error::Parse(format!("could not split into two components: {input}")))?;
    let a = parse_component(&first)
        .ok_or_else(|| Error::Parse(format!("could not parse component: {first}")))?;
    let b = parse_component(&second)
        .ok_or_else(|| Error::Parse(format!("could not parse component: {second}")))?;

    let resolved = resolve(a, b, options.default_axis_order)?;
    let coord = Coordinate::wgs84(resolved.lat, resolved.lon);
    coord.validate().map_err(|_| {
        Error::Parse(format!(
            "coordinate out of range: {}, {}",
            resolved.lat, resolved.lon
        ))
    })?;

    Ok(Fix {
        coord,
        accuracy: None,
        timestamp: None,
        source: Some(RawSource {
            raw: input.to_string(),
            confidence: Confidence::new(resolved.confidence),
            axis_order: resolved.axis_order,
            datum_ambiguity: None,
            notes: resolved.notes,
        }),
    })
}

// ===========================================================================
// Parsing internals
// ===========================================================================

/// Confidence when both hemisphere letters are present (axis order is certain).
const CONFIDENCE_EXPLICIT: f64 = 1.0;
/// Confidence when one hemisphere letter, or the `|v| > 90` range rule,
/// determines the axis order.
const CONFIDENCE_RESOLVED: f64 = 0.9;
/// Confidence when both values are in latitude range and the order was assumed
/// from `default_axis_order`.
const CONFIDENCE_ASSUMED: f64 = 0.7;

/// One parsed component: an unsigned magnitude in degrees, its sign, and the
/// axis a hemisphere letter pinned it to (if any).
struct Component {
    magnitude: f64,
    negative: bool,
    axis: Option<Axis>,
}

impl Component {
    fn signed(&self) -> f64 {
        if self.negative {
            -self.magnitude
        } else {
            self.magnitude
        }
    }
}

/// The resolved lat/lon plus how the axis order was decided.
struct Resolved {
    lat: f64,
    lon: f64,
    axis_order: Option<AxisOrder>,
    confidence: f64,
    notes: Vec<String>,
}

/// Canonicalize typography: Unicode primes → `'`/`"`, masculine ordinal → `°`,
/// exotic spaces → ASCII space; apply the decimal-comma rewrite when requested.
fn normalize(input: &str, options: &TextParseOptions) -> String {
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
    if options.decimal_comma {
        rewrite_decimal_comma(&mapped)
    } else {
        mapped
    }
}

/// Replace a comma that sits between two digits with a decimal point, so the
/// remaining commas / whitespace act as list separators.
fn rewrite_decimal_comma(s: &str) -> String {
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
/// comma, else by hemisphere-letter boundary, else by whitespace into two.
fn split_two(s: &str, decimal_comma: bool) -> Option<(String, String)> {
    if !decimal_comma {
        if let Some((a, b)) = s.split_once(',') {
            if !a.trim().is_empty() && !b.trim().is_empty() {
                return Some((a.trim().to_string(), b.trim().to_string()));
            }
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
fn is_isolated(chars: &[char], i: usize) -> bool {
    let before = i == 0 || !chars[i - 1].is_ascii_alphabetic();
    let after = i + 1 >= chars.len() || !chars[i + 1].is_ascii_alphabetic();
    before && after
}

/// Parse one component into a magnitude, sign, and (optional) axis.
fn parse_component(component: &str) -> Option<Component> {
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
            return None;
        }
        negative = true;
    }
    let magnitude = parse_magnitude(body)?;
    Some(Component {
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
/// (`deg min sec`), or concatenated NMEA (`DDMM.mmm` / `DDDMM.mmm`).
fn parse_magnitude(body: &str) -> Option<f64> {
    let fields: Vec<&str> = body
        .split(|c: char| matches!(c, '°' | '\'' | '"') || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect();
    match fields.as_slice() {
        [single] => {
            let value: f64 = single.parse().ok()?;
            if !value.is_finite() || value < 0.0 {
                return None;
            }
            if integer_digits(single) >= 4 {
                nmea_decode(value) // concatenated DDMM.mmm
            } else {
                Some(value)
            }
        }
        [deg, min] => components_to_degrees(deg, min, None),
        [deg, min, sec] => components_to_degrees(deg, min, Some(sec)),
        _ => None,
    }
}

fn components_to_degrees(deg: &str, min: &str, sec: Option<&&str>) -> Option<f64> {
    let degrees = deg.parse::<f64>().ok()?;
    let minutes = min.parse::<f64>().ok()?;
    let seconds = sec.map_or(Some(0.0), |s| s.parse::<f64>().ok())?;
    if !degrees.is_finite()
        || degrees < 0.0
        || !minutes.is_finite()
        || !(0.0..60.0).contains(&minutes)
        || !seconds.is_finite()
        || !(0.0..60.0).contains(&seconds)
    {
        return None;
    }
    Some(degrees + minutes / 60.0 + seconds / 3600.0)
}

/// Count the digits before the decimal point (leading sign stripped).
fn integer_digits(s: &str) -> usize {
    s.trim_start_matches(['+', '-'])
        .split('.')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .count()
}

/// Decode a concatenated NMEA value: `DDMM.mmm` → decimal degrees.
fn nmea_decode(value: f64) -> Option<f64> {
    let degrees = (value / 100.0).trunc();
    let minutes = value - degrees * 100.0;
    (minutes < 60.0).then_some(degrees + minutes / 60.0)
}

/// Resolve which component is latitude vs longitude, recording how it was
/// decided and a confidence.
fn resolve(a: Component, b: Component, default_order: AxisOrder) -> Result<Resolved> {
    let a_val = a.signed();
    let b_val = b.signed();

    // Both axes pinned by hemisphere letters.
    if let (Some(ax), Some(bx)) = (a.axis, b.axis) {
        return match (ax, bx) {
            (Axis::Latitude, Axis::Longitude) => Ok(explicit(a_val, b_val)),
            (Axis::Longitude, Axis::Latitude) => Ok(explicit(b_val, a_val)),
            _ => Err(Error::Parse(
                "two hemisphere letters name the same axis".to_string(),
            )),
        };
    }

    // Exactly one axis pinned by a hemisphere letter; the other is its complement.
    match (a.axis, b.axis) {
        (Some(Axis::Latitude), None) => return Ok(one_letter(a_val, b_val, AxisOrder::LatLon)),
        (Some(Axis::Longitude), None) => return Ok(one_letter(b_val, a_val, AxisOrder::LonLat)),
        (None, Some(Axis::Latitude)) => return Ok(one_letter(b_val, a_val, AxisOrder::LonLat)),
        (None, Some(Axis::Longitude)) => return Ok(one_letter(a_val, b_val, AxisOrder::LatLon)),
        _ => {}
    }

    // No hemisphere letters: range rule, then the configured default.
    let a_lon_only = a_val.abs() > 90.0;
    let b_lon_only = b_val.abs() > 90.0;
    match (a_lon_only, b_lon_only) {
        (true, true) => Err(Error::Parse(
            "both components exceed the latitude range".to_string(),
        )),
        (true, false) => Ok(range_forced(b_val, a_val, AxisOrder::LonLat)),
        (false, true) => Ok(range_forced(a_val, b_val, AxisOrder::LatLon)),
        (false, false) => Ok(match default_order {
            AxisOrder::LatLon => assumed(a_val, b_val, AxisOrder::LatLon),
            AxisOrder::LonLat => assumed(b_val, a_val, AxisOrder::LonLat),
        }),
    }
}

fn explicit(lat: f64, lon: f64) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: None,
        confidence: CONFIDENCE_EXPLICIT,
        notes: Vec::new(),
    }
}

fn one_letter(lat: f64, lon: f64, _order: AxisOrder) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: None, // a hemisphere letter fixes the axis; nothing was assumed
        confidence: CONFIDENCE_RESOLVED,
        notes: Vec::new(),
    }
}

fn range_forced(lat: f64, lon: f64, order: AxisOrder) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: Some(order),
        confidence: CONFIDENCE_RESOLVED,
        notes: vec!["axis order forced by the |value| > 90 rule".to_string()],
    }
}

fn assumed(lat: f64, lon: f64, order: AxisOrder) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: Some(order),
        confidence: CONFIDENCE_ASSUMED,
        notes: vec!["axis order assumed from the default".to_string()],
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    fn source(fix: &Fix) -> &RawSource {
        fix.source.as_ref().expect("text parse records a source")
    }

    #[test]
    fn dd_comma_defaults_to_latlon() {
        let fix = parse("40.7128, -74.006").unwrap();
        assert_close(fix.coord.lat, 40.7128, 1e-9);
        assert_close(fix.coord.lon, -74.006, 1e-9);
        let s = source(&fix);
        // Both magnitudes are in latitude range, so the order is assumed.
        assert_close(s.confidence.value(), 0.7, 1e-12);
        assert_eq!(s.axis_order, Some(AxisOrder::LatLon));
    }

    #[test]
    fn range_rule_forces_longitude() {
        // 151.2 exceeds the latitude range, so it must be longitude (Sydney).
        let fix = parse("151.2, -33.9").unwrap();
        assert_close(fix.coord.lat, -33.9, 1e-9);
        assert_close(fix.coord.lon, 151.2, 1e-9);
        let s = source(&fix);
        assert_close(s.confidence.value(), 0.9, 1e-12);
        assert_eq!(s.axis_order, Some(AxisOrder::LonLat));
    }

    #[test]
    fn dms_with_both_hemispheres() {
        let fix = parse("40°42′46″N 74°00′22″W").unwrap();
        assert_close(fix.coord.lat, 40.0 + 42.0 / 60.0 + 46.0 / 3600.0, 1e-9);
        assert_close(fix.coord.lon, -(74.0 + 22.0 / 3600.0), 1e-9);
        let s = source(&fix);
        assert_close(s.confidence.value(), 1.0, 1e-12);
        assert_eq!(s.axis_order, None);
    }

    #[test]
    fn hemispheres_can_be_swapped() {
        let fix = parse("74°00′22″W 40°42′46″N").unwrap();
        assert_close(fix.coord.lat, 40.0 + 42.0 / 60.0 + 46.0 / 3600.0, 1e-9);
        assert_close(fix.coord.lon, -(74.0 + 22.0 / 3600.0), 1e-9);
        assert_close(source(&fix).confidence.value(), 1.0, 1e-12);
    }

    #[test]
    fn ascii_symbols_and_prefix_hemispheres() {
        // ASCII quotes plus leading hemisphere letters.
        let fix = parse("N40 42'46\" W74 00'22\"").unwrap();
        assert_close(fix.coord.lat, 40.0 + 42.0 / 60.0 + 46.0 / 3600.0, 1e-9);
        assert_close(fix.coord.lon, -(74.0 + 22.0 / 3600.0), 1e-9);
    }

    #[test]
    fn nmea_concatenated() {
        let fix = parse("4042.766N 07400.456W").unwrap();
        assert_close(fix.coord.lat, 40.0 + 42.766 / 60.0, 1e-9);
        assert_close(fix.coord.lon, -(74.0 + 0.456 / 60.0), 1e-9);
        assert_eq!(source(&fix).axis_order, None);
    }

    #[test]
    fn space_separated_ddm_with_suffix_hemispheres() {
        let fix = parse("40 42.766 N 74 0.456 W").unwrap();
        assert_close(fix.coord.lat, 40.0 + 42.766 / 60.0, 1e-9);
        assert_close(fix.coord.lon, -(74.0 + 0.456 / 60.0), 1e-9);
    }

    #[test]
    fn decimal_comma_option() {
        let opts = TextParseOptions {
            decimal_comma: true,
            default_axis_order: AxisOrder::LatLon,
        };
        let fix = parse_with("40,7128 -74,006", &opts).unwrap();
        assert_close(fix.coord.lat, 40.7128, 1e-9);
        assert_close(fix.coord.lon, -74.006, 1e-9);
        // The same string without the option is a different (failing) parse.
        assert!(parse("40,7128 -74,006").is_err());
    }

    #[test]
    fn default_lonlat_swaps_assignment() {
        let opts = TextParseOptions {
            decimal_comma: false,
            default_axis_order: AxisOrder::LonLat,
        };
        let fix = parse_with("-74.006, 40.7128", &opts).unwrap();
        assert_close(fix.coord.lon, -74.006, 1e-9);
        assert_close(fix.coord.lat, 40.7128, 1e-9);
        assert_eq!(source(&fix).axis_order, Some(AxisOrder::LonLat));
    }

    #[test]
    fn invalid_inputs_error() {
        assert!(parse("200, 0").is_err()); // forced longitude 200 > 180
        assert!(parse("100, 150").is_err()); // both exceed the latitude range
        assert!(parse("hello world").is_err()); // not numeric
        assert!(parse("42").is_err()); // only one component
        assert!(parse("").is_err()); // empty
        assert!(parse("40 60 N 74 0 W").is_err()); // invalid minutes
        assert!(parse("40 0 60 N 74 0 0 W").is_err()); // invalid seconds
        assert!(parse("4060.0N 07400.0W").is_err()); // invalid NMEA minutes
        assert!(parse("-40N 74W").is_err()); // sign contradicts hemisphere
        assert!(parse("NaN, 0").is_err());
    }

    #[test]
    fn south_and_east_hemispheres() {
        let fix = parse("33.9S 151.2E").unwrap();
        assert_close(fix.coord.lat, -33.9, 1e-9);
        assert_close(fix.coord.lon, 151.2, 1e-9);
    }

    #[test]
    fn one_hemisphere_letter_resolves_the_pair() {
        // (lat-letter, none): N pins the first component as latitude.
        let f = parse("40.7N 74.006").unwrap();
        assert_close(f.coord.lat, 40.7, 1e-9);
        assert_close(f.coord.lon, 74.006, 1e-9);
        let s = source(&f);
        assert_close(s.confidence.value(), 0.9, 1e-12);
        assert_eq!(s.axis_order, None);

        // (lon-letter, none): W pins the first as longitude → swap.
        let f = parse("74.006W 40.7").unwrap();
        assert_close(f.coord.lat, 40.7, 1e-9);
        assert_close(f.coord.lon, -74.006, 1e-9);

        // (none, lon-letter): W pins the second as longitude. The lat/lon
        // happen to coincide with the assumed-default path, so assert the
        // confidence and axis-order too (the letter resolves it, 0.9 / None).
        let f = parse("40.7 74.006W").unwrap();
        assert_close(f.coord.lat, 40.7, 1e-9);
        assert_close(f.coord.lon, -74.006, 1e-9);
        let s = source(&f);
        assert_close(s.confidence.value(), 0.9, 1e-12);
        assert_eq!(s.axis_order, None);

        // (none, lat-letter): N pins the second as latitude → swap.
        let f = parse("74.006 40.7N").unwrap();
        assert_close(f.coord.lat, 40.7, 1e-9);
        assert_close(f.coord.lon, 74.006, 1e-9);
    }

    #[test]
    fn range_rule_boundary_excludes_exactly_90() {
        // Exactly 90 is a valid latitude, so the order stays *assumed* (0.7),
        // not forced by the range rule.
        let f = parse("90, 50").unwrap();
        assert_close(f.coord.lat, 90.0, 1e-9);
        assert_close(f.coord.lon, 50.0, 1e-9);
        assert_close(source(&f).confidence.value(), 0.7, 1e-12);

        let f = parse("50, 90").unwrap();
        assert_close(f.coord.lat, 50.0, 1e-9);
        assert_close(f.coord.lon, 90.0, 1e-9);
        assert_close(source(&f).confidence.value(), 0.7, 1e-12);
    }

    #[test]
    fn rewrite_decimal_comma_only_between_digits() {
        assert_eq!(rewrite_decimal_comma("40,7128"), "40.7128");
        assert_eq!(rewrite_decimal_comma("a,5"), "a,5"); // previous char not a digit
        assert_eq!(rewrite_decimal_comma("5,a"), "5,a"); // next char not a digit
        assert_eq!(rewrite_decimal_comma(",5"), ",5"); // leading comma (no previous)
        assert_eq!(rewrite_decimal_comma("5,"), "5,"); // trailing comma (no next)
    }

    #[test]
    fn split_two_rejects_half_empty_comma_split() {
        // "5," splits to ("5", "") on the comma; the empty half must not be
        // returned as a component pair.
        assert!(split_two("5,", false).is_none());
        assert_eq!(
            split_two("40, -74", false),
            Some(("40".to_string(), "-74".to_string()))
        );
    }

    #[test]
    fn is_isolated_excludes_word_letters() {
        let suffixed: Vec<char> = "46\"N".chars().collect();
        assert!(is_isolated(&suffixed, 3)); // N after a quote
        let word: Vec<char> = "North".chars().collect();
        assert!(!is_isolated(&word, 0)); // N before 'o'
        let flanked: Vec<char> = "aNa".chars().collect();
        assert!(!is_isolated(&flanked, 1)); // N between letters
    }
}
