use super::*;
use crate::Coordinate;
use crate::fix::{Accuracy, Fix};

/// Property tests of the `format → parse → format` round trip.
mod round_trip;

fn opts(
    representation: Representation,
    precision: Option<u8>,
    symbol_style: SymbolStyle,
    hemisphere_style: HemisphereStyle,
    locale: Option<&str>,
) -> FormatOptions {
    FormatOptions {
        representation,
        precision,
        symbol_style,
        hemisphere_style,
        locale: locale.map(String::from),
    }
}

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn fmt(coord: &Coordinate, options: &FormatOptions) -> String {
    format(coord, options).expect("DD/DMS/DDM formatting is infallible")
}

#[test]
fn dd_default_matches_doc() {
    let c = wgs(40.7128, -74.006);
    assert_eq!(fmt(&c, &FormatOptions::default()), "40.712800, -74.006000");
}

#[test]
fn dd_cardinal_and_precision() {
    let c = wgs(40.7128, -74.006);
    assert_eq!(
        fmt(
            &c,
            &opts(
                Representation::DecimalDegrees,
                Some(6),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal,
                None,
            ),
        ),
        "40.712800 N, 74.006000 W"
    );
    // Precision rounds.
    let dd = |p| {
        fmt(
            &c,
            &opts(
                Representation::DecimalDegrees,
                Some(p),
                SymbolStyle::Unicode,
                HemisphereStyle::Signed,
                None,
            ),
        )
    };
    assert_eq!(dd(0), "41, -74");
    assert_eq!(dd(2), "40.71, -74.01");
}

#[test]
fn dms_clean_and_styles() {
    let c = wgs(10.5, 20.25);
    let dms = |sym, hemi| fmt(&c, &opts(Representation::Dms, Some(0), sym, hemi, None));
    assert_eq!(
        dms(SymbolStyle::Unicode, HemisphereStyle::Cardinal),
        "10°30′00″N 20°15′00″E"
    );
    assert_eq!(
        dms(SymbolStyle::Ascii, HemisphereStyle::Cardinal),
        "10°30'00\"N 20°15'00\"E"
    );
    assert_eq!(
        dms(SymbolStyle::Letters, HemisphereStyle::Cardinal),
        "10d30m00sN 20d15m00sE"
    );
}

#[test]
fn dms_signed_puts_minus_on_degrees() {
    let c = wgs(10.5, -20.25);
    assert_eq!(
        fmt(
            &c,
            &opts(
                Representation::Dms,
                Some(0),
                SymbolStyle::Unicode,
                HemisphereStyle::Signed,
                None,
            ),
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
            &opts(
                Representation::Dms,
                Some(2),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal,
                None,
            ),
        ),
        "1°00′00.00″N 0°00′00.00″E"
    );
}

#[test]
fn ddm_clean_and_doc_example() {
    let c = wgs(40.5, -74.25);
    assert_eq!(
        fmt(
            &c,
            &opts(
                Representation::Ddm,
                Some(3),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal,
                None,
            ),
        ),
        "40°30.000′N 74°15.000′W"
    );
    // The module-doc example: 40°42.766′N.
    let doc = wgs(40.0 + 42.766 / 60.0, 0.0);
    let out = fmt(
        &doc,
        &opts(
            Representation::Ddm,
            Some(3),
            SymbolStyle::Unicode,
            HemisphereStyle::Cardinal,
            None,
        ),
    );
    assert!(out.starts_with("40°42.766′N"), "{out}");
}

#[test]
fn locale_decimal_comma() {
    let c = wgs(40.7128, -74.006);
    // Comma locale: comma decimals, whitespace list separator (round-trips
    // through `parse_coordinate_with` with `decimal_comma` set).
    assert_eq!(
        fmt(
            &c,
            &opts(
                Representation::DecimalDegrees,
                Some(6),
                SymbolStyle::Unicode,
                HemisphereStyle::Signed,
                Some("de-DE"),
            ),
        ),
        "40,712800 -74,006000"
    );
    // Dot locale is the default behaviour.
    assert_eq!(
        fmt(
            &c,
            &opts(
                Representation::DecimalDegrees,
                Some(6),
                SymbolStyle::Unicode,
                HemisphereStyle::Signed,
                Some("en-US"),
            ),
        ),
        "40.712800, -74.006000"
    );
}

#[test]
fn uses_decimal_comma_follows_locale() {
    let with_locale = |locale: Option<&str>| FormatOptions {
        locale: locale.map(String::from),
        ..FormatOptions::default()
    };
    assert!(!with_locale(None).uses_decimal_comma());
    assert!(!with_locale(Some("en-US")).uses_decimal_comma());
    assert!(with_locale(Some("de-DE")).uses_decimal_comma());
    assert!(with_locale(Some("fr_CA")).uses_decimal_comma());
    assert!(with_locale(Some("PT")).uses_decimal_comma());
}

#[test]
fn format_fix_derives_precision_from_accuracy() {
    let coord = wgs(40.7128, -74.006);
    let fix = |horizontal_m: Option<f64>| Fix {
        coord,
        accuracy: Some(Accuracy {
            horizontal_m,
            vertical_m: None,
        }),
        timestamp: None,
        source: None,
    };
    let dd_default = opts(
        Representation::DecimalDegrees,
        None,
        SymbolStyle::Unicode,
        HemisphereStyle::Signed,
        None,
    );
    // ~1 km accuracy -> 2 decimals; ~1 m -> 5 decimals.
    assert_eq!(
        format_fix(&fix(Some(1000.0)), &dd_default).unwrap(),
        "40.71, -74.01"
    );
    assert_eq!(
        format_fix(&fix(Some(1.0)), &dd_default).unwrap(),
        "40.71280, -74.00600"
    );
    // Unusable accuracy (none / zero / negative) falls back to the
    // representation default (6).
    assert_eq!(
        format_fix(&fix(None), &dd_default).unwrap(),
        "40.712800, -74.006000"
    );
    assert_eq!(
        format_fix(&fix(Some(0.0)), &dd_default).unwrap(),
        "40.712800, -74.006000"
    );
    assert_eq!(
        format_fix(&fix(Some(-5.0)), &dd_default).unwrap(),
        "40.712800, -74.006000"
    );
}

#[test]
fn format_fix_derives_dms_ddm_precision_in_their_own_units() {
    let coord = wgs(40.7128, -74.006);
    let fix = |horizontal_m: Option<f64>| Fix {
        coord,
        accuracy: Some(Accuracy {
            horizontal_m,
            vertical_m: None,
        }),
        timestamp: None,
        source: None,
    };
    let derived = |representation| {
        opts(
            representation,
            None,
            SymbolStyle::Unicode,
            HemisphereStyle::Cardinal,
            None,
        )
    };
    let dms = derived(Representation::Dms);
    let ddm = derived(Representation::Ddm);
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
        format(&coord, &dms).unwrap()
    );
    // Plus Code has no precision knob; accuracy leaves it unchanged.
    let plus = derived(Representation::PlusCode);
    assert_eq!(
        format_fix(&fix(Some(1.0)), &plus).unwrap(),
        format(&coord, &plus).unwrap()
    );
}

#[test]
fn precision_for_accuracy_uses_the_representations_unit() {
    use Representation::{Ddm, DecimalDegrees, Dms, PlusCode};
    assert_eq!(precision_for_accuracy(1.0, DecimalDegrees), Some(5));
    assert_eq!(precision_for_accuracy(1.0, Dms), Some(1));
    assert_eq!(precision_for_accuracy(1.0, Ddm), Some(3));
    assert_eq!(precision_for_accuracy(0.03, Dms), Some(3));
    assert_eq!(precision_for_accuracy(1.0e-9, Dms), Some(8));
    assert_eq!(precision_for_accuracy(1.0e6, Ddm), Some(0));
    assert_eq!(precision_for_accuracy(1.0, PlusCode), None);
    assert_eq!(precision_for_accuracy(f64::NAN, Dms), None);
    assert_eq!(precision_for_accuracy(0.0, Ddm), None);
}

#[test]
fn ddm_minutes_carry_rolls_up() {
    // Minutes round to 60.000 at p=3 and must carry into the degree.
    let c = wgs(0.9999999, 0.0);
    assert_eq!(
        fmt(
            &c,
            &opts(
                Representation::Ddm,
                Some(3),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal,
                None,
            ),
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
            &opts(
                Representation::Dms,
                Some(0),
                SymbolStyle::Unicode,
                HemisphereStyle::Signed,
                None,
            ),
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
            &opts(
                Representation::Dms,
                Some(0),
                SymbolStyle::Unicode,
                HemisphereStyle::Cardinal,
                None,
            ),
        ),
        "0°00′00″N 0°00′00″E"
    );
}

#[test]
fn plus_code_representation() {
    let c = wgs(47.0000625, 8.0000625);
    assert_eq!(
        fmt(
            &c,
            &opts(
                Representation::PlusCode,
                None,
                SymbolStyle::Unicode,
                HemisphereStyle::Signed,
                None,
            ),
        ),
        "8FVC2222+22"
    );
}
