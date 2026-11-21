//! `format(parse(format(c))) == format(c)`: rendering what was parsed back
//! from a rendering reproduces it exactly, for every representation, each of
//! its settings the module docs promise is stable, and both decimal
//! separators; and the value parsed back stays within the rendered precision.

use proptest::prelude::*;

use super::*;
use crate::parse::{ParseOptions, parse_coordinate, parse_coordinate_with};
use crate::test_support::assert_within_meters;

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
        let text = format(c, FormatOptions::default()).unwrap();
        let parsed = parse_coordinate(&text).unwrap();
        assert_within_meters(parsed.coord, c, 0.2);
    }
}

/// A latitude biased toward the places formatting breaks: the poles, the
/// equator, and values that round to zero or to a pole at the rendered
/// precision.
fn latitude() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => -90.0..=90.0_f64,
        1 => prop::sample::select(vec![-90.0, 90.0, 0.0, -0.0]),
        1 => -1e-6..1e-6_f64,
        1 => (89.999_999..=90.0_f64).prop_flat_map(|v| prop_oneof![Just(v), Just(-v)]),
    ]
}

/// A longitude biased toward the antimeridian, the prime meridian, and values
/// that round to zero or to ±180 at the rendered precision.
fn longitude() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => -180.0..=180.0_f64,
        1 => prop::sample::select(vec![-180.0, 180.0, 0.0, -0.0]),
        1 => -1e-6..1e-6_f64,
        1 => (179.999_999..=180.0_f64).prop_flat_map(|v| prop_oneof![Just(v), Just(-v)]),
    ]
}

const DD_MAX: u8 = Representation::MAX_DD_DECIMALS;
const DMS_MAX: u8 = Representation::MAX_DMS_SECOND_DECIMALS;
const DDM_MAX: u8 = Representation::MAX_DDM_MINUTE_DECIMALS;

fn symbols() -> impl Strategy<Value = SymbolStyle> {
    prop::sample::select(vec![
        SymbolStyle::Unicode,
        SymbolStyle::Ascii,
        SymbolStyle::Letters,
    ])
}

fn hemisphere() -> impl Strategy<Value = HemisphereStyle> {
    prop::sample::select(vec![HemisphereStyle::Signed, HemisphereStyle::Cardinal])
}

fn separator() -> impl Strategy<Value = DecimalSeparator> {
    prop::sample::select(vec![DecimalSeparator::Point, DecimalSeparator::Comma])
}

/// Every representation, each with every value of its own settings that
/// [`format`] accepts: the angle representations up to their capped decimal
/// counts. Why the caps sit where they do is pinned in
/// `angle_decimal_caps_are_the_last_counts_that_round_trip`.
fn representation() -> impl Strategy<Value = Representation> {
    prop_oneof![
        3 => angle_representation(),
        1 => prop::sample::select(vec![2_u8, 4, 6, 8, 10, 11, 12, 13, 14, 15])
            .prop_map(|length| Representation::PlusCode { length }),
        1 => (1..=12_u8).prop_map(|length| Representation::Geohash { length }),
        1 => (1..=3_u8).prop_map(|pairs| Representation::Maidenhead { pairs }),
        1 => prop::sample::select(vec![
            MgrsPrecision::Km100,
            MgrsPrecision::Km10,
            MgrsPrecision::Km1,
            MgrsPrecision::M100,
            MgrsPrecision::M10,
            MgrsPrecision::M1,
        ])
        .prop_map(|precision| Representation::Mgrs { precision }),
    ]
}

/// DD, DMS and DDM, each up to its capped decimal count, or unset.
fn angle_representation() -> impl Strategy<Value = Representation> {
    let decimals = |max: u8| prop::option::of(0..=max);
    prop_oneof![
        (decimals(DD_MAX), hemisphere())
            .prop_map(|(decimals, hemisphere)| dd(decimals, hemisphere)),
        (decimals(DMS_MAX), symbols(), hemisphere()).prop_map(|(p, s, h)| dms(p, s, h)),
        (decimals(DDM_MAX), symbols(), hemisphere()).prop_map(|(p, s, h)| ddm(p, s, h)),
    ]
}

/// Options for each generated representation, with either separator.
fn options_of(
    representation: impl Strategy<Value = Representation>,
) -> impl Strategy<Value = FormatOptions> {
    (representation, separator()).prop_map(|(representation, decimal_separator)| {
        FormatOptions::default()
            .with_representation(representation)
            .with_decimal_separator(decimal_separator)
    })
}

fn options() -> impl Strategy<Value = FormatOptions> {
    options_of(representation())
}

/// The angle representations at exactly their capped decimal count, the
/// finest rendering [`format`] accepts and so the one most likely to drift.
fn capped_options() -> impl Strategy<Value = FormatOptions> {
    let representation = prop_oneof![
        hemisphere().prop_map(|h| dd(Some(DD_MAX), h)),
        (symbols(), hemisphere()).prop_map(|(s, h)| dms(Some(DMS_MAX), s, h)),
        (symbols(), hemisphere()).prop_map(|(s, h)| ddm(Some(DDM_MAX), s, h)),
    ];
    options_of(representation)
}

/// Parse a rendering back to a coordinate through `parse_coordinate_with`
/// given `ParseOptions::for_format` of the options, for every representation.
fn reparse(text: &str, options: FormatOptions) -> Coordinate {
    parse_coordinate_with(text, ParseOptions::for_format(options))
        .unwrap_or_else(|e| panic!("{text:?} did not re-parse: {e}"))
        .coord
}

/// One trip: render, parse back, render again. Returns both renderings.
fn trip(lat: f64, lon: f64, options: FormatOptions) -> (String, String) {
    let first = fmt(wgs(lat, lon), options);
    let parsed = reparse(&first, options);
    (first, fmt(parsed, options))
}

/// Half the last unit an angle representation renders, in degrees: the most
/// a correctly rounded rendering can move a component. An unset count is the
/// documented default (DD 6 places, DMS 0.01″, DDM 0.001′).
fn half_rendered_unit(representation: Representation) -> f64 {
    let (places, units_per_degree) = match representation {
        Representation::DecimalDegrees { decimals, .. } => (decimals.unwrap_or(6), 1.0),
        Representation::Dms {
            second_decimals, ..
        } => (second_decimals.unwrap_or(2), 3600.0),
        Representation::Ddm {
            minute_decimals, ..
        } => (minute_decimals.unwrap_or(3), 60.0),
        other => unreachable!("{other:?} is not an angle representation"),
    };
    0.5 * 10_f64.powi(-i32::from(places)) / units_per_degree
}

/// What `f64` arithmetic may add to half a rendered unit through the
/// degree / minute / second split and the re-parse: far below the finest
/// default precision (DMS 0.01″, about 2.8e-6°).
const F64_SLACK_DEGREES: f64 = 1e-12;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// C11: a value that rounds to zero renders unsigned (and N / E), so the
    /// round trip holds at the equator, the prime meridian, and every value
    /// that rounds onto them. The pinned cases are in
    /// `rounding_to_zero_renders_unsigned`.
    #[test]
    fn format_parse_format_is_stable(
        lat in latitude(),
        lon in longitude(),
        options in options(),
    ) {
        let (first, second) = trip(lat, lon, options);
        prop_assert_eq!(first, second, "options {:?}", options);
    }

    /// The round trip holds at each angle representation's capped decimal
    /// count, not only somewhere below it.
    #[test]
    fn format_parse_format_is_stable_at_the_decimal_caps(
        lat in latitude(),
        lon in longitude(),
        options in capped_options(),
    ) {
        let (first, second) = trip(lat, lon, options);
        prop_assert_eq!(first, second, "options {:?}", options);
    }

    /// The round trip keeps the value, not only the text: each re-parsed
    /// component lies within half a unit of the last rendered place of the
    /// original, for DD, DMS and DDM at every decimal count, both separators,
    /// the poles and the antimeridian.
    #[test]
    fn reparsed_value_stays_within_half_a_rendered_unit(
        lat in latitude(),
        lon in longitude(),
        options in options_of(angle_representation()),
    ) {
        let text = fmt(wgs(lat, lon), options);
        let parsed = reparse(&text, options);
        let bound = half_rendered_unit(options.representation) + F64_SLACK_DEGREES;
        let (dlat, dlon) = ((parsed.lat() - lat).abs(), (parsed.lon() - lon).abs());
        prop_assert!(dlat <= bound, "{text:?}: lat {lat} drifted {dlat}° past {bound}°");
        prop_assert!(dlon <= bound, "{text:?}: lon {lon} drifted {dlon}° past {bound}°");
    }
}

/// The MGRS counterexamples the property found while the decoder returned the
/// nominal center of a square its grid-zone designator truncates (#149): that
/// center can lie in the neighbouring zone, band, or projection, so the
/// re-rendering named a different square. Each now decodes inside its own
/// designator and renders back unchanged.
#[test]
fn mgrs_truncated_square_decodes_inside_its_zone() {
    let km100 = opts(Representation::Mgrs {
        precision: MgrsPrecision::Km100,
    });
    for (lat, lon, rendering) in [
        // Zone 1's westernmost square, cut by the antimeridian (was `60NZF`).
        (0.0, -180.0, "01NAA"),
        // Zone 60's easternmost square, cut by it from the other side (was
        // `01NAB`).
        (1.0, 180.0, "60NZG"),
        // Norway: zone 32 widened west to 3°E across band V (was `31VDG`).
        (60.0, 3.001, "32VJM"),
        // Svalbard: zone 33 widened west to 9°E from band X's 72°N edge (was
        // `32WNE`).
        (72.001, 10.0, "33XUV"),
        // A band edge: band P starts at 8°N (was `31NEJ`).
        (8.0, 3.0, "31PEJ"),
        // The UTM / UPS seams: 84°N (was `YZA`) and 80°S (was `31CDM`).
        (83.99, 1.0, "31XDP"),
        (-80.001, 5.0, "BAZ"),
    ] {
        let (first, second) = trip(lat, lon, km100);
        assert_eq!(first, rendering, "({lat}, {lon})");
        assert_eq!(second, rendering, "({lat}, {lon})");
    }
}

/// The C11 counterexamples the property found before the sign was chosen
/// after rounding, pinned so the property is not the only record of them.
#[test]
fn rounding_to_zero_renders_unsigned() {
    // DD drops the sign of a value that rounds to zero.
    let dd_signed = opts(dd(Some(2), HemisphereStyle::Signed));
    let (first, second) = trip(-1e-9, 10.0, dd_signed);
    assert_eq!(first, "0.00, 10.00");
    assert_eq!(second, "0.00, 10.00");
    // So does DMS, so the re-parsed 0.0 renders identically.
    let dms_signed = opts(dms(Some(2), SymbolStyle::Unicode, HemisphereStyle::Signed));
    let (first, second) = trip(-1e-9, 10.0, dms_signed);
    assert_eq!(first, "0°00′00.00″ 10°00′00.00″");
    assert_eq!(second, "0°00′00.00″ 10°00′00.00″");
    // The cardinal style picks the hemisphere after rounding.
    let dd_cardinal = opts(dd(Some(2), HemisphereStyle::Cardinal));
    let (first, second) = trip(-1e-9, 10.0, dd_cardinal);
    assert_eq!(first, "0.00 N, 10.00 E");
    assert_eq!(second, "0.00 N, 10.00 E");
    // A negative zero is unsigned too.
    let (first, _) = trip(-0.0, -0.0, dd_signed);
    assert_eq!(first, "0.00, 0.00");
}

/// One count past each sexagesimal cap, the last digit lies below what an
/// `f64` keeps through the split and the re-parse, so a round trip moves it
/// by one: the renderer itself (below [`format`]'s check) shows the drift the
/// cap exists to prevent. [`format`] rejects that count, and renders the same
/// points stably at the cap.
#[test]
fn angle_decimal_caps_are_the_last_counts_that_round_trip() {
    let symbols = SymbolStyle::Unicode;
    let hemisphere = HemisphereStyle::Cardinal;
    let point = DecimalSeparator::Point;
    // Render through the unchecked renderer, re-parse, and render again.
    let raw_trip = |lat: f64, lon: f64, kind: AngleKind, p: u8| {
        let raw = |c: Coordinate| {
            render::angle_pair(c, kind, p, symbols, hemisphere, point).expect("renders")
        };
        let first = raw(wgs(lat, lon));
        let parsed = reparse(&first, FormatOptions::default());
        (first, raw(parsed))
    };
    let (dms_lat, dms_lon) = (-29.716_708_966_973_357, -160.137_993_148_233_65);
    let (ddm_lat, ddm_lon) = (-8.954_557_882_120_639, 83.849_377_444_689_25);

    // DMS at 10 second decimals drifts.
    let (first, second) = raw_trip(dms_lat, dms_lon, AngleKind::Dms, DMS_MAX + 1);
    assert_eq!(first, "29°43′00.1522811041″S 160°08′16.7753336411″W");
    assert_eq!(second, "29°43′00.1522811041″S 160°08′16.7753336410″W");
    // DDM at 13 minute decimals drifts.
    let (first, second) = raw_trip(ddm_lat, ddm_lon, AngleKind::Ddm, DDM_MAX + 1);
    assert_eq!(first, "8°57.2734729272384′S 83°50.9626466813552′E");
    assert_eq!(second, "8°57.2734729272385′S 83°50.9626466813552′E");

    // `format` refuses those counts.
    let dms_at = |p| opts(dms(Some(p), symbols, hemisphere));
    let ddm_at = |p| opts(ddm(Some(p), symbols, hemisphere));
    for (options, expected) in [
        (dms_at(DMS_MAX + 1), "second_decimals"),
        (ddm_at(DDM_MAX + 1), "minute_decimals"),
    ] {
        let refused = format(wgs(dms_lat, dms_lon), options);
        assert!(
            matches!(refused, Err(Error::InvalidValue { field, .. }) if field == expected),
            "{options:?}: {refused:?}"
        );
    }

    // The same points are stable at the caps.
    let (first, second) = trip(dms_lat, dms_lon, dms_at(DMS_MAX));
    assert_eq!(first, second);
    let (first, second) = trip(ddm_lat, ddm_lon, ddm_at(DDM_MAX));
    assert_eq!(first, second);
}

/// A decimal count past its cap is `InvalidValue` naming the count's own
/// field, through [`format`] and through [`format_fix`] (whose accuracy
/// derives a count only when none is set).
#[test]
fn format_names_the_decimal_count_it_refuses() {
    let (s, u) = (HemisphereStyle::Signed, SymbolStyle::Unicode);
    let c = wgs(40.7128, -74.006);
    let fix = fix_with_accuracy(c, Some(1.0));
    for (representation, expected) in [
        (dd(Some(DD_MAX + 1), s), "decimals"),
        (dms(Some(DMS_MAX + 1), u, s), "second_decimals"),
        (ddm(Some(DDM_MAX + 1), u, s), "minute_decimals"),
    ] {
        let options = opts(representation);
        for refused in [format(c, options), format_fix(&fix, options)] {
            assert!(
                matches!(&refused, Err(Error::InvalidValue { field, .. }) if *field == expected),
                "{representation:?}: {refused:?}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Away from the rounding-to-zero edge, poles and antimeridian included:
    /// every component at least one unit of the coarsest rendered precision
    /// from zero.
    #[test]
    fn format_parse_format_is_stable_away_from_zero(
        lat in prop_oneof![1.0..=90.0_f64, -90.0..=-1.0_f64, Just(90.0), Just(-90.0)],
        lon in prop_oneof![1.0..=180.0_f64, -180.0..=-1.0_f64, Just(180.0), Just(-180.0)],
        options in options(),
    ) {
        let (first, second) = trip(lat, lon, options);
        prop_assert_eq!(first, second, "options {:?}", options);
    }
}
