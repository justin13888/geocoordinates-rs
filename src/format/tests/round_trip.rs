//! `format(parse(format(c))) == format(c)`: rendering what was parsed back
//! from a rendering reproduces it exactly, for every representation, symbol
//! style, hemisphere style, precision and separator locale.

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

fn options() -> impl Strategy<Value = FormatOptions> {
    (
        prop::sample::select(vec![
            Representation::DecimalDegrees,
            Representation::Dms,
            Representation::Ddm,
            Representation::PlusCode,
        ]),
        prop::option::of(0..=9_u8),
        prop::sample::select(vec![
            SymbolStyle::Unicode,
            SymbolStyle::Ascii,
            SymbolStyle::Letters,
        ]),
        prop::sample::select(vec![HemisphereStyle::Signed, HemisphereStyle::Cardinal]),
        prop::sample::select(vec![None, Some("de-DE")]),
    )
        .prop_map(|(representation, precision, symbol, hemisphere, locale)| {
            opts(representation, precision, symbol, hemisphere, locale)
        })
}

/// One trip: render, parse back with the separator the options imply, render
/// again. Returns both renderings.
fn trip(lat: f64, lon: f64, options: &FormatOptions) -> (String, String) {
    let first = fmt(&wgs(lat, lon), options);
    let parse_options = TextParseOptions {
        decimal_comma: options.uses_decimal_comma(),
        ..TextParseOptions::default()
    };
    let parsed = parse_coordinate_with(&first, &parse_options)
        .unwrap_or_else(|e| panic!("{first:?} did not re-parse: {e}"));
    (first, fmt(&parsed.coord, options))
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
        prop_assert_eq!(first, second, "options {:?}", options);
    }
}

/// The C11 counterexamples the property found before the sign was chosen
/// after rounding, pinned so the property is not the only record of them.
#[test]
fn rounding_to_zero_renders_unsigned() {
    let style = |representation, hemisphere| {
        opts(
            representation,
            Some(2),
            SymbolStyle::Unicode,
            hemisphere,
            None,
        )
    };
    // DD drops the sign of a value that rounds to zero.
    let (first, second) = trip(
        -1e-9,
        10.0,
        &style(Representation::DecimalDegrees, HemisphereStyle::Signed),
    );
    assert_eq!(first, "0.00, 10.00");
    assert_eq!(second, "0.00, 10.00");
    // So does DMS, so the re-parsed 0.0 renders identically.
    let (first, second) = trip(
        -1e-9,
        10.0,
        &style(Representation::Dms, HemisphereStyle::Signed),
    );
    assert_eq!(first, "0°00′00.00″ 10°00′00.00″");
    assert_eq!(second, "0°00′00.00″ 10°00′00.00″");
    // The cardinal style picks the hemisphere after rounding.
    let (first, second) = trip(
        -1e-9,
        10.0,
        &style(Representation::DecimalDegrees, HemisphereStyle::Cardinal),
    );
    assert_eq!(first, "0.00 N, 10.00 E");
    assert_eq!(second, "0.00 N, 10.00 E");
    // A negative zero is unsigned too.
    let (first, _) = trip(
        -0.0,
        -0.0,
        &style(Representation::DecimalDegrees, HemisphereStyle::Signed),
    );
    assert_eq!(first, "0.00, 0.00");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Away from the rounding-to-zero edge the round trip holds today, poles
    /// and antimeridian included: every component at least one unit of the
    /// coarsest rendered precision from zero.
    #[test]
    fn format_parse_format_is_stable_away_from_zero(
        lat in prop_oneof![1.0..=90.0_f64, -90.0..=-1.0_f64, Just(90.0), Just(-90.0)],
        lon in prop_oneof![1.0..=180.0_f64, -180.0..=-1.0_f64, Just(180.0), Just(-180.0)],
        options in options(),
    ) {
        let (first, second) = trip(lat, lon, &options);
        prop_assert_eq!(first, second, "options {:?}", options);
    }
}
