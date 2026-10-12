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

    // KNOWN WRONG (C11, flipped by #117): a value that rounds to zero keeps
    // its sign, so DMS prints `-0°00′00.00″` and re-renders the parsed -0.0
    // as `0°00′00.00″`, and the cardinal style picks the hemisphere before
    // rounding (`0.00 S` re-renders as `0.00 N`). The pinned cases are in
    // `rounding_to_zero_flips_the_rendering_today`. #117 activates this
    // property for every representation and separator.
    #[test]
    #[ignore = "C11, fixed by the format slice (#117)"]
    fn format_parse_format_is_stable(
        lat in latitude(),
        lon in longitude(),
        options in options(),
    ) {
        let (first, second) = trip(lat, lon, &options);
        prop_assert_eq!(first, second, "options {:?}", options);
    }
}

/// The C11 counterexamples the property finds today, pinned so the ignored
/// property is not the only record of them.
#[test]
fn rounding_to_zero_flips_the_rendering_today() {
    let style = |representation, hemisphere| {
        opts(
            representation,
            Some(2),
            SymbolStyle::Unicode,
            hemisphere,
            None,
        )
    };
    // KNOWN WRONG (C11, flipped by #117): DD keeps the sign of a value that
    // rounds to zero. It happens to re-render identically, because the parse
    // yields -0.0, which DD also prints with its sign.
    let (first, second) = trip(
        -1e-9,
        10.0,
        &style(Representation::DecimalDegrees, HemisphereStyle::Signed),
    );
    assert_eq!(first, "-0.00, 10.00");
    assert_eq!(second, "-0.00, 10.00");
    // KNOWN WRONG (C11, flipped by #117): DMS keeps that sign too, but
    // renders the re-parsed -0.0 without it, so the round trip drifts.
    let (first, second) = trip(
        -1e-9,
        10.0,
        &style(Representation::Dms, HemisphereStyle::Signed),
    );
    assert_eq!(first, "-0°00′00.00″ 10°00′00.00″");
    assert_eq!(second, "0°00′00.00″ 10°00′00.00″");
    // KNOWN WRONG (C11, flipped by #117): the cardinal style picks the
    // hemisphere before rounding.
    let (first, second) = trip(
        -1e-9,
        10.0,
        &style(Representation::DecimalDegrees, HemisphereStyle::Cardinal),
    );
    assert_eq!(first, "0.00 S, 10.00 E");
    assert_eq!(second, "0.00 N, 10.00 E");
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
