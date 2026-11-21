//! Structured interchange parsers: geometry walks, axis order, altitude
//! meaning, per-position raw fragments, GPX metadata, and indexed rejections.

use super::interchange::*;
use crate::coord::Height;
use crate::error::{Error, ParseErrorKind, Result};
#[cfg(any(feature = "geojson", feature = "wkt", feature = "kml"))]
use crate::fix::AxisOrder;
use crate::fix::{Fix, RawSource};
use crate::test_support::assert_close;

/// The `(lat, lon)` of every fix, in emission order.
#[cfg(any(feature = "geojson", feature = "wkt", feature = "gpx"))]
fn lat_lons(fixes: &[Fix]) -> Vec<(f64, f64)> {
    fixes
        .iter()
        .map(|f| (f.coord.lat(), f.coord.lon()))
        .collect()
}

fn source(fix: &Fix) -> &RawSource {
    fix.source.as_ref().unwrap()
}

// The two assertion helpers are macros rather than functions: a `()`-returning
// helper is a mutation target whose deletion no test can observe.

/// A rejection is `Parse` of `kind` (`Syntax` when omitted) whose detail
/// starts with `prefix`.
macro_rules! assert_parse_err {
    ($r:expr, $prefix:expr $(,)?) => {
        assert_parse_err!($r, $prefix, ParseErrorKind::Syntax)
    };
    ($r:expr, $prefix:expr, $kind:expr) => {{
        let r: Result<Vec<Fix>> = $r;
        let ok = matches!(&r, Err(Error::Parse { kind, detail })
            if *kind == $kind && detail.starts_with($prefix));
        assert!(ok, "{r:?}");
    }};
}

/// The typed rejection of a position at (`lat`, `lon`), out of range.
#[cfg(any(feature = "geojson", feature = "wkt", feature = "kml"))]
fn out_of_range(lat: f64, lon: f64) -> Result<Vec<Fix>> {
    Err(Error::OutOfRange { lat, lon })
}

/// A rejection is a `TooDeep` parse error whose detail starts with `prefix`.
#[cfg(any(feature = "geojson", feature = "wkt", feature = "kml"))]
macro_rules! assert_too_deep {
    ($r:expr, $prefix:expr $(,)?) => {
        assert_parse_err!($r, $prefix, ParseErrorKind::TooDeep)
    };
}

/// Every fix's raw fragment parses back, alone, to that same fix.
#[cfg(any(feature = "geojson", feature = "wkt", feature = "kml"))]
macro_rules! assert_fragments_reparse {
    ($fixes:expr, $parse:expr $(,)?) => {{
        let parse = $parse;
        for fix in $fixes.iter() {
            let again: Vec<Fix> = parse(&source(fix).raw).unwrap();
            assert_eq!(again, std::slice::from_ref(fix), "{}", source(fix).raw);
        }
    }};
}

/// `depth` copies of `open`, then `inner`, then `depth` copies of `close`.
#[cfg(any(feature = "wkt", feature = "kml"))]
fn nested(open: &str, inner: &str, close: &str, depth: usize) -> String {
    format!("{}{inner}{}", open.repeat(depth), close.repeat(depth))
}

#[cfg(feature = "geojson")]
mod geojson;
#[cfg(feature = "gpx")]
mod gpx;
#[cfg(feature = "kml")]
mod kml;
#[cfg(feature = "kml")]
mod kml_point;
#[cfg(feature = "wkt")]
mod wkt;
#[cfg(feature = "kml")]
mod xml_depth;
