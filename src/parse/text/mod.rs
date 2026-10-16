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

/// Parse a free-text DD / DMS / DDM coordinate with default options.
///
/// Free text only: unlike [`parse_coordinate`](super::parse_coordinate), this
/// never reads a `geo:` URI or a grid reference. The returned [`Fix`] records
/// the assumed axis order and parse confidence in its [`RawSource`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input cannot be interpreted.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::fix::AxisOrder;
/// use geocoordinates::parse::parse_text;
///
/// let fix = parse_text("40°42′46″N 74°00′22″W")?;
/// assert!((fix.coord.lat() - 40.712_78).abs() < 1e-5);
/// assert!((fix.coord.lon() + 74.006_11).abs() < 1e-5);
///
/// // Unlabelled numbers are read lat-first; the fix says so.
/// let fix = parse_text("40.7128, -74.006")?;
/// assert_eq!(fix.source.and_then(|s| s.axis_order), Some(AxisOrder::LatLon));
///
/// // Concatenated DDMM.mmm needs a hemisphere letter.
/// assert!(matches!(parse_text("4042.768, -7400.36"), Err(Error::Parse(_))));
/// // A grid reference is not free text.
/// assert!(matches!(parse_text("87G7PX7V+4J"), Err(Error::Parse(_))));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn parse_text(input: &str) -> Result<Fix> {
    parse_free_text(input, &ParseOptions::default())
}

/// [`parse_text`] with explicit options.
///
/// [`ParseOptions::default_axis_order`] and
/// [`ParseOptions::decimal_separator`] apply. Free text has no grid to expect,
/// so a set [`ParseOptions::expect_grid`] is rejected rather than ignored:
/// pass the options to [`parse_coordinate_with`](super::parse_coordinate_with)
/// instead, which tries the grid before falling back to free text.
///
/// # Errors
/// [`crate::Error::InvalidValue`] (field `"expect_grid"`) when
/// `options.expect_grid` is set, and [`crate::Error::Parse`] when the input
/// cannot be interpreted.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::format::DecimalSeparator;
/// use geocoordinates::parse::{GridKind, ParseOptions, parse_text_with};
///
/// let comma = ParseOptions {
///     decimal_separator: DecimalSeparator::Comma,
///     ..ParseOptions::default()
/// };
/// let fix = parse_text_with("48,8566; 2,3522", &comma)?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.8566, 2.3522));
///
/// // Neither reading of 100, 200 is in range.
/// assert!(matches!(parse_text_with("100, 200", &comma), Err(Error::Parse(_))));
///
/// // Expecting a grid reference is a request free text cannot honour.
/// let grid = ParseOptions {
///     expect_grid: Some(GridKind::Geohash),
///     ..ParseOptions::default()
/// };
/// assert!(matches!(
///     parse_text_with("48.8566, 2.3522", &grid),
///     Err(Error::InvalidValue { field: "expect_grid", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn parse_text_with(input: &str, options: &ParseOptions) -> Result<Fix> {
    if let Some(kind) = options.expect_grid {
        return Err(Error::InvalidValue {
            field: "expect_grid",
            detail: format!(
                "free-text parsing reads no grid reference, but {kind:?} was expected; \
                 use parse_coordinate_with"
            ),
        });
    }
    parse_free_text(input, options)
}

/// The free-text parser behind [`parse_text_with`] and
/// [`parse_coordinate_with`](super::parse_coordinate_with): reads
/// `default_axis_order` and `decimal_separator`, and leaves `expect_grid` to
/// the caller.
pub(super) fn parse_free_text(input: &str, options: &ParseOptions) -> Result<Fix> {
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
