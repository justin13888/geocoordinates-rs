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
//!   separator.

use super::AxisOrder;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::fix::{Confidence, Fix, RawSource};

mod normalize;
mod resolve;
#[cfg(test)]
mod tests;

use normalize::{normalize, parse_component, split_two};
use resolve::resolve;

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
