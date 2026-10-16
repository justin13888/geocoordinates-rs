use super::normalize::{is_isolated, rewrite_decimal_comma, rewrite_unit_letters, split_two};
use super::*;
use crate::test_support::assert_close;

use crate::fix::AxisOrder;
use crate::parse::GridKind;

fn source(fix: &Fix) -> &RawSource {
    fix.source.as_ref().expect("text parse records a source")
}

fn comma_options() -> ParseOptions {
    ParseOptions {
        decimal_separator: DecimalSeparator::Comma,
        ..ParseOptions::default()
    }
}

#[test]
fn parse_text_with_rejects_every_expected_grid_instead_of_ignoring_it() {
    for kind in [
        GridKind::PlusCode,
        GridKind::Geohash,
        GridKind::Maidenhead,
        GridKind::Mgrs,
    ] {
        let options = ParseOptions {
            expect_grid: Some(kind),
            ..ParseOptions::default()
        };
        // Valid free text is still refused: the option cannot be honoured.
        assert!(matches!(
            parse_text_with("40.7128, -74.006", &options),
            Err(Error::InvalidValue {
                field: "expect_grid",
                ..
            })
        ));
        // The internal free-text reader used by `parse_coordinate_with` leaves
        // the option to its caller.
        assert!(parse_free_text("40.7128, -74.006", &options).is_ok());
    }
}

#[test]
fn dd_comma_defaults_to_latlon() {
    let fix = parse_text("40.7128, -74.006").unwrap();
    assert_close(fix.coord.lat(), 40.7128, 1e-9);
    assert_close(fix.coord.lon(), -74.006, 1e-9);
    let s = source(&fix);
    // Both magnitudes are in latitude range, so the order is assumed.
    assert_close(s.confidence.value(), 0.7, 1e-12);
    assert_eq!(s.axis_order, Some(AxisOrder::LatLon));
}

#[test]
fn range_rule_forces_longitude() {
    // 151.2 exceeds the latitude range, so it must be longitude (Sydney).
    let fix = parse_text("151.2, -33.9").unwrap();
    assert_close(fix.coord.lat(), -33.9, 1e-9);
    assert_close(fix.coord.lon(), 151.2, 1e-9);
    let s = source(&fix);
    assert_close(s.confidence.value(), 0.9, 1e-12);
    assert_eq!(s.axis_order, Some(AxisOrder::LonLat));
}

#[test]
fn dms_with_both_hemispheres() {
    let fix = parse_text("40°42′46″N 74°00′22″W").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 42.0 / 60.0 + 46.0 / 3600.0, 1e-9);
    assert_close(fix.coord.lon(), -(74.0 + 22.0 / 3600.0), 1e-9);
    let s = source(&fix);
    assert_close(s.confidence.value(), 1.0, 1e-12);
    assert_eq!(s.axis_order, None);
}

#[test]
fn hemispheres_can_be_swapped() {
    let fix = parse_text("74°00′22″W 40°42′46″N").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 42.0 / 60.0 + 46.0 / 3600.0, 1e-9);
    assert_close(fix.coord.lon(), -(74.0 + 22.0 / 3600.0), 1e-9);
    assert_close(source(&fix).confidence.value(), 1.0, 1e-12);
}

#[test]
fn ascii_symbols_and_prefix_hemispheres() {
    // ASCII quotes plus leading hemisphere letters.
    let fix = parse_text("N40 42'46\" W74 00'22\"").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 42.0 / 60.0 + 46.0 / 3600.0, 1e-9);
    assert_close(fix.coord.lon(), -(74.0 + 22.0 / 3600.0), 1e-9);
}

#[test]
fn nmea_concatenated() {
    let fix = parse_text("4042.766N 07400.456W").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 42.766 / 60.0, 1e-9);
    assert_close(fix.coord.lon(), -(74.0 + 0.456 / 60.0), 1e-9);
    assert_eq!(source(&fix).axis_order, None);
}

#[test]
fn space_separated_ddm_with_suffix_hemispheres() {
    let fix = parse_text("40 42.766 N 74 0.456 W").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 42.766 / 60.0, 1e-9);
    assert_close(fix.coord.lon(), -(74.0 + 0.456 / 60.0), 1e-9);
}

#[test]
fn decimal_comma_option() {
    let opts = comma_options();
    let fix = parse_text_with("40,7128 -74,006", &opts).unwrap();
    assert_close(fix.coord.lat(), 40.7128, 1e-9);
    assert_close(fix.coord.lon(), -74.006, 1e-9);
    // The same string without the option is a different (failing) parse.
    assert!(parse_text("40,7128 -74,006").is_err());
}

#[test]
fn default_lonlat_swaps_assignment() {
    let opts = ParseOptions {
        default_axis_order: AxisOrder::LonLat,
        ..ParseOptions::default()
    };
    let fix = parse_text_with("-74.006, 40.7128", &opts).unwrap();
    assert_close(fix.coord.lon(), -74.006, 1e-9);
    assert_close(fix.coord.lat(), 40.7128, 1e-9);
    assert_eq!(source(&fix).axis_order, Some(AxisOrder::LonLat));
}

#[test]
fn invalid_inputs_error() {
    assert!(parse_text("200, 0").is_err()); // forced longitude 200 > 180
    assert!(parse_text("100, 150").is_err()); // both exceed the latitude range
    assert!(parse_text("hello world").is_err()); // not numeric
    assert!(parse_text("42").is_err()); // only one component
    assert!(parse_text("").is_err()); // empty
    assert!(parse_text("40 60 N 74 0 W").is_err()); // invalid minutes
    assert!(parse_text("40 0 60 N 74 0 0 W").is_err()); // invalid seconds
    assert!(parse_text("4060.0N 07400.0W").is_err()); // invalid NMEA minutes
    assert!(parse_text("-40N 74W").is_err()); // sign contradicts hemisphere
    assert!(parse_text("NaN, 0").is_err());
}

#[test]
fn doubled_sign_leaves_a_negative_magnitude_that_is_rejected() {
    // The component parser strips one leading `-`; a second one reaches the
    // magnitude parser as a negative number, which must be rejected rather
    // than folded into the sign (`--5` is not `5`).
    assert!(parse_text("--5, 10").is_err()); // decimal magnitude
    assert!(parse_text("--5 30, 10").is_err()); // DDM degrees field
    assert!(parse_text("--5 30 0, 10").is_err()); // DMS degrees field
}

#[test]
fn south_and_east_hemispheres() {
    let fix = parse_text("33.9S 151.2E").unwrap();
    assert_close(fix.coord.lat(), -33.9, 1e-9);
    assert_close(fix.coord.lon(), 151.2, 1e-9);
}

#[test]
fn one_hemisphere_letter_resolves_the_pair() {
    // (lat-letter, none): N pins the first component as latitude.
    let f = parse_text("40.7N 74.006").unwrap();
    assert_close(f.coord.lat(), 40.7, 1e-9);
    assert_close(f.coord.lon(), 74.006, 1e-9);
    let s = source(&f);
    assert_close(s.confidence.value(), 0.9, 1e-12);
    assert_eq!(s.axis_order, None);

    // (lon-letter, none): W pins the first as longitude → swap.
    let f = parse_text("74.006W 40.7").unwrap();
    assert_close(f.coord.lat(), 40.7, 1e-9);
    assert_close(f.coord.lon(), -74.006, 1e-9);

    // (none, lon-letter): W pins the second as longitude. The lat/lon
    // happen to coincide with the assumed-default path, so assert the
    // confidence and axis-order too (the letter resolves it, 0.9 / None).
    let f = parse_text("40.7 74.006W").unwrap();
    assert_close(f.coord.lat(), 40.7, 1e-9);
    assert_close(f.coord.lon(), -74.006, 1e-9);
    let s = source(&f);
    assert_close(s.confidence.value(), 0.9, 1e-12);
    assert_eq!(s.axis_order, None);

    // (none, lat-letter): N pins the second as latitude → swap.
    let f = parse_text("74.006 40.7N").unwrap();
    assert_close(f.coord.lat(), 40.7, 1e-9);
    assert_close(f.coord.lon(), 74.006, 1e-9);
}

#[test]
fn range_rule_boundary_excludes_exactly_90() {
    // Exactly 90 is a valid latitude, so the order stays *assumed* (0.7),
    // not forced by the range rule.
    let f = parse_text("90, 50").unwrap();
    assert_close(f.coord.lat(), 90.0, 1e-9);
    assert_close(f.coord.lon(), 50.0, 1e-9);
    assert_close(source(&f).confidence.value(), 0.7, 1e-12);

    let f = parse_text("50, 90").unwrap();
    assert_close(f.coord.lat(), 50.0, 1e-9);
    assert_close(f.coord.lon(), 90.0, 1e-9);
    assert_close(source(&f).confidence.value(), 0.7, 1e-12);
}

#[test]
fn rewrite_decimal_comma_only_between_digits() {
    assert_eq!(rewrite_decimal_comma("40,7128"), "40.7128");
    assert_eq!(rewrite_decimal_comma("a,5"), "a,5"); // previous char not a digit
    assert_eq!(rewrite_decimal_comma("5,a"), "5,a"); // next char not a digit
    assert_eq!(rewrite_decimal_comma(",5"), ",5"); // leading comma (no previous)
    assert_eq!(rewrite_decimal_comma("5,"), "5,"); // trailing comma (no next)
}

#[test]
fn rewrite_unit_letters_is_anchored() {
    // DMS and DDM markers; degree and minute markers in either case.
    assert_eq!(rewrite_unit_letters("10d30m00.00sN"), "10°30'00.00\"N");
    assert_eq!(rewrite_unit_letters("10D30M00s"), "10°30'00\"");
    // An uppercase `S` after a seconds number is the South hemisphere, not
    // the seconds marker.
    assert_eq!(rewrite_unit_letters("10D30M00S"), "10°30'00S");
    assert_eq!(rewrite_unit_letters("10d30m15S"), "10°30'15S");
    assert_eq!(rewrite_unit_letters("-20d15.000m"), "-20°15.000'");
    // Both components of a pair are rewritten.
    assert_eq!(
        rewrite_unit_letters("10d30m00sS 20d15m00sW"),
        "10°30'00\"S 20°15'00\"W"
    );
    // An `S` with no seconds number before it is a hemisphere, not a marker.
    assert_eq!(rewrite_unit_letters("10d30.000mS"), "10°30.000'S");
    assert_eq!(rewrite_unit_letters("10d30mS"), "10°30'S");
    // Nor is a lowercase `s` with no seconds number before it a marker.
    assert_eq!(rewrite_unit_letters("10d30ms"), "10°30's");
    assert_eq!(rewrite_unit_letters("10d30.000ms"), "10°30.000's");
    // A degree `d` needs a digit on both sides.
    assert_eq!(rewrite_unit_letters("d5"), "d5");
    assert_eq!(rewrite_unit_letters("40dN"), "40dN");
    assert_eq!(rewrite_unit_letters("40 d30m"), "40 d30m");
    // Without a degree marker, `m` and `s` are left alone (`33.9s` is South).
    assert_eq!(rewrite_unit_letters("33.9s 151.2e"), "33.9s 151.2e");
    // A minutes number not followed by `m` stops the sequence there.
    assert_eq!(rewrite_unit_letters("10d30 00s"), "10°30 00s");
}

#[test]
fn letter_markers_parse_like_symbols() {
    let lat = 10.0 + 30.0 / 60.0;
    let lon = -(20.0 + 15.0 / 60.0);
    for input in [
        "10d30m00.00sN 20d15m00.00sW",
        "10d30m00s 20d15m00sW",
        "10d30.000mN 20d15.000mW",
        "10d30.000m -20d15.000m",
    ] {
        let fix = parse_text(input).unwrap_or_else(|e| panic!("{input}: {e}"));
        assert_close(fix.coord.lat(), lat, 1e-9);
        assert_close(fix.coord.lon(), lon, 1e-9);
    }
    // Letter markers combine with the decimal-comma option.
    let opts = comma_options();
    let fix = parse_text_with("10d30m00,00sS 20d15,000mW", &opts).unwrap();
    assert_close(fix.coord.lat(), -lat, 1e-9);
    assert_close(fix.coord.lon(), lon, 1e-9);
}

#[test]
fn uppercase_s_after_seconds_is_south_not_the_seconds_marker() {
    let lat = 10.0 + 30.0 / 60.0 + 15.0 / 3600.0;
    let lon = 20.0 + 10.0 / 60.0 + 5.0 / 3600.0;
    // Uppercase `S` is the hemisphere: the pair is fully lettered, so the
    // result is South with explicit confidence, never a silent North.
    let fix = crate::parse::parse_coordinate("10d30m15S, 20d10m5E").unwrap();
    assert_close(fix.coord.lat(), -lat, 1e-9);
    assert_close(fix.coord.lon(), lon, 1e-9);
    assert_close(source(&fix).confidence.value(), 1.0, 1e-12);
    // Lowercase `s` is the seconds marker; the hemisphere follows it.
    let fix = crate::parse::parse_coordinate("10d30m15sS, 20d10m5sE").unwrap();
    assert_close(fix.coord.lat(), -lat, 1e-9);
    assert_close(fix.coord.lon(), lon, 1e-9);
    // Lowercase `s` alone is the unit, so the latitude carries no hemisphere
    // and the `E` on the other component resolves it as the positive axis.
    let fix = crate::parse::parse_coordinate("10d30m15s, 20d10m5sE").unwrap();
    assert_close(fix.coord.lat(), lat, 1e-9);
    assert_close(fix.coord.lon(), lon, 1e-9);
}

#[test]
fn split_two_rejects_half_empty_comma_split() {
    // "5," splits to ("5", "") on the comma; the empty half must not be
    // returned as a component pair.
    assert!(split_two("5,", false).is_none());
    // `;` separates only decimal-comma text, whose decimals are already `.`.
    assert_eq!(
        split_two("48.8566; 2.3522", true),
        Some(("48.8566".to_string(), "2.3522".to_string()))
    );
    assert!(split_two("48.8566;2.3522", false).is_none());
    assert_eq!(
        split_two("40, -74", false),
        Some(("40".to_string(), "-74".to_string()))
    );
}

#[test]
fn is_isolated_excludes_word_letters() {
    let suffixed: Vec<char> = "46\"N".chars().collect();
    assert!(is_isolated(&suffixed, 3)); // N after a quote
    let word: Vec<char> = "North".chars().collect();
    assert!(!is_isolated(&word, 0)); // N before 'o'
    let flanked: Vec<char> = "aNa".chars().collect();
    assert!(!is_isolated(&flanked, 1)); // N between letters
}

#[test]
fn concatenated_ddmm_needs_a_hemisphere_letter() {
    // C12: four or more integer digits are DDMM only beside a hemisphere
    // letter; bare, `1234.5` is no degree value at all, not 12°34.5′.
    assert!(parse_text("1234.5, 10").is_err());
    assert!(parse_text("10, 01234.5").is_err());
    let fix = parse_text("1234.5N, 10E").unwrap();
    assert_close(fix.coord.lat(), 12.0 + 34.5 / 60.0, 1e-9);
}

#[test]
fn a_fractional_field_must_be_the_last_one() {
    // C12: `40.5 30` is not 40.5° plus 30′; fractional degrees with minutes,
    // or fractional minutes with seconds, are rejected rather than summed.
    assert!(parse_text("40.5 30, 10").is_err());
    assert!(parse_text("40.5 30 0, 10").is_err());
    assert!(parse_text("40 30.5 10, 74").is_err());
    assert!(parse_text("40.5°30′N 74°W").is_err());
    // The last field may carry the fraction.
    let fix = parse_text("40 30.5, 10").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 30.5 / 60.0, 1e-9);
    let fix = parse_text("40 30 10.5, 74").unwrap();
    assert_close(fix.coord.lat(), 40.0 + 30.0 / 60.0 + 10.5 / 3600.0, 1e-9);
}

#[test]
fn exponent_notation_is_rejected() {
    // C12: the `e` of `4e1` is neither East nor an exponent a coordinate is
    // written with.
    for input in ["4e1 3e1", "4E1 3E1", "4e1, 3e1", "40, 7e1", "4e1N 3e1W"] {
        let r = parse_text(input);
        assert!(r.is_err(), "{input:?}: {r:?}");
    }
}

#[test]
fn decimal_comma_text_splits_on_the_remaining_list_separators() {
    // C12: with a decimal comma, the commas left after the decimal rewrite
    // (and `;`) separate the two components.
    let opts = comma_options();
    for input in ["48,8566, 2,3522", "48,8566 , 2,3522", "48,8566; 2,3522"] {
        let fix = parse_text_with(input, &opts).unwrap_or_else(|e| panic!("{input:?}: {e}"));
        assert_close(fix.coord.lat(), 48.8566, 1e-9);
        assert_close(fix.coord.lon(), 2.3522, 1e-9);
    }
}
