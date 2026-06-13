//! Ingestion: turning real-world input into a [`Fix`].
//!
//! - [`text`] — tolerant free-text / DMS / DDM parsing (always available).
//! - [`from_geo_uri`] — `geo:` URIs per RFC 5870 (always available).
//! - `interchange` — GeoJSON, WKT, GPX, KML (each behind a cargo feature; a
//!   later release).
//! - `sensors` — NMEA 0183 (feature-gated; a later release). EXIF is out of
//!   scope — a separate library handles it, consuming this crate's primitives.
//!
//! ## Axis order is first-class
//!
//! GeoJSON and WKT are **lon-lat (X,Y)**; humans and many EPSG CRS are
//! lat-first. Every parser records the [`AxisOrder`] it assumed and reports a
//! confidence so the application can decide whether to prompt the user.
//!
//! ## Out of scope
//!
//! Map-service URLs (Google `@lat,lon`, OSM, Apple), WKB, and GML are
//! intentionally **not** parsed here — the structured interchange surface is
//! limited to the text formats above plus `geo:` URIs.

pub mod text;

#[cfg(any(feature = "geojson", feature = "wkt", feature = "gpx", feature = "kml"))]
pub mod interchange;

#[cfg(feature = "nmea")]
pub mod sensors;

use crate::coord::{Coordinate, Height};
use crate::error::{Error, Result};
use crate::fix::{Accuracy, Confidence, Fix, RawSource};
use crate::grids::PlusCode;

/// Axis ordering of a textual/structured coordinate. Re-exported from
/// [`fix`](crate::fix), where it lives so parsers can record it on a [`Fix`]'s
/// [`RawSource`].
pub use crate::fix::AxisOrder;

/// Best-effort parse of a single coordinate from arbitrary input.
///
/// Recognizes, in order: a `geo:` URI (see [`from_geo_uri`]); then falls back
/// to free-text DD/DMS/DDM heuristics (see [`text`]). Grid-token detection
/// (UTM / MGRS / Plus Code / geohash) is added as each grid milestone ships —
/// see `ROADMAP.md`. The returned [`Fix`] records parse confidence and the
/// assumed [`AxisOrder`] in its [`RawSource`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] when no interpretation is found.
pub fn parse_coordinate(input: &str) -> Result<Fix> {
    let trimmed = input.trim();
    if trimmed
        .get(..4)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("geo:"))
    {
        from_geo_uri(trimmed)
    } else if let Ok(code) = PlusCode::try_from(trimmed) {
        Ok(plus_code_fix(&code, input))
    } else {
        text::parse(trimmed)
    }
}

/// Build a [`Fix`] from a decoded Plus Code: the cell center, with the cell's
/// error bound recorded as horizontal accuracy. Axis order is unambiguous.
fn plus_code_fix(code: &PlusCode, raw: &str) -> Fix {
    let area = code.decode();
    let bound = area.max_error_m();
    Fix {
        coord: area.into_inner(),
        accuracy: Some(Accuracy {
            horizontal_m: Some(bound),
            vertical_m: None,
        }),
        timestamp: None,
        source: Some(RawSource {
            raw: raw.to_string(),
            confidence: Confidence::new(1.0),
            axis_order: None,
            datum_ambiguity: None,
            notes: Vec::new(),
        }),
    }
}

/// Parse a `geo:` URI per [RFC 5870](https://www.rfc-editor.org/rfc/rfc5870),
/// e.g. `geo:13.4125,103.8667` or `geo:48.2,16.3,183;crs=wgs84;u=40`.
///
/// Latitude comes first (the RFC fixes the axis order), an optional third
/// number is the altitude in meters, and the `crs`/`u` parameters set the
/// reference system and the horizontal accuracy (meters) on the returned
/// [`Fix`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] when the input is not a well-formed `geo:`
/// URI.
pub fn from_geo_uri(input: &str) -> Result<Fix> {
    let trimmed = input.trim();
    let scheme = trimmed.get(..4);
    if scheme.is_none_or(|s| !s.eq_ignore_ascii_case("geo:")) {
        return Err(Error::Parse(format!("not a geo: URI: {input}")));
    }
    let rest = &trimmed[4..];

    // The coordinate part precedes the first ';'; parameters follow it.
    let mut parts = rest.split(';');
    let coords = parts.next().unwrap_or_default();

    let mut numbers = coords.split(',');
    let lat = parse_number(numbers.next(), input)?;
    let lon = parse_number(numbers.next(), input)?;
    let altitude = match numbers.next() {
        Some(alt) => Some(parse_number(Some(alt), input)?),
        None => None,
    };
    if numbers.next().is_some() {
        return Err(Error::Parse(format!("too many coordinate fields: {input}")));
    }

    let mut horizontal_m = None;
    let mut notes = Vec::new();
    for param in parts {
        let param = param.trim();
        if param.is_empty() {
            continue; // tolerate a trailing ';'
        }
        let (key, value) = param.split_once('=').unwrap_or((param, ""));
        match key.trim().to_ascii_lowercase().as_str() {
            "crs" => {
                if !value.trim().eq_ignore_ascii_case("wgs84") {
                    notes.push(format!("unknown crs '{}'; assuming WGS-84", value.trim()));
                }
            }
            "u" => {
                if let Ok(u) = value.trim().parse::<f64>() {
                    horizontal_m = Some(u);
                } else {
                    notes.push(format!(
                        "ignored unparseable uncertainty '{}'",
                        value.trim()
                    ));
                }
            }
            _ => notes.push(format!("ignored unknown parameter '{key}'")),
        }
    }

    let mut coord = Coordinate::wgs84(lat, lon);
    coord
        .validate()
        .map_err(|_| Error::Parse(format!("geo: coordinate out of range: {input}")))?;
    if let Some(alt) = altitude {
        coord = coord.with_height(Height::Ellipsoidal(alt));
    }

    Ok(Fix {
        coord,
        accuracy: horizontal_m.map(|h| Accuracy {
            horizontal_m: Some(h),
            vertical_m: None,
        }),
        timestamp: None,
        source: Some(RawSource {
            raw: input.to_string(),
            confidence: Confidence::new(1.0), // geo: is unambiguous
            axis_order: None,                 // RFC 5870 fixes lat-first
            datum_ambiguity: None,
            notes,
        }),
    })
}

/// Parse a required `f64` field of a `geo:` URI, erroring with context.
fn parse_number(field: Option<&str>, input: &str) -> Result<f64> {
    field
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| Error::Parse(format!("invalid geo: URI: {input}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{FormatOptions, format};
    use crate::test_support::{assert_close, assert_within_meters};

    #[test]
    fn geo_uri_basic() {
        let fix = from_geo_uri("geo:13.4125,103.8667").unwrap();
        assert_close(fix.coord.lat, 13.4125, 1e-9);
        assert_close(fix.coord.lon, 103.8667, 1e-9);
        let s = fix.source.as_ref().unwrap();
        assert_eq!(s.axis_order, None); // RFC 5870 fixes lat-first
        assert_close(s.confidence.value(), 1.0, 1e-12);
        assert!(fix.accuracy.is_none());
    }

    #[test]
    fn geo_uri_with_altitude_crs_and_accuracy() {
        let fix = from_geo_uri("geo:48.2,16.3,183;crs=wgs84;u=40").unwrap();
        assert_close(fix.coord.lat, 48.2, 1e-9);
        assert_close(fix.coord.lon, 16.3, 1e-9);
        assert_eq!(fix.coord.height, Some(Height::Ellipsoidal(183.0)));
        assert_eq!(
            fix.accuracy,
            Some(Accuracy {
                horizontal_m: Some(40.0),
                vertical_m: None,
            })
        );
    }

    #[test]
    fn geo_uri_scheme_is_case_insensitive() {
        assert!(from_geo_uri("GEO:13.4125,103.8667").is_ok());
    }

    #[test]
    fn geo_uri_crs_parameter() {
        // A recognized crs (wgs84) adds no note.
        let known = from_geo_uri("geo:1,2;crs=wgs84").unwrap();
        let notes = &known.source.unwrap().notes;
        assert!(notes.iter().all(|n| !n.contains("crs")), "{notes:?}");
        // An unrecognized crs is noted; the coordinate stays WGS-84.
        let unknown = from_geo_uri("geo:1,2;crs=epsg:7030").unwrap();
        assert!(
            unknown
                .source
                .unwrap()
                .notes
                .iter()
                .any(|n| n.contains("unknown crs"))
        );
    }

    #[test]
    fn geo_uri_errors() {
        assert!(from_geo_uri("geo:").is_err());
        assert!(from_geo_uri("geo:abc").is_err());
        assert!(from_geo_uri("geo:91,0").is_err()); // out of range
        assert!(from_geo_uri("http:1,2").is_err()); // wrong scheme
    }

    #[test]
    fn parse_coordinate_dispatches() {
        // geo: path -> axis order fixed (None).
        let geo = parse_coordinate("geo:13.4125,103.8667").unwrap();
        assert_eq!(geo.source.as_ref().unwrap().axis_order, None);
        // text path -> axis order recorded.
        let text = parse_coordinate("40.7128, -74.006").unwrap();
        assert!(text.source.as_ref().unwrap().axis_order.is_some());
    }

    #[test]
    fn coordinate_from_str() {
        let c: Coordinate = "40.7128, -74.006".parse().unwrap();
        assert_close(c.lat, 40.7128, 1e-9);
        assert_close(c.lon, -74.006, 1e-9);
        assert!("not a coordinate".parse::<Coordinate>().is_err());
    }

    #[test]
    fn round_trip_format_then_parse() {
        // Default DD formatting (6 dp ≈ 0.11 m) must re-parse to within 0.2 m.
        let coords = [
            Coordinate::wgs84(40.7128, -74.006),
            Coordinate::wgs84(-33.8688, 151.2093), // range rule forces order back
            Coordinate::wgs84(0.0, 0.0),
            Coordinate::wgs84(51.5074, -0.1278),
        ];
        for c in coords {
            let text = format(&c, &FormatOptions::default()).unwrap();
            let parsed = parse_coordinate(&text).unwrap();
            assert_within_meters(&parsed.coord, &c, 0.2);
        }
    }

    #[test]
    fn parse_coordinate_detects_plus_code() {
        let fix = parse_coordinate("8FVC2222+22").unwrap();
        assert_close(fix.coord.lat, 47.0000625, 1e-6);
        assert_close(fix.coord.lon, 8.0000625, 1e-6);
        let s = fix.source.as_ref().unwrap();
        assert_eq!(s.axis_order, None); // a Plus Code fixes the axis order
        assert_close(s.confidence.value(), 1.0, 1e-12);
        // The cell error bound (~8.4 m for a length-10 code) is the accuracy.
        let acc = fix.accuracy.unwrap().horizontal_m.unwrap();
        assert!((5.0..15.0).contains(&acc), "{acc}");
    }

    #[test]
    fn plus_code_round_trip() {
        use crate::format::Representation;
        let c = Coordinate::wgs84(40.7128, -74.006);
        let options = FormatOptions {
            representation: Representation::PlusCode,
            ..FormatOptions::default()
        };
        let code = format(&c, &options).unwrap();
        let fix = parse_coordinate(&code).unwrap();
        // The original lies within the decoded cell's reported bound.
        let bound = fix.accuracy.unwrap().horizontal_m.unwrap();
        assert_within_meters(&fix.coord, &c, bound);
    }
}
