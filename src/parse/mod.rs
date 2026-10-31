//! Ingestion: turning real-world input into a [`Fix`].
//!
//! - [`parse_text`] / [`parse_text_with`] — tolerant free-text / DMS / DDM
//!   parsing (always available).
//! - [`parse_coordinate`] / [`parse_coordinate_with`] — best-effort detection
//!   across `geo:` URIs, grid references, and free text.
//! - [`from_geo_uri`] — `geo:` URIs per RFC 5870 (always available).
//! - `interchange` — GeoJSON, WKT, GPX, KML (each behind a cargo feature).
//! - `sensors` — NMEA 0183 (feature-gated). Reading EXIF out of a file is out of
//!   scope — a separate library handles it, consuming this crate's primitives.
//!
//! ## Axis order is first-class
//!
//! GeoJSON and KML are **lon-lat (X,Y)**, WKT leaves the order to the caller,
//! and humans and many EPSG CRS are lat-first. Every parser records in
//! [`RawSource::axis_order`] the order its source listed the axes in — `None`
//! only when the axes are labelled (a hemisphere letter, GPX `lat`/`lon`
//! attributes) or there is no axis pair at all (a grid reference) — and reports
//! a confidence so the application can decide whether to prompt the user.
//!
//! ## Out of scope
//!
//! Map-service URLs (Google `@lat,lon`, OSM, Apple), WKB, and GML are
//! intentionally **not** parsed here — the structured interchange surface is
//! limited to the text formats above plus `geo:` URIs.

mod options;
mod text;

#[cfg(any(feature = "geojson", feature = "wkt", feature = "gpx", feature = "kml"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "geojson", feature = "wkt", feature = "gpx", feature = "kml")))
)]
pub mod interchange;

#[cfg(feature = "nmea")]
#[cfg_attr(docsrs, doc(cfg(feature = "nmea")))]
pub mod sensors;

#[cfg(test)]
mod detection_tests;
#[cfg(all(
    test,
    any(feature = "geojson", feature = "wkt", feature = "gpx", feature = "kml")
))]
mod interchange_tests;

/// Nominal GPS user-equivalent range error (meters): turns a dimensionless
/// HDOP into a rough horizontal-accuracy estimate (`HDOP × UERE`), for NMEA
/// `GGA` and GPX `<hdop>` alike.
#[cfg(any(
    feature = "nmea",
    feature = "geojson",
    feature = "wkt",
    feature = "gpx",
    feature = "kml"
))]
const NOMINAL_UERE_M: f64 = 5.0;

/// `HDOP × UERE` as a one-sigma horizontal accuracy, for NMEA `GGA` and GPX
/// `<hdop>` alike.
#[cfg(any(
    feature = "nmea",
    feature = "geojson",
    feature = "wkt",
    feature = "gpx",
    feature = "kml"
))]
fn hdop_accuracy(hdop: f64) -> Result<Accuracy> {
    if !hdop.is_finite() || hdop < 0.0 {
        return Err(syntax(format!(
            "hdop must be finite and nonnegative, got {hdop}"
        )));
    }
    // A finite HDOP can still overflow once scaled (e.g. 1e308 × 5 m).
    let scaled = hdop * NOMINAL_UERE_M;
    let horizontal = Length::from_meters(scaled);
    Accuracy::new(AccuracyKind::OneSigma, Some(horizontal), None).map_err(|_| {
        syntax(format!(
            "hdop {hdop} scales to a horizontal accuracy out of range \
             ({hdop} × {NOMINAL_UERE_M} m = {scaled} m)"
        ))
    })
}

/// A [`ParseErrorKind::Syntax`] error: input the format's grammar does not
/// allow, or a field holding a value the format does not allow.
fn syntax(detail: impl Into<String>) -> Error {
    Error::parse(ParseErrorKind::Syntax, detail)
}

use crate::approx::Approx;
use crate::coord::{Coordinate, Height};
use crate::error::{Error, ParseErrorKind, Result};
use crate::fix::{Accuracy, AccuracyKind, AxisOrder, Confidence, Fix, RawSource};
use crate::grids::{Geohash, Maidenhead, Mgrs, PlusCode};
use crate::units::Length;

pub use options::{GridKind, ParseOptions};
pub use text::{parse_text, parse_text_with};

/// Best-effort parse of a single coordinate from arbitrary input, with the
/// default [`ParseOptions`] (lat-first, `.` decimals, no expected grid).
///
/// Recognizes, in order: a `geo:` URI (see [`from_geo_uri`]), a Plus Code, an
/// MGRS reference, and then free-text DD/DMS/DDM (see [`parse_text`]). A grid
/// reference decodes to its cell's center, with the cell's error bound as the
/// horizontal accuracy. Geohash and Maidenhead strings are **not** detected,
/// because short ones are indistinguishable from other input (`"42"` and
/// `"bed"` are valid geohashes); ask for them through
/// [`ParseOptions::expect_grid`] with [`parse_coordinate_with`]. UTM has no
/// string form here and is not detected either. The returned [`Fix`] records
/// confidence and the assumed [`AxisOrder`] in its [`RawSource`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] when no interpretation is found, with the
/// [`ParseErrorKind`] of the route that rejected it:
/// [`from_geo_uri`]'s for a `geo:` URI, otherwise [`parse_text`]'s.
///
/// ```
/// use geocoordinates::{Error, ParseErrorKind};
/// use geocoordinates::parse::parse_coordinate;
///
/// let fix = parse_coordinate("40.7128, -74.006")?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (40.7128, -74.006));
///
/// // A Plus Code decodes to its cell center, with the cell as the accuracy.
/// let fix = parse_coordinate("87G7PX7V+4J")?;
/// assert!((fix.coord.lat() - 40.7128).abs() < 1e-3);
/// assert!(fix.accuracy.and_then(|a| a.horizontal()).is_some());
///
/// assert!(matches!(
///     parse_coordinate("somewhere"),
///     Err(Error::Parse { kind: ParseErrorKind::Syntax, .. })
/// ));
/// assert!(matches!(
///     parse_coordinate("geo:48.2,16.3;crs=epsg:3857"),
///     Err(Error::Parse { kind: ParseErrorKind::Unsupported, .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn parse_coordinate(input: &str) -> Result<Fix> {
    parse_coordinate_with(input, &ParseOptions::default())
}

/// [`parse_coordinate`] with explicit options.
///
/// After a `geo:` URI, the grid [`ParseOptions::expect_grid`] names is tried
/// first, then Plus Code and MGRS, then free text read with
/// [`ParseOptions::default_axis_order`] and
/// [`ParseOptions::decimal_separator`]. To read back what
/// [`format`](crate::format::format) rendered, pass
/// [`FormatOptions::parse_options`](crate::format::FormatOptions::parse_options).
///
/// # Errors
/// Returns [`crate::Error::Parse`] when no interpretation is found, with the
/// kind [`parse_coordinate`] gives. When `expect_grid` is set, the detail also
/// gives why the input is not a valid reference of that kind.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::parse::{GridKind, ParseOptions, parse_coordinate_with};
///
/// let options = ParseOptions::default().with_expect_grid(Some(GridKind::Maidenhead));
/// let fix = parse_coordinate_with("FN20", &options)?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (40.5, -75.0));
///
/// // Neither a locator nor coordinate text: the error gives both reasons.
/// let Err(Error::Parse { detail, .. }) = parse_coordinate_with("FN2", &options) else {
///     panic!("expected a parse error");
/// };
/// assert!(detail.starts_with("not a Maidenhead reference"));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn parse_coordinate_with(input: &str, options: &ParseOptions) -> Result<Fix> {
    // Dispatch on the trimmed input, but hand every builder the verbatim
    // `input` so `RawSource::raw` never depends on which path matched. Each
    // builder trims for itself.
    let trimmed = input.trim();
    if trimmed
        .get(..4)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("geo:"))
    {
        return from_geo_uri(input);
    }
    let expected = options
        .expect_grid
        .map(|kind| decode_grid(kind, trimmed).map_err(|e| (kind, e)));
    let decoded = match expected {
        Some(Ok(area)) => Ok(area),
        _ => decode_grid(GridKind::PlusCode, trimmed)
            .or_else(|_| decode_grid(GridKind::Mgrs, trimmed)),
    };
    if let Ok(area) = decoded {
        return grid_fix(area, input);
    }
    text::parse_free_text(input, options).map_err(|text_error| match (expected, text_error) {
        (Some(Err((grid, grid_error))), Error::Parse { kind, detail }) => Error::Parse {
            kind,
            detail: format!("not a {grid:?} reference ({grid_error}); {detail}"),
        },
        (_, text_error) => text_error,
    })
}

/// Decode `trimmed` as a reference of the given grid kind, to its cell's
/// center and error bound.
fn decode_grid(kind: GridKind, trimmed: &str) -> Result<Approx<Coordinate>> {
    match kind {
        GridKind::PlusCode => PlusCode::parse(trimmed).map(|c| c.decode()),
        GridKind::Geohash => Geohash::parse(trimmed).map(|g| g.decode()),
        GridKind::Maidenhead => Maidenhead::parse(trimmed).map(|m| m.decode()),
        GridKind::Mgrs => Mgrs::parse(trimmed).map(|m| m.to_coordinate()),
    }
}

/// Build a [`Fix`] from a decoded grid cell: the cell center, with the cell's
/// error bound recorded as an [`AccuracyKind::Bound`] horizontal accuracy. A
/// grid reference lists no axis pair, so it records no axis order.
fn grid_fix(area: Approx<Coordinate>, raw: &str) -> Result<Fix> {
    let bound = area.max_error();
    let mut fix = Fix::from_coord(area.into_inner());
    fix.accuracy = Some(Accuracy::new(AccuracyKind::Bound, Some(bound), None)?);
    fix.source = Some(RawSource::new(raw, Confidence::CERTAIN));
    Ok(fix)
}

/// Parse a `geo:` URI per [RFC 5870](https://www.rfc-editor.org/rfc/rfc5870),
/// e.g. `geo:13.4125,103.8667` or `geo:48.2,16.3,183;crs=wgs84;u=40`.
///
/// Latitude comes first (the RFC fixes the axis order), an optional third
/// number is the altitude in meters, and the `crs`/`u` parameters set the
/// reference system and the horizontal accuracy (meters) on the returned
/// [`Fix`]. The RFC states no confidence level for `u=`, so that accuracy is
/// [`AccuracyKind::Unspecified`].
///
/// # Errors
/// Returns [`crate::Error::Parse`] of kind
/// [`Unsupported`](crate::ParseErrorKind::Unsupported) when the `crs`
/// parameter names a reference system other than `wgs84`, and of kind
/// [`Syntax`](crate::ParseErrorKind::Syntax) when the input is otherwise not a
/// well-formed `geo:` URI or its position is out of range.
///
/// ```
/// use geocoordinates::fix::AccuracyKind;
/// use geocoordinates::{Error, Height, ParseErrorKind};
/// use geocoordinates::parse::from_geo_uri;
///
/// let fix = from_geo_uri("geo:48.2,16.3,183;crs=wgs84;u=40")?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.2, 16.3));
/// assert_eq!(fix.coord.height(), Some(Height::Ellipsoidal(183.0)));
/// let accuracy = fix.accuracy.expect("u= sets the accuracy");
/// assert_eq!(accuracy.horizontal().map(|h| h.meters()), Some(40.0));
/// assert_eq!(accuracy.kind(), AccuracyKind::Unspecified);
///
/// assert!(matches!(
///     from_geo_uri("geo:48.2"),
///     Err(Error::Parse { kind: ParseErrorKind::Syntax, .. })
/// ));
/// assert!(matches!(
///     from_geo_uri("geo:48.2,16.3;crs=nad27"),
///     Err(Error::Parse { kind: ParseErrorKind::Unsupported, .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn from_geo_uri(input: &str) -> Result<Fix> {
    let trimmed = input.trim();
    let scheme = trimmed.get(..4);
    if scheme.is_none_or(|s| !s.eq_ignore_ascii_case("geo:")) {
        return Err(syntax(format!("not a geo: URI: {input}")));
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
        return Err(syntax(format!("too many coordinate fields: {input}")));
    }

    let mut horizontal_m = None;
    let mut seen_crs = false;
    let mut seen_u = false;
    let mut notes = Vec::new();
    for param in parts {
        let param = param.trim();
        if param.is_empty() {
            return Err(syntax(format!("empty geo: parameter: {input}")));
        }
        let (key, value) = param.split_once('=').unwrap_or((param, ""));
        match key.trim().to_ascii_lowercase().as_str() {
            "crs" => {
                if seen_crs || seen_u {
                    return Err(syntax(format!(
                        "crs must appear once and before u: {input}"
                    )));
                }
                seen_crs = true;
                if !value.trim().eq_ignore_ascii_case("wgs84") {
                    return Err(Error::parse(
                        ParseErrorKind::Unsupported,
                        format!("unsupported geo: CRS '{}': {input}", value.trim()),
                    ));
                }
            }
            "u" => {
                if seen_u {
                    return Err(syntax(format!(
                        "duplicate geo: uncertainty parameter: {input}"
                    )));
                }
                seen_u = true;
                let u = parse_number(Some(value), input)?;
                if u < 0.0 {
                    return Err(syntax(format!(
                        "geo: uncertainty must be nonnegative: {input}"
                    )));
                }
                horizontal_m = Some(u);
            }
            _ => notes.push(format!("ignored unknown parameter '{key}'")),
        }
    }

    let out_of_range = |_| syntax(format!("geo: coordinate out of range: {input}"));
    let mut coord = Coordinate::wgs84(lat, lon).map_err(out_of_range)?;
    if let Some(alt) = altitude {
        coord = coord
            .with_height(Height::Ellipsoidal(alt))
            .map_err(out_of_range)?;
    }

    let mut fix = Fix::from_coord(coord);
    // RFC 5870 states no confidence level for `u=`.
    fix.accuracy = horizontal_m
        .map(|h| {
            Accuracy::new(
                AccuracyKind::Unspecified,
                Some(Length::from_meters(h)),
                None,
            )
        })
        .transpose()?;
    let mut source = RawSource::new(input, Confidence::CERTAIN); // geo: is unambiguous
    source.axis_order = Some(AxisOrder::LatLon); // RFC 5870 lists lat first
    source.notes = notes;
    fix.source = Some(source);
    Ok(fix)
}

/// Parse a required `f64` field of a `geo:` URI, erroring with context.
fn parse_number(field: Option<&str>, input: &str) -> Result<f64> {
    let value = field
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| syntax(format!("invalid geo: URI: {input}")))?;
    if !value.is_finite() {
        return Err(syntax(format!(
            "geo: numeric fields must be finite: {input}"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{FormatOptions, format};
    use crate::test_support::{assert_close, assert_within_meters};

    fn wgs(lat: f64, lon: f64) -> Coordinate {
        Coordinate::wgs84(lat, lon).unwrap()
    }

    #[test]
    fn geo_uri_basic() {
        let fix = from_geo_uri("geo:13.4125,103.8667").unwrap();
        assert_close(fix.coord.lat(), 13.4125, 1e-9);
        assert_close(fix.coord.lon(), 103.8667, 1e-9);
        let s = fix.source.as_ref().unwrap();
        assert_eq!(s.axis_order, Some(AxisOrder::LatLon)); // RFC 5870 lists lat first
        assert_close(s.confidence.value(), 1.0, 1e-12);
        assert!(fix.accuracy.is_none());
    }

    #[test]
    fn geo_uri_with_altitude_crs_and_accuracy() {
        let fix = from_geo_uri("geo:48.2,16.3,183;crs=wgs84;u=40").unwrap();
        assert_close(fix.coord.lat(), 48.2, 1e-9);
        assert_close(fix.coord.lon(), 16.3, 1e-9);
        assert_eq!(fix.coord.height(), Some(Height::Ellipsoidal(183.0)));
        // RFC 5870 states no confidence level for `u=`.
        assert_eq!(
            fix.accuracy,
            Some(
                Accuracy::new(
                    AccuracyKind::Unspecified,
                    Some(Length::from_meters(40.0)),
                    None
                )
                .unwrap()
            )
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
        assert!(from_geo_uri("geo:1,2;crs=epsg:7030").is_err());
    }

    #[test]
    fn geo_uri_errors() {
        assert!(from_geo_uri("geo:").is_err());
        assert!(from_geo_uri("geo:abc").is_err());
        assert!(from_geo_uri("geo:91,0").is_err()); // out of range
        assert!(from_geo_uri("http:1,2").is_err()); // wrong scheme
        assert!(from_geo_uri("geo:1,2;u=-1").is_err());
        assert!(from_geo_uri("geo:1,2;u=NaN").is_err());
        assert!(from_geo_uri("geo:1,2;u=1;crs=wgs84").is_err());
        assert!(from_geo_uri("geo:1,2;u=1;u=2").is_err());
        assert!(from_geo_uri("geo:1,2;").is_err());
        assert!(from_geo_uri("geo:1,2,inf").is_err());
    }

    /// RFC 5870 allows `u=0` (an exact position); only a negative value is
    /// rejected.
    #[test]
    fn geo_uri_accepts_zero_uncertainty() {
        let fix = from_geo_uri("geo:1,2;u=0").unwrap();
        assert_eq!(fix.accuracy.unwrap().horizontal(), Some(Length::ZERO));
    }

    #[test]
    fn parse_coordinate_dispatches() {
        // geo: path -> the RFC's lat-first listing order.
        let geo = parse_coordinate("geo:13.4125,103.8667").unwrap();
        assert_eq!(
            geo.source.as_ref().unwrap().axis_order,
            Some(AxisOrder::LatLon)
        );
        // text path -> axis order recorded.
        let text = parse_coordinate("40.7128, -74.006").unwrap();
        assert!(text.source.as_ref().unwrap().axis_order.is_some());
    }

    #[test]
    fn parse_coordinate_yields_the_coordinate() {
        let c: Coordinate = parse_coordinate("40.7128, -74.006").unwrap().coord;
        assert_close(c.lat(), 40.7128, 1e-9);
        assert_close(c.lon(), -74.006, 1e-9);
        assert!(parse_coordinate("not a coordinate").is_err());
    }

    #[test]
    fn round_trip_format_then_parse() {
        // Default DD formatting (6 dp ≈ 0.11 m) must re-parse to within 0.2 m.
        let coords = [
            wgs(40.7128, -74.006),
            wgs(-33.8688, 151.2093), // range rule forces order back
            wgs(0.0, 0.0),
            wgs(51.5074, -0.1278),
        ];
        for c in coords {
            let text = format(c, &FormatOptions::default()).unwrap();
            let parsed = parse_coordinate(&text).unwrap();
            assert_within_meters(parsed.coord, c, 0.2);
        }
    }

    #[test]
    fn round_trip_every_style_and_locale() {
        use crate::format::{DecimalSeparator, HemisphereStyle, Representation, SymbolStyle};
        // Hemispheres, the range rule, the antimeridian and both poles.
        let coords = [
            wgs(40.7128, -74.006),
            wgs(-33.8688, 151.2093),
            wgs(10.5, -20.25),
            wgs(0.0, 0.0),
            wgs(-0.5, 180.0),
            wgs(12.25, -180.0),
            wgs(90.0, 45.0),
            wgs(-90.0, -135.5),
        ];
        let symbols = [
            SymbolStyle::Unicode,
            SymbolStyle::Ascii,
            SymbolStyle::Letters,
        ];
        let hemispheres = [HemisphereStyle::Signed, HemisphereStyle::Cardinal];
        let mut representations = Vec::new();
        for hemisphere in hemispheres {
            representations.push(Representation::DecimalDegrees {
                decimals: None,
                hemisphere,
            });
            for symbols in symbols {
                representations.push(Representation::Dms {
                    second_decimals: None,
                    symbols,
                    hemisphere,
                });
                representations.push(Representation::Ddm {
                    minute_decimals: None,
                    symbols,
                    hemisphere,
                });
            }
        }
        let locales = [None, Some("en-US"), Some("de-DE"), Some("fr")];
        for c in coords {
            for &representation in &representations {
                for locale in locales {
                    let decimal_separator =
                        locale.map_or(DecimalSeparator::Point, DecimalSeparator::for_locale);
                    let options = FormatOptions {
                        representation,
                        decimal_separator,
                    };
                    let text = format(c, &options).unwrap();
                    let parsed = parse_coordinate_with(&text, &options.parse_options())
                        .unwrap_or_else(|e| panic!("{text:?} ({options:?}): {e}"));
                    // Default precisions: DD 6 dp (0.11 m), DMS 0.01″
                    // (0.3 m), DDM 0.001′ (1.9 m) — each rounds to half.
                    assert_within_meters(parsed.coord, c, 1.0);
                    if locale.is_none() {
                        let plain = parse_coordinate(&text).unwrap();
                        assert_eq!(plain.coord, parsed.coord, "{text:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn parse_coordinate_with_reads_decimal_comma_on_the_text_path_only() {
        let comma = ParseOptions {
            decimal_separator: crate::format::DecimalSeparator::Comma,
            ..ParseOptions::default()
        };
        let fix = parse_coordinate_with("40,7128 -74,006", &comma).unwrap();
        assert_close(fix.coord.lat(), 40.7128, 1e-9);
        assert_close(fix.coord.lon(), -74.006, 1e-9);
        assert!(parse_coordinate("40,7128 -74,006").is_err());
        // geo: URIs and Plus Codes ignore the text options.
        let geo = parse_coordinate_with("geo:13.4125,103.8667", &comma).unwrap();
        assert_close(geo.coord.lon(), 103.8667, 1e-9);
        let plus = parse_coordinate_with("8FVC2222+22", &comma).unwrap();
        assert_close(plus.coord.lat(), 47.0000625, 1e-6);
    }

    #[test]
    fn parse_coordinate_detects_plus_code() {
        let fix = parse_coordinate("8FVC2222+22").unwrap();
        assert_close(fix.coord.lat(), 47.0000625, 1e-6);
        assert_close(fix.coord.lon(), 8.0000625, 1e-6);
        let s = fix.source.as_ref().unwrap();
        assert_eq!(s.axis_order, None); // a Plus Code fixes the axis order
        assert_close(s.confidence.value(), 1.0, 1e-12);
        // The cell error bound (~8.4 m for a length-10 code) is the accuracy,
        // tagged as a hard bound.
        let accuracy = fix.accuracy.unwrap();
        assert_eq!(accuracy.kind(), AccuracyKind::Bound);
        let acc = accuracy.horizontal().unwrap().meters();
        assert!((5.0..15.0).contains(&acc), "{acc}");
    }

    #[test]
    fn plus_code_round_trip() {
        use crate::format::Representation;
        let c = Coordinate::wgs84(40.7128, -74.006).unwrap();
        let options = FormatOptions {
            representation: Representation::PlusCode { length: 10 },
            ..FormatOptions::default()
        };
        let code = format(c, &options).unwrap();
        let fix = parse_coordinate(&code).unwrap();
        // The original lies within the decoded cell's reported bound.
        let bound = fix.accuracy.unwrap().horizontal().unwrap().meters();
        assert_within_meters(fix.coord, c, bound);
    }

    #[test]
    fn parse_coordinate_keeps_raw_input_verbatim_on_every_path() {
        for input in [
            "  geo:13.4125,103.8667 \n",
            "\t8FVC2222+22  ",
            " 40.7128, -74.0060\t",
        ] {
            let fix = parse_coordinate(input).unwrap();
            assert_eq!(fix.source.unwrap().raw, input, "{input:?}");
        }
    }
}
