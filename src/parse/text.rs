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

use super::{AxisOrder, ParseReport};
use crate::error::Result;

/// Options controlling tolerant parsing.
#[derive(Debug, Clone)]
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
/// # Errors
/// Returns [`crate::Error::Parse`] when the input cannot be interpreted.
pub fn parse(input: &str) -> Result<ParseReport> {
    parse_with(input, &TextParseOptions::default())
}

/// Parse a free-text coordinate with explicit options.
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input cannot be interpreted.
pub fn parse_with(input: &str, options: &TextParseOptions) -> Result<ParseReport> {
    todo!("normalize typography, split, parse DD/DMS/DDM, resolve axis order, score confidence")
}
