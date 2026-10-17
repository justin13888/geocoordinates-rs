use super::*;
use crate::fix::{Accuracy, AccuracyKind, Fix};
use crate::{Coordinate, Crs};

/// Property tests of the `format → parse → format` round trip.
mod round_trip;

/// Options for `representation` with a point separator.
fn opts(representation: Representation) -> FormatOptions {
    FormatOptions {
        representation,
        decimal_separator: DecimalSeparator::Point,
    }
}

fn dd(decimals: Option<u8>, hemisphere: HemisphereStyle) -> Representation {
    Representation::DecimalDegrees {
        decimals,
        hemisphere,
    }
}

fn dms(
    second_decimals: Option<u8>,
    symbols: SymbolStyle,
    hemisphere: HemisphereStyle,
) -> Representation {
    Representation::Dms {
        second_decimals,
        symbols,
        hemisphere,
    }
}

fn ddm(
    minute_decimals: Option<u8>,
    symbols: SymbolStyle,
    hemisphere: HemisphereStyle,
) -> Representation {
    Representation::Ddm {
        minute_decimals,
        symbols,
        hemisphere,
    }
}

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn fmt(coord: &Coordinate, options: &FormatOptions) -> String {
    format(*coord, options).expect("formatting succeeds")
}

/// A fix whose one-sigma accuracy has this horizontal magnitude; with `None`,
/// the accuracy carries only a vertical magnitude.
fn fix_with_accuracy(coord: Coordinate, horizontal_m: Option<f64>) -> Fix {
    fix_with_kind(coord, horizontal_m, AccuracyKind::OneSigma)
}

fn fix_with_kind(coord: Coordinate, horizontal_m: Option<f64>, kind: AccuracyKind) -> Fix {
    let vertical = horizontal_m
        .is_none()
        .then_some(crate::Length::from_meters(5.0));
    let horizontal = horizontal_m.map(crate::Length::from_meters);
    let mut fix = Fix::from_coord(coord);
    fix.accuracy = Some(Accuracy::new(kind, horizontal, vertical).unwrap());
    fix
}

#[test]
fn dd_default_matches_doc() {
    let c = wgs(40.7128, -74.006);
    assert_eq!(fmt(&c, &FormatOptions::default()), "40.712800, -74.006000");
}

#[test]
fn default_options_leave_decimals_unset() {
    assert_eq!(
        FormatOptions::default(),
        opts(dd(None, HemisphereStyle::Signed))
    );
}

#[test]
fn dd_cardinal_and_precision() {
    let c = wgs(40.7128, -74.006);
    assert_eq!(
        fmt(&c, &opts(dd(Some(6), HemisphereStyle::Cardinal))),
        "40.712800 N, 74.006000 W"
    );
    // Precision rounds.
    let dd = |p| fmt(&c, &opts(dd(Some(p), HemisphereStyle::Signed)));
    assert_eq!(dd(0), "41, -74");
    assert_eq!(dd(2), "40.71, -74.01");
}

#[test]
fn dms_clean_and_styles() {
    let c = wgs(10.5, 20.25);
    let dms = |sym| fmt(&c, &opts(dms(Some(0), sym, HemisphereStyle::Cardinal)));
    assert_eq!(dms(SymbolStyle::Unicode), "10°30′00″N 20°15′00″E");
    assert_eq!(dms(SymbolStyle::Ascii), "10°30'00\"N 20°15'00\"E");
    assert_eq!(dms(SymbolStyle::Letters), "10d30m00sN 20d15m00sE");
}

#[test]
fn dms_signed_puts_minus_on_degrees() {
    let c = wgs(10.5, -20.25);
    assert_eq!(
        fmt(
            &c,
            &opts(dms(Some(0), SymbolStyle::Unicode, HemisphereStyle::Signed))
        ),
        "10°30′00″ -20°15′00″"
    );
}

#[test]
fn dms_seconds_carry_rolls_up() {
    // lat ~0.9999999 -> 0°59′59.99964″, which rounds to 60″ at p=2 and must
    // carry to 1°00′00.00″ rather than render 0°59′60.00″ or 0°60′00″.
    let c = wgs(0.9999999, 0.0);
    assert_eq!(
        fmt(
            &c,
            &opts(dms(
                Some(2),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal
            ))
        ),
        "1°00′00.00″N 0°00′00.00″E"
    );
}

#[test]
fn ddm_clean_and_doc_example() {
    let c = wgs(40.5, -74.25);
    let cardinal = opts(ddm(
        Some(3),
        SymbolStyle::Unicode,
        HemisphereStyle::Cardinal,
    ));
    assert_eq!(fmt(&c, &cardinal), "40°30.000′N 74°15.000′W");
    // The variant-doc example: 40°42.768′N 74°00.360′W.
    let out = fmt(&wgs(40.7128, -74.006), &cardinal);
    assert_eq!(out, "40°42.768′N 74°00.360′W");
}

#[test]
fn comma_separator() {
    let c = wgs(40.7128, -74.006);
    // Comma decimals, whitespace list separator (round-trips through
    // `parse_coordinate_with` given `FormatOptions::parse_options`).
    let comma = FormatOptions {
        representation: dd(Some(6), HemisphereStyle::Signed),
        decimal_separator: DecimalSeparator::Comma,
    };
    assert_eq!(fmt(&c, &comma), "40,712800 -74,006000");
    // So do the minutes and seconds of DMS / DDM.
    let comma_dms = FormatOptions {
        representation: dms(Some(1), SymbolStyle::Unicode, HemisphereStyle::Cardinal),
        ..comma
    };
    assert_eq!(fmt(&c, &comma_dms), "40°42′46,1″N 74°00′21,6″W");
    // A point separator is the default behaviour.
    assert_eq!(
        fmt(&c, &opts(dd(Some(6), HemisphereStyle::Signed))),
        "40.712800, -74.006000"
    );
}

#[test]
fn comma_separator_leaves_grid_output_unchanged() {
    let c = wgs(47.0000625, 8.0000625);
    let representation = Representation::PlusCode { length: 10 };
    let comma = FormatOptions {
        representation,
        decimal_separator: DecimalSeparator::Comma,
    };
    assert_eq!(fmt(&c, &comma), fmt(&c, &opts(representation)));
}

#[test]
fn format_fix_derives_precision_from_accuracy() {
    let coord = wgs(40.7128, -74.006);
    let fix = |horizontal_m| fix_with_accuracy(coord, horizontal_m);
    let dd_default = opts(dd(None, HemisphereStyle::Signed));
    // ~1 km accuracy -> 2 decimals; ~1 m -> 5 decimals.
    assert_eq!(
        format_fix(&fix(Some(1000.0)), &dd_default).unwrap(),
        "40.71, -74.01"
    );
    assert_eq!(
        format_fix(&fix(Some(1.0)), &dd_default).unwrap(),
        "40.71280, -74.00600"
    );
    // An accuracy with no horizontal magnitude, or a zero one, falls back to
    // the representation default (6).
    for unusable in [None, Some(0.0)] {
        assert_eq!(
            format_fix(&fix(unusable), &dd_default).unwrap(),
            "40.712800, -74.006000"
        );
    }
    // A set decimal count wins over the accuracy.
    let three = opts(dd(Some(3), HemisphereStyle::Signed));
    assert_eq!(
        format_fix(&fix(Some(1000.0)), &three).unwrap(),
        "40.713, -74.006"
    );
    // A fix without any accuracy record renders as `format` does.
    let bare = Fix::from_coord(coord);
    assert_eq!(
        format_fix(&bare, &dd_default).unwrap(),
        fmt(&coord, &dd_default)
    );
}

/// Only the magnitude sets the precision: every accuracy kind renders alike.
#[test]
fn format_fix_treats_every_accuracy_kind_alike() {
    let coord = wgs(40.7128, -74.006);
    for kind in [
        AccuracyKind::Bound,
        AccuracyKind::OneSigma,
        AccuracyKind::Cep50,
        AccuracyKind::Unspecified,
    ] {
        let fix = fix_with_kind(coord, Some(1000.0), kind);
        assert_eq!(
            format_fix(&fix, &FormatOptions::default()).unwrap(),
            "40.71, -74.01",
            "{kind:?}"
        );
    }
}

/// C10: the default options leave precision unset, so `format_fix` derives
/// it from the fix's accuracy instead of printing a fixed six decimals.
#[test]
fn format_fix_with_default_options_follows_accuracy() {
    let fix = fix_with_accuracy(wgs(40.7128, -74.006), Some(1000.0));
    assert_eq!(
        format_fix(&fix, &FormatOptions::default()).unwrap(),
        "40.71, -74.01"
    );
    // A bare coordinate still gets the representation's default precision.
    assert_eq!(
        fmt(&fix.coord, &FormatOptions::default()),
        "40.712800, -74.006000"
    );
}

#[test]
fn format_fix_derives_dms_ddm_precision_in_their_own_units() {
    let coord = wgs(40.7128, -74.006);
    let fix = |horizontal_m| fix_with_accuracy(coord, horizontal_m);
    let dms = opts(dms(None, SymbolStyle::Unicode, HemisphereStyle::Cardinal));
    let ddm = opts(ddm(None, SymbolStyle::Unicode, HemisphereStyle::Cardinal));
    // DMS counts decimals of arcseconds (1″ ≈ 30.9 m): ~1 m -> 1 decimal,
    // ~1 km -> whole seconds.
    assert_eq!(
        format_fix(&fix(Some(1.0)), &dms).unwrap(),
        "40°42′46.1″N 74°00′21.6″W"
    );
    assert_eq!(
        format_fix(&fix(Some(1000.0)), &dms).unwrap(),
        "40°42′46″N 74°00′22″W"
    );
    // DDM counts decimals of arcminutes (1′ ≈ 1.85 km): ~1 m -> 3
    // decimals, ~1 km -> whole minutes.
    assert_eq!(
        format_fix(&fix(Some(1.0)), &ddm).unwrap(),
        "40°42.768′N 74°00.360′W"
    );
    assert_eq!(
        format_fix(&fix(Some(1000.0)), &ddm).unwrap(),
        "40°43′N 74°00′W"
    );
    // Unusable accuracy falls back to the representation default.
    assert_eq!(
        format_fix(&fix(None), &dms).unwrap(),
        format(coord, &dms).unwrap()
    );
    // The grid representations have no decimal count; accuracy leaves them
    // unchanged.
    for representation in [
        Representation::PlusCode { length: 10 },
        Representation::Geohash { length: 7 },
        Representation::Maidenhead { pairs: 3 },
        Representation::Mgrs {
            precision: MgrsPrecision::M10,
        },
    ] {
        assert_eq!(
            format_fix(&fix(Some(1.0)), &opts(representation)).unwrap(),
            format(coord, &opts(representation)).unwrap()
        );
    }
}

#[test]
fn precision_for_accuracy_uses_the_representations_unit() {
    let dd = dd(None, HemisphereStyle::Signed);
    let dms = dms(None, SymbolStyle::Unicode, HemisphereStyle::Signed);
    let ddm = ddm(None, SymbolStyle::Unicode, HemisphereStyle::Signed);
    assert_eq!(precision_for_accuracy(1.0, dd), Some(5));
    assert_eq!(precision_for_accuracy(1.0, dms), Some(1));
    assert_eq!(precision_for_accuracy(1.0, ddm), Some(3));
    assert_eq!(precision_for_accuracy(0.03, dms), Some(3));
    assert_eq!(precision_for_accuracy(1.0e-9, dms), Some(8));
    assert_eq!(precision_for_accuracy(1.0e6, ddm), Some(0));
    assert_eq!(
        precision_for_accuracy(1.0, Representation::PlusCode { length: 10 }),
        None
    );
    assert_eq!(
        precision_for_accuracy(1.0, Representation::Geohash { length: 7 }),
        None
    );
    assert_eq!(
        precision_for_accuracy(1.0, Representation::Maidenhead { pairs: 3 }),
        None
    );
    assert_eq!(
        precision_for_accuracy(
            1.0,
            Representation::Mgrs {
                precision: MgrsPrecision::M1
            }
        ),
        None
    );
    assert_eq!(precision_for_accuracy(0.0, ddm), None);
}

#[test]
fn ddm_minutes_carry_rolls_up() {
    // Minutes round to 60.000 at p=3 and must carry into the degree.
    let c = wgs(0.9999999, 0.0);
    assert_eq!(
        fmt(
            &c,
            &opts(ddm(
                Some(3),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal
            ))
        ),
        "1°00.000′N 0°00.000′E"
    );
}

#[test]
fn signed_zero_has_no_minus() {
    // A zero component must not gain a leading '-' in the signed style.
    let c = wgs(0.0, 10.0);
    assert_eq!(
        fmt(
            &c,
            &opts(dms(Some(0), SymbolStyle::Unicode, HemisphereStyle::Signed))
        ),
        "0°00′00″ 10°00′00″"
    );
}

#[test]
fn negative_zero_longitude_is_east() {
    let c = wgs(0.0, -0.0);
    assert_eq!(
        fmt(
            &c,
            &opts(dms(
                Some(0),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal
            ))
        ),
        "0°00′00″N 0°00′00″E"
    );
}

#[test]
fn value_rounding_to_zero_takes_the_positive_hemisphere() {
    // C11: the sign is chosen after rounding, in every angle representation.
    let c = wgs(-1e-9, -1e-9);
    assert_eq!(
        fmt(&c, &opts(dd(Some(3), HemisphereStyle::Cardinal))),
        "0.000 N, 0.000 E"
    );
    assert_eq!(
        fmt(
            &c,
            &opts(ddm(Some(2), SymbolStyle::Ascii, HemisphereStyle::Signed))
        ),
        "0°00.00' 0°00.00'"
    );
    assert_eq!(
        fmt(
            &c,
            &opts(dms(
                Some(1),
                SymbolStyle::Letters,
                HemisphereStyle::Cardinal
            ))
        ),
        "0d00m00.0sN 0d00m00.0sE"
    );
    // A value that rounds away from zero keeps its sign.
    let c = wgs(-0.004, -0.006);
    assert_eq!(
        fmt(&c, &opts(dd(Some(2), HemisphereStyle::Signed))),
        "0.00, -0.01"
    );
}

#[test]
fn plus_code_representation() {
    let c = wgs(47.0000625, 8.0000625);
    let plus = |length| fmt(&c, &opts(Representation::PlusCode { length }));
    assert_eq!(plus(10), "8FVC2222+22");
    assert_eq!(plus(8), "8FVC2222+");
    assert_eq!(plus(11), PlusCode::encode(c, 11).unwrap().to_string());
}

#[test]
fn geohash_maidenhead_and_mgrs_representations() {
    let geohash = fmt(
        &wgs(42.6, -5.6),
        &opts(Representation::Geohash { length: 5 }),
    );
    assert_eq!(geohash, "ezs42");
    let maidenhead = fmt(
        &wgs(40.5, -75.0),
        &opts(Representation::Maidenhead { pairs: 2 }),
    );
    assert_eq!(maidenhead, "FN20");
    let mgrs = |precision| fmt(&wgs(40.0, -75.0), &opts(Representation::Mgrs { precision }));
    assert_eq!(mgrs(MgrsPrecision::M1), "18TWK0000027757");
    assert_eq!(mgrs(MgrsPrecision::Km100), "18TWK");
}

#[test]
fn too_many_decimals_is_an_error_naming_the_field() {
    let c = wgs(40.7128, -74.006);
    let (u, s) = (SymbolStyle::Unicode, HemisphereStyle::Signed);
    let cases = [
        (dd(Some(16), s), "decimals"),
        (dms(Some(10), u, s), "second_decimals"),
        (dms(Some(u8::MAX), u, s), "second_decimals"),
        (ddm(Some(13), u, s), "minute_decimals"),
        (ddm(Some(16), u, s), "minute_decimals"),
    ];
    for (representation, expected) in cases {
        match format(c, &opts(representation)) {
            Err(Error::InvalidValue { field, detail }) => {
                assert_eq!(field, expected);
                assert!(detail.contains("round-trips"), "{detail}");
            }
            other => panic!("expected InvalidValue for {representation:?}, got {other:?}"),
        }
    }
    // Each cap is the limit, and it renders.
    assert_eq!(
        format(c, &opts(dd(Some(15), s))).unwrap(),
        "40.712800000000001, -74.006000000000000"
    );
    assert!(format(c, &opts(dms(Some(9), u, s))).is_ok());
    assert!(format(c, &opts(ddm(Some(12), u, s))).is_ok());
    // `format_fix` applies the same limit to a set count.
    let fix = fix_with_accuracy(c, Some(1.0));
    assert!(format_fix(&fix, &opts(dd(Some(16), HemisphereStyle::Signed))).is_err());
}

#[test]
fn grid_sizes_out_of_range_are_errors() {
    let c = wgs(40.7128, -74.006);
    for representation in [
        Representation::PlusCode { length: 9 },
        Representation::PlusCode { length: 16 },
        Representation::Geohash { length: 0 },
        Representation::Geohash { length: 13 },
        Representation::Maidenhead { pairs: 0 },
        Representation::Maidenhead { pairs: 4 },
    ] {
        assert!(
            matches!(
                format(c, &opts(representation)),
                Err(Error::InvalidValue { .. })
            ),
            "{representation:?}"
        );
    }
}

#[test]
fn grids_require_wgs84_but_angles_render_any_datum() {
    let gcj = Coordinate::gcj02(39.915, 116.404).unwrap();
    let mismatch = Err(Error::CrsMismatch {
        expected: Crs::Wgs84,
        found: Crs::Gcj02,
    });
    for representation in [
        Representation::PlusCode { length: 10 },
        Representation::Geohash { length: 7 },
        Representation::Maidenhead { pairs: 3 },
        Representation::Mgrs {
            precision: MgrsPrecision::M1,
        },
    ] {
        assert_eq!(format(gcj, &opts(representation)), mismatch);
    }
    assert_eq!(
        fmt(&gcj, &FormatOptions::default()),
        "39.915000, 116.404000"
    );
}

#[cfg(feature = "serde")]
#[test]
fn format_options_serde_round_trips() {
    let options = FormatOptions {
        representation: dms(Some(1), SymbolStyle::Ascii, HemisphereStyle::Cardinal),
        decimal_separator: DecimalSeparator::Comma,
    };
    let json = serde_json::to_string(&options).unwrap();
    assert_eq!(
        json,
        r#"{"representation":{"Dms":{"second_decimals":1,"symbols":"Ascii","hemisphere":"Cardinal"}},"decimal_separator":"Comma"}"#
    );
    assert_eq!(
        serde_json::from_str::<FormatOptions>(&json).unwrap(),
        options
    );
    let mgrs = opts(Representation::Mgrs {
        precision: MgrsPrecision::M10,
    });
    let json = serde_json::to_string(&mgrs).unwrap();
    assert_eq!(serde_json::from_str::<FormatOptions>(&json).unwrap(), mgrs);
}

/// A field absent from the serialized form takes its default, so options
/// written before a field existed still deserialize once one is added.
#[cfg(feature = "serde")]
#[test]
fn options_serde_fills_missing_fields_from_default() {
    use crate::fix::AxisOrder;
    use crate::parse::{GridKind, ParseOptions};

    assert_eq!(
        serde_json::from_str::<FormatOptions>("{}").unwrap(),
        FormatOptions::default()
    );
    assert_eq!(
        serde_json::from_str::<FormatOptions>(r#"{"decimal_separator":"Comma"}"#).unwrap(),
        FormatOptions::default().with_decimal_separator(DecimalSeparator::Comma)
    );
    assert_eq!(
        serde_json::from_str::<ParseOptions>("{}").unwrap(),
        ParseOptions::default()
    );
    let parse = ParseOptions::default()
        .with_default_axis_order(AxisOrder::LonLat)
        .with_expect_grid(Some(GridKind::Mgrs));
    assert_eq!(
        serde_json::from_str::<ParseOptions>(
            r#"{"default_axis_order":"LonLat","expect_grid":"Mgrs"}"#
        )
        .unwrap(),
        parse
    );
}

/// The builders set exactly their own field, and the options hash by value.
#[test]
fn builders_set_their_field_and_options_hash_by_value() {
    use std::collections::HashSet;

    use crate::fix::AxisOrder;
    use crate::parse::{GridKind, ParseOptions};

    let geohash = Representation::Geohash { length: 5 };
    let format_options = FormatOptions::default()
        .with_representation(geohash)
        .with_decimal_separator(DecimalSeparator::Comma);
    assert_eq!(
        format_options,
        FormatOptions {
            representation: geohash,
            decimal_separator: DecimalSeparator::Comma,
        }
    );
    let parse_options = ParseOptions::default()
        .with_default_axis_order(AxisOrder::LonLat)
        .with_decimal_separator(DecimalSeparator::Comma)
        .with_expect_grid(Some(GridKind::Geohash));
    assert_eq!(
        parse_options,
        ParseOptions {
            default_axis_order: AxisOrder::LonLat,
            decimal_separator: DecimalSeparator::Comma,
            expect_grid: Some(GridKind::Geohash),
        }
    );
    assert_eq!(parse_options.with_expect_grid(None).expect_grid, None);

    let set: HashSet<FormatOptions> = [format_options, format_options, FormatOptions::default()]
        .into_iter()
        .collect();
    assert_eq!(set.len(), 2);
    let set: HashSet<ParseOptions> = [parse_options, parse_options, ParseOptions::default()]
        .into_iter()
        .collect();
    assert_eq!(set.len(), 2);
}
