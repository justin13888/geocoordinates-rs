//! `format(parse(format(c))) == format(c)`: rendering what was parsed back
//! from a rendering reproduces it exactly, for every representation, each of
//! its settings the module docs promise is stable, and both decimal
//! separators.

use proptest::prelude::*;

use super::*;
use crate::parse::parse_coordinate_with;
use crate::parse::text::TextParseOptions;

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

/// The highest DMS second decimal count the module docs promise is stable.
const DMS_STABLE_DECIMALS: u8 = 9;
/// The highest DDM minute decimal count the module docs promise is stable.
const DDM_STABLE_DECIMALS: u8 = 12;

/// Every representation, each with every value of its own settings that the
/// module docs promise is stable: DD up to [`MAX_DECIMALS`], DMS and DDM up to
/// their limits above. The drift past those limits is pinned in
/// `angle_decimals_past_f64_resolution_drift_by_one_digit`.
fn representation() -> impl Strategy<Value = Representation> {
    let decimals = |max: u8| prop::option::of(0..=max);
    let symbols = || {
        prop::sample::select(vec![
            SymbolStyle::Unicode,
            SymbolStyle::Ascii,
            SymbolStyle::Letters,
        ])
    };
    let hemisphere =
        || prop::sample::select(vec![HemisphereStyle::Signed, HemisphereStyle::Cardinal]);
    prop_oneof![
        (decimals(MAX_DECIMALS), hemisphere())
            .prop_map(|(decimals, hemisphere)| dd(decimals, hemisphere)),
        (decimals(DMS_STABLE_DECIMALS), symbols(), hemisphere()).prop_map(|(p, s, h)| dms(p, s, h)),
        (decimals(DDM_STABLE_DECIMALS), symbols(), hemisphere()).prop_map(|(p, s, h)| ddm(p, s, h)),
        prop::sample::select(vec![2_u8, 4, 6, 8, 10, 11, 12, 13, 14, 15])
            .prop_map(|length| Representation::PlusCode { length }),
        (1..=12_u8).prop_map(|length| Representation::Geohash { length }),
        (1..=3_u8).prop_map(|pairs| Representation::Maidenhead { pairs }),
        prop::sample::select(vec![
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

fn options() -> impl Strategy<Value = FormatOptions> {
    (
        representation(),
        prop::sample::select(vec![DecimalSeparator::Point, DecimalSeparator::Comma]),
    )
        .prop_map(|(representation, decimal_separator)| FormatOptions {
            representation,
            decimal_separator,
        })
}

/// Parse a rendering back to a coordinate: through `parse_coordinate_with`
/// with the separator the options imply, or, for the grids it does not detect
/// yet (Geohash, Maidenhead, MGRS), through the grid's own parser and decoder.
fn reparse(text: &str, options: &FormatOptions) -> Coordinate {
    let decoded = match options.representation {
        Representation::Geohash { .. } => Geohash::try_from(text).map(|g| g.decode()),
        Representation::Maidenhead { .. } => Maidenhead::try_from(text).map(|m| m.decode()),
        Representation::Mgrs { .. } => Mgrs::try_from(text).map(|m| m.to_coordinate()),
        _ => {
            let parse_options = TextParseOptions {
                decimal_comma: options.decimal_separator == DecimalSeparator::Comma,
                ..TextParseOptions::default()
            };
            return parse_coordinate_with(text, &parse_options)
                .unwrap_or_else(|e| panic!("{text:?} did not re-parse: {e}"))
                .coord;
        }
    };
    decoded
        .unwrap_or_else(|e| panic!("{text:?} did not re-parse: {e}"))
        .into_inner()
}

/// One trip: render, parse back, render again. Returns both renderings.
fn trip(lat: f64, lon: f64, options: &FormatOptions) -> (String, String) {
    let first = fmt(&wgs(lat, lon), options);
    let parsed = reparse(&first, options);
    (first, fmt(&parsed, options))
}

/// Whether a re-rendering moved an MGRS reference into another grid-zone
/// designator: the zone number and latitude band of a UTM reference
/// (`18T`), or the single polar letter (`A`, `B`, `Y`, `Z`) of a UPS one,
/// whose next two letters already name the 100 km square.
///
/// KNOWN WRONG (#149): [`Mgrs::to_coordinate`] returns the center of
/// the nominal square even where the grid zone truncates it, and that center
/// can lie in the neighbouring zone or band, so the re-rendering names a
/// different square. The pinned case is
/// `mgrs_truncated_square_center_leaves_its_zone_today`. Every other drift,
/// including a different square inside the same zone or polar area, still
/// fails the property.
fn mgrs_left_its_zone(first: &str, second: &str, options: &FormatOptions) -> bool {
    let gzd = |s: &str| {
        let digits = s.chars().take_while(char::is_ascii_digit).count();
        s.chars().take(digits + 1).collect::<String>()
    };
    matches!(options.representation, Representation::Mgrs { .. }) && gzd(first) != gzd(second)
}

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
        let (first, second) = trip(lat, lon, &options);
        prop_assume!(!mgrs_left_its_zone(&first, &second, &options));
        prop_assert_eq!(first, second, "options {:?}", options);
    }
}

/// The MGRS counterexample excluded from the property above, pinned so the
/// exclusion is visible and flips when the decoder clips truncated squares.
#[test]
fn mgrs_truncated_square_center_leaves_its_zone_today() {
    let km100 = opts(Representation::Mgrs {
        precision: MgrsPrecision::Km100,
    });
    // KNOWN WRONG (#149): zone 1's westernmost 100 km square is cut
    // by the antimeridian; its nominal center lies west of 180°W, in zone 60.
    let (first, second) = trip(0.0, -180.0, &km100);
    assert_eq!(first, "01NAA");
    assert_eq!(second, "60NZF");
    assert!(mgrs_left_its_zone(&first, &second, &km100));
    // A drift inside one zone is not excused.
    assert!(!mgrs_left_its_zone("18TWK", "18TWL", &km100));
    // Nor is a drift between 100 km squares of one polar area: a UPS
    // reference's designator is its first letter alone.
    assert!(!mgrs_left_its_zone("ZAB", "ZAC", &km100));
    assert!(!mgrs_left_its_zone("BCS", "BDS", &km100));
    assert!(mgrs_left_its_zone("ZAB", "YZB", &km100));
    assert!(mgrs_left_its_zone("ZAB", "32XNJ", &km100));
    assert!(!mgrs_left_its_zone(
        "01NAA",
        "60NZF",
        &FormatOptions::default()
    ));
}

/// The C11 counterexamples the property found before the sign was chosen
/// after rounding, pinned so the property is not the only record of them.
#[test]
fn rounding_to_zero_renders_unsigned() {
    // DD drops the sign of a value that rounds to zero.
    let dd_signed = opts(dd(Some(2), HemisphereStyle::Signed));
    let (first, second) = trip(-1e-9, 10.0, &dd_signed);
    assert_eq!(first, "0.00, 10.00");
    assert_eq!(second, "0.00, 10.00");
    // So does DMS, so the re-parsed 0.0 renders identically.
    let dms_signed = opts(dms(Some(2), SymbolStyle::Unicode, HemisphereStyle::Signed));
    let (first, second) = trip(-1e-9, 10.0, &dms_signed);
    assert_eq!(first, "0°00′00.00″ 10°00′00.00″");
    assert_eq!(second, "0°00′00.00″ 10°00′00.00″");
    // The cardinal style picks the hemisphere after rounding.
    let dd_cardinal = opts(dd(Some(2), HemisphereStyle::Cardinal));
    let (first, second) = trip(-1e-9, 10.0, &dd_cardinal);
    assert_eq!(first, "0.00 N, 10.00 E");
    assert_eq!(second, "0.00 N, 10.00 E");
    // A negative zero is unsigned too.
    let (first, _) = trip(-0.0, -0.0, &dd_signed);
    assert_eq!(first, "0.00, 0.00");
}

/// Past the stable limits the module docs state, the last digit lies below
/// what an `f64` keeps through the sexagesimal split and the re-parse, so a
/// round trip can move it by one. Pinned so the narrowed guarantee is
/// visible, and so the same points at the limits stay stable.
#[test]
fn angle_decimals_past_f64_resolution_drift_by_one_digit() {
    let dms_at = |p| {
        opts(dms(
            Some(p),
            SymbolStyle::Unicode,
            HemisphereStyle::Cardinal,
        ))
    };
    let ddm_at = |p| {
        opts(ddm(
            Some(p),
            SymbolStyle::Unicode,
            HemisphereStyle::Cardinal,
        ))
    };
    let (dms_lat, dms_lon) = (-29.716_708_966_973_357, -160.137_993_148_233_65);
    let (ddm_lat, ddm_lon) = (-8.954_557_882_120_639, 83.849_377_444_689_25);

    // KNOWN LIMIT: DMS at 10 second decimals.
    let (first, second) = trip(dms_lat, dms_lon, &dms_at(DMS_STABLE_DECIMALS + 1));
    assert_eq!(first, "29°43′00.1522811041″S 160°08′16.7753336411″W");
    assert_eq!(second, "29°43′00.1522811041″S 160°08′16.7753336410″W");
    // KNOWN LIMIT: DDM at 13 minute decimals.
    let (first, second) = trip(ddm_lat, ddm_lon, &ddm_at(DDM_STABLE_DECIMALS + 1));
    assert_eq!(first, "8°57.2734729272384′S 83°50.9626466813552′E");
    assert_eq!(second, "8°57.2734729272385′S 83°50.9626466813552′E");

    // The same points are stable at the limits.
    let (first, second) = trip(dms_lat, dms_lon, &dms_at(DMS_STABLE_DECIMALS));
    assert_eq!(first, second);
    let (first, second) = trip(ddm_lat, ddm_lon, &ddm_at(DDM_STABLE_DECIMALS));
    assert_eq!(first, second);
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
        let (first, second) = trip(lat, lon, &options);
        prop_assume!(!mgrs_left_its_zone(&first, &second, &options));
        prop_assert_eq!(first, second, "options {:?}", options);
    }
}
