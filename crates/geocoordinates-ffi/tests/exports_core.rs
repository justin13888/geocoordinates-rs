//! Calls every core FFI export (coordinates, China datums, angles, lengths,
//! validation, `Fix`, formatting, parsing) at least once with an asserted value.
//!
//! The China reference triple is the crate's own vector from
//! `src/test_support.rs` (coordtransform-rs). Other values are pinned to what
//! the core crate produces today, so a behaviour change in the mirror or the
//! core shows up here as a failing assertion.

use geocoordinates_ffi::*;

/// Reference triple from `src/test_support.rs`, `(lat, lon)`.
const WGS: (f64, f64) = (39.915, 116.404);
const GCJ: (f64, f64) = (39.916_404_281_501_64, 116.410_244_499_169_38);
const BD: (f64, f64) = (39.922_699_552_216_216, 116.416_627_243_787_33);

fn close(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() <= tol,
        "expected {expected} ± {tol}, got {actual}"
    );
}

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

/// A WGS84 record built by hand with an out-of-range latitude, bypassing
/// `coordinate_new`'s validation.
const OUT_OF_RANGE: Coordinate = Coordinate {
    lat: 91.0,
    lon: 0.0,
    height: None,
    crs: Crs::Wgs84,
};

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

// --- Coordinate constructors ---

#[test]
fn coordinate_constructors_tag_their_crs() {
    let expect = |c: Coordinate, crs: Crs| {
        assert_eq!(
            c,
            Coordinate {
                lat: 1.5,
                lon: -2.5,
                height: None,
                crs,
            }
        );
    };
    expect(wgs84(1.5, -2.5), Crs::Wgs84);
    expect(coordinate_new(1.5, -2.5, Crs::Gcj02).unwrap(), Crs::Gcj02);
    expect(coordinate_new(1.5, -2.5, Crs::Bd09).unwrap(), Crs::Bd09);
}

#[test]
fn coordinate_new_rejects_out_of_range_and_non_finite_input() {
    match coordinate_new(91.0, 0.0, Crs::Wgs84) {
        Err(GeoError::OutOfRange { lat, lon }) => assert_eq!((lat, lon), (91.0, 0.0)),
        other => panic!("expected OutOfRange, got {other:?}"),
    }
    match coordinate_new(0.0, f64::NAN, Crs::Wgs84) {
        Err(GeoError::OutOfRange { lat, lon }) => {
            assert_eq!(lat, 0.0);
            assert!(lon.is_nan());
        }
        other => panic!("expected OutOfRange, got {other:?}"),
    }
}

#[test]
fn exports_reject_a_hand_built_out_of_range_record() {
    let bad = OUT_OF_RANGE;
    let is_out_of_range =
        |e: GeoError| matches!(e, GeoError::OutOfRange { lat, lon } if (lat, lon) == (91.0, 0.0));
    assert!(is_out_of_range(convert(bad, Crs::Gcj02).unwrap_err()));
    assert!(is_out_of_range(
        geodesic_distance_m(bad, wgs84(0.0, 0.0)).unwrap_err()
    ));
    assert!(is_out_of_range(
        format_coordinate(bad, opts(dd(Some(4)))).unwrap_err()
    ));
}

// --- China datums: exact forward ---

#[test]
fn china_forward_conversions_match_the_reference_vector() {
    let g = wgs84_to_gcj02(Wgs84 {
        lat: WGS.0,
        lon: WGS.1,
    })
    .unwrap();
    close(g.lat, GCJ.0, 1e-9);
    close(g.lon, GCJ.1, 1e-9);
    let b = wgs84_to_bd09(Wgs84 {
        lat: WGS.0,
        lon: WGS.1,
    })
    .unwrap();
    close(b.lat, BD.0, 1e-9);
    close(b.lon, BD.1, 1e-9);
    let gb = gcj02_to_bd09(Gcj02 {
        lat: GCJ.0,
        lon: GCJ.1,
    })
    .unwrap();
    close(gb.lat, BD.0, 1e-12);
    close(gb.lon, BD.1, 1e-12);
}

// --- China datums: approximate inverse ---

#[test]
fn gcj02_inverses_land_within_their_stated_bounds() {
    let gcj = Gcj02 {
        lat: GCJ.0,
        lon: GCJ.1,
    };
    let fast = gcj02_to_wgs84_fast(gcj).unwrap();
    assert_eq!(fast.max_error_m, 6.0);
    close(fast.lat, 39.915_001_202_576_08, 1e-12);
    close(fast.lon, 116.404_001_915_379_34, 1e-12);
    let refined = gcj02_to_wgs84_refined(gcj).unwrap();
    assert_eq!(refined.max_error_m, 0.5);
    close(refined.lat, WGS.0, 1e-9);
    close(refined.lon, WGS.1, 1e-9);
}

#[test]
fn bd09_inverses_land_within_their_stated_bounds() {
    let bd = Bd09 {
        lat: BD.0,
        lon: BD.1,
    };
    let fast = bd09_to_gcj02_fast(bd).unwrap();
    assert_eq!(fast.max_error_m, 0.5);
    close(fast.lat, 39.916_403_256_363_04, 1e-12);
    close(fast.lon, 116.410_245_062_296_18, 1e-12);
    let refined = bd09_to_gcj02_refined(bd).unwrap();
    assert_eq!(refined.max_error_m, 0.5);
    close(refined.lat, GCJ.0, 1e-9);
    close(refined.lon, GCJ.1, 1e-9);
    // The composed inverse carries the summed bound of its two legs.
    let wgs = bd09_to_wgs84_refined(bd).unwrap();
    assert_eq!(wgs.max_error_m, 1.0);
    close(wgs.lat, WGS.0, 1e-9);
    close(wgs.lon, WGS.1, 1e-9);
}

// --- Baidu Web Mercator ---

#[test]
fn baidu_mercator_round_trips_through_bd09_and_coordinate() {
    let merc = baidu_mercator_from_bd09(Bd09 {
        lat: BD.0,
        lon: BD.1,
    })
    .unwrap();
    close(merc.x, 12_959_580.673_891_693, 1e-6);
    close(merc.y, 4_827_036.867_073_46, 1e-6);
    let back = baidu_mercator_to_bd09(merc).unwrap();
    close(back.lat, 39.922_699_628_640_58, 1e-12);
    close(back.lon, 116.416_627_243_787_27, 1e-12);
    let coord = baidu_mercator_to_coordinate(merc).unwrap();
    assert_eq!(coord.crs, Crs::Bd09);
    assert_eq!((coord.lat, coord.lon), (back.lat, back.lon));
    assert_eq!(coord.height, None);
    let from_coord =
        baidu_mercator_from_coordinate(coordinate_new(BD.0, BD.1, Crs::Bd09).unwrap()).unwrap();
    assert_eq!(from_coord, merc);
}

#[test]
fn baidu_mercator_rejects_a_non_bd09_coordinate() {
    match baidu_mercator_from_coordinate(wgs84(BD.0, BD.1)) {
        Err(GeoError::CrsMismatch { expected, found }) => {
            assert_eq!(expected, Crs::Bd09);
            assert_eq!(found, Crs::Wgs84);
        }
        other => panic!("expected CrsMismatch, got {other:?}"),
    }
}

#[test]
fn out_of_china_separates_beijing_from_london() {
    assert!(!out_of_china(WGS.0, WGS.1));
    assert!(out_of_china(51.5074, -0.1278));
}

#[test]
fn haversine_distance_is_pinned() {
    let ny = wgs84(40.7128, -74.006);
    let london = wgs84(51.5074, -0.1278);
    close(
        haversine_distance_m(ny, london).unwrap(),
        5_570_229.873_656_523,
        1e-6,
    );
    assert_eq!(haversine_distance_m(ny, ny).unwrap(), 0.0);
}

// --- Angle encodings ---

#[test]
fn degrees_decompose_into_dms_and_ddm() {
    let dms = dms_from_degrees(40.712_775_3, Axis::Latitude).unwrap();
    assert_eq!((dms.degrees, dms.minutes), (40, 42));
    close(dms.seconds, 45.991_08, 1e-9);
    assert_eq!(dms.hemisphere, Hemisphere::North);
    let ddm = ddm_from_degrees(-74.006, Axis::Longitude).unwrap();
    assert_eq!(ddm.degrees, 74);
    close(ddm.minutes, 0.36, 1e-9);
    assert_eq!(ddm.hemisphere, Hemisphere::West);
    // -0.0 reads as the positive hemisphere.
    let zero = dms_from_degrees(-0.0, Axis::Latitude).unwrap();
    assert_eq!(zero.hemisphere, Hemisphere::North);
}

#[test]
fn from_degrees_rejects_values_outside_the_axis() {
    for result in [
        dms_from_degrees(f64::NAN, Axis::Latitude).map(|_| ()),
        dms_from_degrees(90.5, Axis::Latitude).map(|_| ()),
        ddm_from_degrees(-180.5, Axis::Longitude).map(|_| ()),
    ] {
        assert!(
            matches!(&result, Err(GeoError::InvalidValue { field, .. }) if field == "decimal degrees"),
            "{result:?}"
        );
    }
}

#[test]
fn dms_and_ddm_convert_between_each_other_and_back_to_degrees() {
    let dms = Dms {
        degrees: 40,
        minutes: 42,
        seconds: 45.991_08,
        hemisphere: Hemisphere::North,
    };
    let ddm = dms_to_ddm(dms).unwrap();
    assert_eq!((ddm.degrees, ddm.hemisphere), (40, Hemisphere::North));
    close(ddm.minutes, 42.766_518, 1e-9);
    close(dms_to_degrees(dms).unwrap(), 40.712_775_3, 1e-12);

    let west = Ddm {
        degrees: 74,
        minutes: 0.36,
        hemisphere: Hemisphere::West,
    };
    let back = ddm_to_dms(west).unwrap();
    assert_eq!((back.degrees, back.minutes), (74, 0));
    close(back.seconds, 21.6, 1e-9);
    assert_eq!(back.hemisphere, Hemisphere::West);
    close(ddm_to_degrees(west).unwrap(), -74.006, 1e-12);
}

#[test]
fn hand_built_angle_records_are_validated_on_use() {
    let good_dms = Dms {
        degrees: 40,
        minutes: 59,
        seconds: 59.5,
        hemisphere: Hemisphere::South,
    };
    assert_eq!(dms_validate(good_dms), Ok(()));
    let good_ddm = Ddm {
        degrees: 180,
        minutes: 0.0,
        hemisphere: Hemisphere::West,
    };
    assert_eq!(ddm_validate(good_ddm), Ok(()));

    let invalid = |result: Result<(), GeoError>, expected: &str| {
        assert!(
            matches!(&result, Err(GeoError::InvalidValue { field, .. }) if field == expected),
            "expected InvalidValue {{ field: {expected} }}, got {result:?}"
        );
    };
    let minutes_60 = Dms {
        minutes: 60,
        ..good_dms
    };
    invalid(dms_validate(minutes_60), "minutes");
    invalid(dms_to_degrees(minutes_60).map(|_| ()), "minutes");
    invalid(dms_to_ddm(minutes_60).map(|_| ()), "minutes");
    invalid(
        dms_validate(Dms {
            seconds: 60.0,
            ..good_dms
        }),
        "seconds",
    );
    invalid(
        dms_validate(Dms {
            degrees: 91,
            ..good_dms
        }),
        "degrees",
    );
    let past_antimeridian = Ddm {
        minutes: 0.5,
        ..good_ddm
    };
    invalid(ddm_validate(past_antimeridian), "degrees");
    invalid(ddm_to_degrees(past_antimeridian).map(|_| ()), "degrees");
    invalid(ddm_to_dms(past_antimeridian).map(|_| ()), "degrees");
    invalid(
        ddm_validate(Ddm {
            minutes: f64::NAN,
            ..good_ddm
        }),
        "degrees",
    );
    invalid(
        ddm_validate(Ddm {
            degrees: 10,
            minutes: f64::NAN,
            hemisphere: Hemisphere::North,
        }),
        "minutes",
    );
}

#[test]
fn hemisphere_sign_is_negative_only_for_south_and_west() {
    assert_eq!(hemisphere_sign(Hemisphere::North), 1.0);
    assert_eq!(hemisphere_sign(Hemisphere::East), 1.0);
    assert_eq!(hemisphere_sign(Hemisphere::South), -1.0);
    assert_eq!(hemisphere_sign(Hemisphere::West), -1.0);
}

#[test]
fn normalization_helpers_handle_antimeridian_and_poles() {
    assert_eq!(wrap_longitude(180.0), -180.0);
    assert_eq!(wrap_longitude(190.0), -170.0);
    assert_eq!(clamp_latitude(91.0), 90.0);
    assert_eq!(clamp_latitude(-95.0), -90.0);
    assert_eq!(normalize_degrees(-1.0), 359.0);
    assert_eq!(normalize_degrees(720.0), 0.0);
    // Half-open: a tiny negative never rounds up to 360, and -0.0 becomes +0.0.
    assert_eq!(normalize_degrees(-1e-20), 0.0);
    assert!(normalize_degrees(-0.0).is_sign_positive());
}

#[test]
fn length_unit_conversions_are_exact() {
    assert_eq!(length_from_unit(1.0, LengthUnit::Kilometer), 1000.0);
    assert_eq!(length_from_unit(1.0, LengthUnit::Foot), 0.3048);
    close(
        length_from_unit(3937.0, LengthUnit::UsSurveyFoot),
        1200.0,
        1e-9,
    );
    assert_eq!(length_from_unit(2.0, LengthUnit::Meter), 2.0);
    close(length_to_unit(1852.0, LengthUnit::NauticalMile), 1.0, 1e-12);
    assert_eq!(length_from_unit(1.0, LengthUnit::Mile), 1609.344);
    assert_eq!(length_from_unit(1.0, LengthUnit::Yard), 0.9144);
}

// --- Validation and Fix ---

#[test]
fn coordinate_validate_rejects_out_of_range_latitude() {
    assert!(coordinate_validate(wgs84(40.0, -74.0)).is_ok());
    match coordinate_validate(OUT_OF_RANGE) {
        Err(GeoError::OutOfRange { lat, lon }) => assert_eq!((lat, lon), (91.0, 0.0)),
        other => panic!("expected OutOfRange, got {other:?}"),
    }
}

#[test]
fn coordinate_validate_rejects_a_non_finite_height() {
    let coord = Coordinate {
        height: Some(Height::Ellipsoidal {
            meters: f64::INFINITY,
        }),
        ..wgs84(40.0, -74.0)
    };
    match coordinate_validate(coord) {
        Err(GeoError::InvalidValue { .. }) => {}
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn null_island_detection_uses_a_micro_degree_window() {
    assert!(coordinate_is_null_island(wgs84(0.0, 0.0)).unwrap());
    assert!(coordinate_is_null_island(wgs84(1e-7, -1e-7)).unwrap());
    assert!(!coordinate_is_null_island(wgs84(1e-5, 0.0)).unwrap());
}

#[test]
fn null_island_and_fix_from_coord_reject_an_out_of_range_record() {
    match coordinate_is_null_island(OUT_OF_RANGE) {
        Err(GeoError::OutOfRange { lat, lon }) => assert_eq!((lat, lon), (91.0, 0.0)),
        other => panic!("expected OutOfRange, got {other:?}"),
    }
    match fix_from_coord(OUT_OF_RANGE) {
        Err(GeoError::OutOfRange { lat, lon }) => assert_eq!((lat, lon), (91.0, 0.0)),
        other => panic!("expected OutOfRange, got {other:?}"),
    }
}

#[test]
fn fix_from_coord_carries_no_metadata() {
    let coord = wgs84(WGS.0, WGS.1);
    let fix = fix_from_coord(coord).unwrap();
    assert_eq!(
        fix,
        Fix {
            coord,
            accuracy: None,
            timestamp: None,
            source: None,
        }
    );
}

#[test]
fn confidence_new_clamps_into_the_unit_interval() {
    assert_eq!(confidence_new(2.0).value, 1.0);
    assert_eq!(confidence_new(-0.5).value, 0.0);
    assert_eq!(confidence_new(f64::NAN).value, 0.0);
    assert_eq!(confidence_new(0.25).value, 0.25);
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
    assert!(mgrs.accuracy.unwrap().horizontal_m.unwrap() < 1.0);
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
fn from_geo_uri_reads_altitude_and_uncertainty() {
    let fix = from_geo_uri("geo:48.2,16.3,183;u=40".to_string()).unwrap();
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
            horizontal_m: Some(40.0),
            vertical_m: None,
        })
    );
    let source = fix.source.unwrap();
    assert_eq!(source.axis_order, Some(AxisOrder::LatLon));
    assert_eq!(source.confidence.value, 1.0);
    assert!(matches!(
        from_geo_uri("40.7128, -74.006".to_string()),
        Err(GeoError::Parse { .. })
    ));
}
