//! Grid detection in [`parse_coordinate_with`], the [`ParseOptions`]
//! contract, and the never-panic property of every string parser.

use proptest::prelude::*;

use super::*;
use crate::format::{DecimalSeparator, FormatOptions, Representation, format};
use crate::grids::MgrsPrecision;
use crate::test_support::{assert_close, assert_within_meters};

fn expecting(kind: GridKind) -> ParseOptions {
    ParseOptions {
        expect_grid: Some(kind),
        ..ParseOptions::default()
    }
}

fn accuracy_m(fix: &Fix) -> f64 {
    fix.accuracy.and_then(|a| a.horizontal_m).unwrap()
}

#[test]
fn default_options_read_lat_first_point_decimal_text_and_expect_no_grid() {
    let options = ParseOptions::default();
    assert_eq!(options.default_axis_order, AxisOrder::LatLon);
    assert_eq!(options.decimal_separator, DecimalSeparator::Point);
    assert_eq!(options.expect_grid, None);
}

#[test]
fn mgrs_is_always_detected() {
    for input in [
        "18TWK0000027757",
        "18T WK 00000 27757",
        " 18twk0000027757\t",
    ] {
        let fix = parse_coordinate(input).unwrap_or_else(|e| panic!("{input:?}: {e}"));
        let expected = Mgrs::try_from(input).unwrap().to_coordinate();
        assert_eq!(fix.coord, expected.into_inner(), "{input:?}");
        assert_close(accuracy_m(&fix), expected.max_error_m(), 1e-9);
        let source = fix.source.unwrap();
        assert_eq!(source.raw, input);
        assert_eq!(source.axis_order, None);
        assert_close(source.confidence.value(), 1.0, 1e-12);
    }
}

#[test]
fn ambiguous_grids_and_utm_are_not_detected_by_default() {
    // A geohash, a Maidenhead locator and a UTM triple are not coordinates
    // unless asked for; neither are a lone number or a bare word, though
    // both are valid geohashes.
    for input in ["ezs42", "FN20", "18N 500000 4427757", "42", "bed"] {
        let r = parse_coordinate(input);
        assert!(matches!(r, Err(Error::Parse(_))), "{input:?}: {r:?}");
    }
}

#[test]
fn expect_grid_decodes_the_ambiguous_grids() {
    let cases = [
        (
            GridKind::Geohash,
            "ezs42",
            Geohash::try_from("ezs42").unwrap().decode(),
        ),
        (
            GridKind::Geohash,
            "42",
            Geohash::try_from("42").unwrap().decode(),
        ),
        (
            GridKind::Maidenhead,
            "FN20",
            Maidenhead::try_from("FN20").unwrap().decode(),
        ),
    ];
    for (kind, input, expected) in cases {
        let fix = parse_coordinate_with(input, &expecting(kind)).unwrap();
        assert_eq!(fix.coord, expected.into_inner(), "{input:?}");
        assert_close(accuracy_m(&fix), expected.max_error_m(), 1e-9);
        assert_eq!(fix.source.unwrap().axis_order, None);
    }
}

#[test]
fn expect_grid_comes_before_the_always_detected_grids() {
    // `18TWK` is both a 100 km MGRS square and a valid geohash; expecting a
    // geohash reads it as one, and by default it is the MGRS square.
    let as_geohash = parse_coordinate_with("18TWK", &expecting(GridKind::Geohash)).unwrap();
    let geohash = Geohash::try_from("18TWK").unwrap().decode();
    assert_eq!(as_geohash.coord, geohash.into_inner());
    let as_mgrs = parse_coordinate("18TWK").unwrap();
    assert_eq!(
        as_mgrs.coord,
        Mgrs::try_from("18TWK")
            .unwrap()
            .to_coordinate()
            .into_inner()
    );
    // Expecting an always-detected kind changes nothing about what matches.
    for kind in [GridKind::PlusCode, GridKind::Mgrs] {
        let fix = parse_coordinate_with("8FVC2222+22", &expecting(kind)).unwrap();
        assert_close(fix.coord.lat(), 47.0000625, 1e-6);
    }
}

#[test]
fn input_that_is_not_the_expected_grid_still_parses_by_the_other_routes() {
    let options = expecting(GridKind::Maidenhead);
    let text = parse_coordinate_with("40.7128, -74.006", &options).unwrap();
    assert_close(text.coord.lat(), 40.7128, 1e-9);
    let geo = parse_coordinate_with("geo:13.4125,103.8667", &options).unwrap();
    assert_close(geo.coord.lat(), 13.4125, 1e-9);
    let mgrs = parse_coordinate_with("18TWK0000027757", &options).unwrap();
    assert_eq!(mgrs.source.unwrap().axis_order, None);
}

#[test]
fn a_failed_expected_grid_explains_both_readings() {
    let r = parse_coordinate_with("zz99", &expecting(GridKind::Maidenhead));
    let Err(Error::Parse(message)) = r else {
        panic!("{r:?}");
    };
    assert!(
        message.starts_with("not a Maidenhead reference ("),
        "{message}"
    );
    assert!(message.contains("two components"), "{message}");
    // Without an expected grid the text error stands alone.
    let Err(Error::Parse(plain)) = parse_coordinate("zz99") else {
        panic!("zz99 parsed");
    };
    assert!(!plain.contains("reference"), "{plain}");
}

#[test]
fn parse_options_expect_the_grid_their_representation_renders() {
    let grid_cases = [
        (
            Representation::PlusCode { length: 10 },
            Some(GridKind::PlusCode),
        ),
        (
            Representation::Geohash { length: 5 },
            Some(GridKind::Geohash),
        ),
        (
            Representation::Maidenhead { pairs: 2 },
            Some(GridKind::Maidenhead),
        ),
        (
            Representation::Mgrs {
                precision: MgrsPrecision::M1,
            },
            Some(GridKind::Mgrs),
        ),
        (FormatOptions::default().representation, None),
    ];
    for separator in [DecimalSeparator::Point, DecimalSeparator::Comma] {
        for (representation, expect_grid) in grid_cases {
            let options = FormatOptions {
                representation,
                decimal_separator: separator,
            };
            let parse = options.parse_options();
            assert_eq!(parse.expect_grid, expect_grid, "{options:?}");
            assert_eq!(parse.decimal_separator, separator);
            assert_eq!(parse.default_axis_order, AxisOrder::LatLon);
        }
    }
}

#[test]
fn every_grid_rendering_round_trips_through_parse_options() {
    let c = Coordinate::wgs84(40.7128, -74.006).unwrap();
    for representation in [
        Representation::PlusCode { length: 10 },
        Representation::Geohash { length: 5 },
        Representation::Geohash { length: 2 },
        Representation::Maidenhead { pairs: 1 },
        Representation::Maidenhead { pairs: 3 },
        Representation::Mgrs {
            precision: MgrsPrecision::M10,
        },
    ] {
        let options = FormatOptions {
            representation,
            ..FormatOptions::default()
        };
        let text = format(&c, &options).unwrap();
        let fix = parse_coordinate_with(&text, &options.parse_options())
            .unwrap_or_else(|e| panic!("{text:?}: {e}"));
        // The original lies within the decoded cell's reported bound.
        assert_within_meters(&fix.coord, &c, accuracy_m(&fix));
    }
}

#[test]
fn concatenated_ddmm_with_hemispheres_is_not_mistaken_for_mgrs() {
    let fix = parse_coordinate("4042.766N 07400.456W").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 42.766 / 60.0, 1e-9);
    assert!(fix.accuracy.is_none());
}

#[cfg(feature = "serde")]
#[test]
fn parse_options_and_grid_kind_round_trip_through_serde() {
    let options = ParseOptions {
        default_axis_order: AxisOrder::LonLat,
        decimal_separator: DecimalSeparator::Comma,
        expect_grid: Some(GridKind::Maidenhead),
    };
    let json = serde_json::to_string(&options).unwrap();
    assert_eq!(
        serde_json::from_str::<ParseOptions>(&json).unwrap(),
        options
    );
}

proptest! {
    /// No string parser panics on arbitrary input, under any options: each
    /// either parses or returns an error.
    #[test]
    fn string_parsers_never_panic(
        input in prop_oneof![
            any::<String>(),
            "[0-9NSEWnsewdmDM.,;:°'\" +\\-geo]{0,24}",
            "[0-9A-Za-z +]{0,16}",
        ],
        lon_lat in any::<bool>(),
        comma in any::<bool>(),
        grid in prop::option::of(prop::sample::select(vec![
            GridKind::PlusCode,
            GridKind::Geohash,
            GridKind::Maidenhead,
            GridKind::Mgrs,
        ])),
    ) {
        let options = ParseOptions {
            default_axis_order: if lon_lat { AxisOrder::LonLat } else { AxisOrder::LatLon },
            decimal_separator: if comma { DecimalSeparator::Comma } else { DecimalSeparator::Point },
            expect_grid: grid,
        };
        let _ = parse_coordinate_with(&input, &options);
        let _ = text::parse_with(&input, &options);
        let _ = from_geo_uri(&input);
        #[cfg(feature = "nmea")]
        let _ = sensors::from_nmea_sentence(&input);
        #[cfg(feature = "serde")]
        {
            let _ = serde_json::from_str::<ParseOptions>(&input);
            let _ = serde_json::from_str::<Fix>(&input);
            let _ = serde_json::from_str::<Mgrs>(&input);
        }
    }
}
