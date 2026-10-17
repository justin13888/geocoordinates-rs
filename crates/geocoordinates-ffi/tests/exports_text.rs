//! Calls every coordinate-text FFI export (formatting options and rendering,
//! free-text and `geo:` URI parsing) at least once with an asserted value.
//!
//! Values are pinned to what the core crate produces today, so a behaviour
//! change in the mirror or the core shows up here as a failing assertion.

use geocoordinates_ffi::*;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

/// Options for `representation` with a point separator.
fn opts(representation: Representation) -> FormatOptions {
    FormatOptions {
        representation,
        decimal_separator: DecimalSeparator::Point,
    }
}

fn dd(decimals: Option<u8>) -> Representation {
    Representation::DecimalDegrees {
        decimals,
        hemisphere: HemisphereStyle::Signed,
    }
}

// --- Formatting ---

#[test]
fn format_coordinate_renders_each_representation() {
    let ny = wgs84(40.7128, -74.006);
    assert_eq!(
        format_coordinate(ny, opts(dd(Some(6)))).unwrap(),
        "40.712800, -74.006000"
    );
    let dms = opts(Representation::Dms {
        second_decimals: Some(0),
        symbols: SymbolStyle::Unicode,
        hemisphere: HemisphereStyle::Cardinal,
    });
    assert_eq!(
        format_coordinate(wgs84(10.5, 20.25), dms).unwrap(),
        "10°30′00″N 20°15′00″E"
    );
    let ddm = opts(Representation::Ddm {
        minute_decimals: Some(3),
        symbols: SymbolStyle::Ascii,
        hemisphere: HemisphereStyle::Cardinal,
    });
    assert_eq!(
        format_coordinate(ny, ddm).unwrap(),
        "40°42.768'N 74°00.360'W"
    );
    let plus = opts(Representation::PlusCode { length: 10 });
    assert_eq!(
        format_coordinate(wgs84(47.000_062_5, 8.000_062_5), plus).unwrap(),
        "8FVC2222+22"
    );
    let geohash = opts(Representation::Geohash { length: 5 });
    assert_eq!(
        format_coordinate(wgs84(42.6, -5.6), geohash).unwrap(),
        "ezs42"
    );
    let maidenhead = opts(Representation::Maidenhead { pairs: 2 });
    assert_eq!(
        format_coordinate(wgs84(40.5, -75.0), maidenhead).unwrap(),
        "FN20"
    );
    let mgrs = opts(Representation::Mgrs {
        precision: MgrsPrecision::M1,
    });
    assert_eq!(
        format_coordinate(wgs84(40.0, -75.0), mgrs).unwrap(),
        "18TWK0000027757"
    );
}

#[test]
fn format_coordinate_reports_typed_errors() {
    let ny = wgs84(40.7128, -74.006);
    match format_coordinate(ny, opts(dd(Some(16)))) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "decimals"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
    // The sexagesimal caps are the most that round-trip: DMS 9, DDM 12.
    let (symbols, hemisphere) = (SymbolStyle::Unicode, HemisphereStyle::Cardinal);
    let dms = |p| Representation::Dms {
        second_decimals: Some(p),
        symbols,
        hemisphere,
    };
    let ddm = |p| Representation::Ddm {
        minute_decimals: Some(p),
        symbols,
        hemisphere,
    };
    assert!(format_coordinate(ny, opts(dms(9))).is_ok());
    assert!(format_coordinate(ny, opts(ddm(12))).is_ok());
    for (r, field) in [(dms(10), "second_decimals"), (ddm(13), "minute_decimals")] {
        let refused = format_coordinate(ny, opts(r));
        assert!(matches!(&refused, Err(GeoError::InvalidValue { field: f, .. }) if f == field));
    }
    assert!(matches!(
        format_coordinate(ny, opts(Representation::Geohash { length: 13 })),
        Err(GeoError::InvalidValue { .. })
    ));
    let gcj = coordinate_new(39.915, 116.404, Crs::Gcj02).unwrap();
    assert!(matches!(
        format_coordinate(gcj, opts(Representation::Maidenhead { pairs: 3 })),
        Err(GeoError::CrsMismatch { .. })
    ));
}

#[test]
fn format_coordinate_honours_a_comma_separator() {
    let de = FormatOptions {
        representation: dd(Some(2)),
        decimal_separator: decimal_separator_for_locale("de-DE".to_string()),
    };
    assert_eq!(
        format_coordinate(wgs84(40.7128, -74.006), de).unwrap(),
        "40,71 -74,01"
    );
}

#[test]
fn format_fix_derives_precision_from_accuracy() {
    let fix = Fix {
        coord: wgs84(40.7128, -74.006),
        accuracy: Some(Accuracy {
            kind: AccuracyKind::OneSigma,
            horizontal_m: Some(1000.0),
            vertical_m: None,
        }),
        timestamp: None,
        source: None,
    };
    assert_eq!(
        format_fix(fix.clone(), opts(dd(None))).unwrap(),
        "40.71, -74.01"
    );
    // The default options leave the decimal count unset, so they derive it too.
    assert_eq!(
        format_fix(fix, format_options_default()).unwrap(),
        "40.71, -74.01"
    );
}

#[test]
fn format_options_default_is_signed_dd_with_a_point() {
    assert_eq!(format_options_default(), opts(dd(None)));
    assert_eq!(
        format_coordinate(wgs84(40.7128, -74.006), format_options_default()).unwrap(),
        "40.712800, -74.006000"
    );
}

#[test]
fn decimal_separator_for_locale_follows_the_heuristic() {
    let sep = |tag: &str| decimal_separator_for_locale(tag.to_string());
    assert_eq!(sep("en-US"), DecimalSeparator::Point);
    assert_eq!(sep("de-DE"), DecimalSeparator::Comma);
    assert_eq!(sep("fr_CA"), DecimalSeparator::Comma);
    assert_eq!(sep("de-CH"), DecimalSeparator::Point);
    assert_eq!(sep("not a tag"), DecimalSeparator::Point);
}

// --- Parsing ---

#[test]
fn parse_coordinate_records_the_assumed_axis_order() {
    let fix = parse_coordinate("40.7128, -74.006".to_string()).unwrap();
    assert_eq!(fix.coord, wgs84(40.7128, -74.006));
    let source = fix.source.unwrap();
    assert_eq!(source.raw, "40.7128, -74.006");
    assert_eq!(source.axis_order, Some(AxisOrder::LatLon));
    assert_eq!(source.confidence.value, 0.7);
    assert_eq!(source.notes, vec!["axis order assumed from the default"]);
}

#[test]
fn parse_coordinate_with_and_parse_text_with_read_a_decimal_comma() {
    let options = ParseOptions {
        decimal_separator: DecimalSeparator::Comma,
        ..parse_options_default()
    };
    for fix in [
        parse_coordinate_with("40,7128 -74,006".to_string(), options).unwrap(),
        parse_text_with("40,7128, -74,006".to_string(), options).unwrap(),
    ] {
        assert_eq!(fix.coord, wgs84(40.7128, -74.006));
        assert_eq!(fix.source.unwrap().axis_order, Some(AxisOrder::LatLon));
    }
}

#[test]
fn parse_text_reads_free_text_with_the_default_options_and_rejects_grids() {
    let fix = parse_text("40°42′46″N 74°00′22″W".to_string()).unwrap();
    assert!((fix.coord.lat - 40.712_78).abs() < 1e-5, "{fix:?}");
    assert!((fix.coord.lon + 74.006_11).abs() < 1e-5, "{fix:?}");
    let fix = parse_text("40.7128, -74.006".to_string()).unwrap();
    assert_eq!(fix.coord, wgs84(40.7128, -74.006));
    assert_eq!(fix.source.unwrap().axis_order, Some(AxisOrder::LatLon));
    assert!(matches!(
        parse_text("4042.768, -7400.36".to_string()),
        Err(GeoError::Parse {
            kind: ParseErrorKind::Ambiguous,
            ..
        })
    ));
    // A grid reference is not free text, though `parse_coordinate` reads it.
    assert!(matches!(
        parse_text("87G7PX7V+4J".to_string()),
        Err(GeoError::Parse {
            kind: ParseErrorKind::Syntax,
            ..
        })
    ));
    assert!(parse_coordinate("87G7PX7V+4J".to_string()).is_ok());
}

#[test]
fn parse_text_with_rejects_an_expected_grid_instead_of_ignoring_it() {
    let options = ParseOptions {
        expect_grid: Some(GridKind::Geohash),
        ..parse_options_default()
    };
    assert!(matches!(
        parse_text_with("40.7128, -74.006".to_string(), options),
        Err(GeoError::InvalidValue { ref field, .. }) if field == "expect_grid"
    ));
    // The same options still read the text through `parse_coordinate_with`.
    let fix = parse_coordinate_with("40.7128, -74.006".to_string(), options).unwrap();
    assert_eq!(fix.coord, wgs84(40.7128, -74.006));
}

#[test]
fn parse_coordinate_detects_mgrs_and_decodes_an_expected_geohash() {
    let mgrs = parse_coordinate("18TWK0000027757".to_string()).unwrap();
    let accuracy = mgrs.accuracy.unwrap();
    assert_eq!(accuracy.kind, AccuracyKind::Bound);
    assert!(accuracy.horizontal_m.unwrap() < 1.0);
    assert!(parse_coordinate("ezs42".to_string()).is_err());
    let geohash = FormatOptions {
        representation: Representation::Geohash { length: 5 },
        decimal_separator: DecimalSeparator::Point,
    };
    let options = format_options_parse_options(geohash);
    assert_eq!(options.expect_grid, Some(GridKind::Geohash));
    let fix = parse_coordinate_with("ezs42".to_string(), options).unwrap();
    assert!((fix.coord.lat - 42.6).abs() < 0.03, "{fix:?}");
}

#[test]
fn parse_geo_uri_reads_altitude_and_uncertainty() {
    let fix = parse_geo_uri("geo:48.2,16.3,183;u=40".to_string()).unwrap();
    assert_eq!(
        fix.coord,
        Coordinate {
            lat: 48.2,
            lon: 16.3,
            height: Some(Height::Ellipsoidal { meters: 183.0 }),
            crs: Crs::Wgs84,
        }
    );
    assert_eq!(
        fix.accuracy,
        Some(Accuracy {
            kind: AccuracyKind::Unspecified,
            horizontal_m: Some(40.0),
            vertical_m: None,
        })
    );
    let source = fix.source.unwrap();
    assert_eq!(source.axis_order, Some(AxisOrder::LatLon));
    assert_eq!(source.confidence.value, 1.0);
    assert!(matches!(
        parse_geo_uri("40.7128, -74.006".to_string()),
        Err(GeoError::Parse {
            kind: ParseErrorKind::Syntax,
            ..
        })
    ));
}
