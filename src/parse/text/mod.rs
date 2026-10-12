//! Tolerant free-text and DMS/DDM coordinate parsing.
//!
//! Handles the hairiest input: signed decimal (`40.7128, -74.006`), DMS with
//! assorted symbols (`°'"`, Unicode primes `′″`, letter markers `d m s`, bare
//! spaces), hemisphere as prefix *or* suffix, DDM, and concatenated forms
//! (`4042.766N`).
//!
//! Hard problems handled explicitly:
//! - **Axis-order ambiguity** (`40, -74` — lat,lon or lon,lat?): resolved with
//!   range heuristics plus a configurable default, reporting confidence.
//! - **Locale**: a European decimal comma (`40,7128`) collides with the list
//!   separator; [`ParseOptions::decimal_separator`] says which it is.
//!
//! Input that has no single reading is rejected rather than guessed at:
//! concatenated `DDMM.mmm` without a hemisphere letter (`1234.5` is no degree
//! value), a fraction on any field but the last (`40.5 30`), and exponent
//! notation (`4e1`).

use super::ParseOptions;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::fix::{Confidence, Fix, RawSource};
use crate::format::DecimalSeparator;

mod normalize;
mod resolve;
#[cfg(test)]
mod tests;

use normalize::{normalize, parse_component, split_two};
use resolve::resolve;

/// Parse a free-text coordinate with default options.
///
/// The returned [`Fix`] records the assumed axis order and parse confidence in
/// its [`RawSource`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input cannot be interpreted.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::parse::AxisOrder;
/// use geocoordinates::parse::text::parse;
///
/// let fix = parse("40°42′46″N 74°00′22″W")?;
/// assert!((fix.coord.lat() - 40.712_78).abs() < 1e-5);
/// assert!((fix.coord.lon() + 74.006_11).abs() < 1e-5);
///
/// // Unlabelled numbers are read lat-first; the fix says so.
/// let fix = parse("40.7128, -74.006")?;
/// assert_eq!(fix.source.and_then(|s| s.axis_order), Some(AxisOrder::LatLon));
///
/// // Concatenated DDMM.mmm needs a hemisphere letter.
/// assert!(matches!(parse("4042.768, -7400.36"), Err(Error::Parse(_))));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn parse(input: &str) -> Result<Fix> {
    parse_with(input, &ParseOptions::default())
}

/// Parse a free-text coordinate with explicit options.
///
/// Only [`ParseOptions::default_axis_order`] and
/// [`ParseOptions::decimal_separator`] apply here;
/// [`ParseOptions::expect_grid`] is read by
/// [`parse_coordinate_with`](super::parse_coordinate_with), which tries grids
/// before falling back to this parser.
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input cannot be interpreted.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::format::DecimalSeparator;
/// use geocoordinates::parse::ParseOptions;
/// use geocoordinates::parse::text::parse_with;
///
/// let comma = ParseOptions {
///     decimal_separator: DecimalSeparator::Comma,
///     ..ParseOptions::default()
/// };
/// let fix = parse_with("48,8566; 2,3522", &comma)?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.8566, 2.3522));
///
/// // Neither reading of 100, 200 is in range.
/// assert!(matches!(parse_with("100, 200", &comma), Err(Error::Parse(_))));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn parse_with(input: &str, options: &ParseOptions) -> Result<Fix> {
    let decimal_comma = options.decimal_separator == DecimalSeparator::Comma;
    let normalized = normalize(input, decimal_comma);
    let (first, second) = split_two(&normalized, decimal_comma)
        .ok_or_else(|| Error::Parse(format!("could not split into two components: {input}")))?;
    let a = parse_component(&first)
        .ok_or_else(|| Error::Parse(format!("could not parse component: {first}")))?;
    let b = parse_component(&second)
        .ok_or_else(|| Error::Parse(format!("could not parse component: {second}")))?;

    let resolved = resolve(a, b, options.default_axis_order)?;
    let coord = Coordinate::wgs84(resolved.lat, resolved.lon).map_err(|_| {
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
