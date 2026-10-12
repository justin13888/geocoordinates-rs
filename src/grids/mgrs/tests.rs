use super::*;
use crate::test_support::{assert_close, assert_within_geodesic_meters};

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

// (lat, lon, MGRS at 1 m) reference strings from the `mgrs` Python library.
const REFS: &[(f64, f64, &str)] = &[
    (40.0, -75.0, "18TWK0000027757"),
    (48.8584, 2.2945, "31UDQ4825211954"),
    (-33.8568, 151.2153, "56HLH3490052288"),
    (60.0, 5.0, "32VKM7697958157"),
    (0.0, 0.0, "31NAA6602100000"),
    (-1.0, -1.0, "30MYD2256189402"),
    (82.0, 20.0, "33XWM7759308183"), // UTM band X, above 80.5° (pins the X decode)
    (85.0, 0.0, "ZAB0000044542"),
    (87.0, 45.0, "ZCE3556864431"),
    (-85.0, 30.0, "BCS7772881040"),
    (89.0, -150.0, "YZH4448696151"),
    (-85.0, -100.0, "ASM5298103545"), // UPS zone A (south-west)
];

#[test]
fn encode_reference_strings() {
    for &(lat, lon, expected) in REFS {
        assert_eq!(
            Mgrs::try_from_coordinate(c(lat, lon), MgrsPrecision::M1)
                .unwrap()
                .as_str(),
            expected,
            "({lat},{lon})"
        );
    }
}

#[test]
fn encode_precision_levels() {
    let m = Mgrs::try_from_coordinate(c(40.0, -75.0), MgrsPrecision::M100).unwrap();
    assert_eq!(m.as_str(), "18TWK000277"); // 3+3 digits
    assert_eq!(m.precision(), MgrsPrecision::M100);
    // A non-zero easting remainder confirms the digits are scaled (divided),
    // not multiplied: 48 252 m → "482" at 100 m precision.
    assert_eq!(
        Mgrs::try_from_coordinate(c(48.8584, 2.2945), MgrsPrecision::M100)
            .unwrap()
            .as_str(),
        "31UDQ482119"
    );
    // Every precision, coarsest to finest, adds one digit to each half.
    let expected = [
        (MgrsPrecision::Km100, "18TWK"),
        (MgrsPrecision::Km10, "18TWK02"),
        (MgrsPrecision::Km1, "18TWK0027"),
        (MgrsPrecision::M100, "18TWK000277"),
        (MgrsPrecision::M10, "18TWK00002775"),
        (MgrsPrecision::M1, "18TWK0000027757"),
    ];
    for (precision, s) in expected {
        let m = Mgrs::try_from_coordinate(c(40.0, -75.0), precision).unwrap();
        assert_eq!(m.as_str(), s);
        assert_eq!(m.precision(), precision);
        assert_eq!(Mgrs::try_from(s).unwrap().precision(), precision);
    }
}

#[test]
fn precision_meters_are_the_square_sides() {
    let sides = [
        (MgrsPrecision::Km100, 100_000),
        (MgrsPrecision::Km10, 10_000),
        (MgrsPrecision::Km1, 1_000),
        (MgrsPrecision::M100, 100),
        (MgrsPrecision::M10, 10),
        (MgrsPrecision::M1, 1),
    ];
    for (precision, meters) in sides {
        assert_eq!(precision.meters(), meters, "{precision:?}");
    }
}

#[test]
fn encode_rejects_a_non_wgs84_coordinate() {
    let gcj = Coordinate::gcj02(40.0, 116.0).unwrap();
    assert!(matches!(
        Mgrs::try_from_coordinate(gcj, MgrsPrecision::M1),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn display_and_string_conversions_round_trip() {
    let m = Mgrs::try_from(String::from("z ab 0000 044542")).expect("valid");
    assert_eq!(m.to_string(), "ZAB0000044542");
    assert_eq!(m.to_string().parse::<Mgrs>().unwrap(), m);
    assert_eq!(String::from(m.clone()), m.as_str());
    assert_invalid_grid_ref(Mgrs::try_from(String::from("99TWK")));
}

#[test]
fn decode_recovers_each_reference() {
    for &(lat, lon, s) in REFS {
        let m = Mgrs::try_from(s).expect("valid MGRS");
        let approx = m.to_coordinate();
        let k_min = if s.starts_with(|c: char| c.is_ascii_digit()) {
            0.9996
        } else {
            0.994
        };
        assert_close(
            approx.max_error_m(),
            core::f64::consts::FRAC_1_SQRT_2 / k_min,
            1e-12,
        );
        // The published bound is a ground distance on the ellipsoid, so it
        // is checked with the geodesic oracle, not a spherical one.
        assert_within_geodesic_meters(approx.value(), &c(lat, lon), approx.max_error_m());
    }
}

/// The largest geodesic (ground) distance from the decoded center to any
/// corner of the cell, given the cell's grid corners.
fn worst_corner_distance(center: &Coordinate, corners: [Coordinate; 4]) -> f64 {
    corners
        .iter()
        .map(|corner| {
            crate::geodesy::geodesic_distance(center, corner)
                .unwrap()
                .meters()
        })
        .fold(0.0, f64::max)
}

#[test]
fn bound_covers_ground_distance_where_scale_is_minimal() {
    // Where k is at its minimum (the UTM central meridian, the UPS pole),
    // a grid half-diagonal is longest on the ground: grid / k. The bound
    // must cover it, and the old grid-meter bound falls short of it.
    let half_diag = 1000.0 * core::f64::consts::FRAC_1_SQRT_2;

    // UTM: zone 31's central meridian (easting 500 km) at the equator.
    let utm = Mgrs::try_from("31NEA0000").expect("valid");
    assert_eq!(utm.precision(), MgrsPrecision::Km1);
    let approx = utm.to_coordinate();
    let corner = |e: f64, n: f64| {
        Utm::new(31, UtmHemisphere::North, e, n)
            .unwrap()
            .to_coordinate()
    };
    let worst = worst_corner_distance(
        approx.value(),
        [
            corner(500_000.0, 0.0),
            corner(501_000.0, 0.0),
            corner(500_000.0, 1_000.0),
            corner(501_000.0, 1_000.0),
        ],
    );
    assert!(worst > half_diag, "UTM worst {worst} m");
    assert!(worst <= approx.max_error_m(), "UTM worst {worst} m");

    // UPS: the cell whose south-west corner is the North Pole.
    let ups = Mgrs::try_from("ZAH0000").expect("valid");
    assert_eq!(ups.precision(), MgrsPrecision::Km1);
    let approx = ups.to_coordinate();
    let corner = |e: f64, n: f64| {
        Ups::new(UtmHemisphere::North, e, n)
            .unwrap()
            .to_coordinate()
    };
    let worst = worst_corner_distance(
        approx.value(),
        [
            corner(2_000_000.0, 2_000_000.0),
            corner(2_001_000.0, 2_000_000.0),
            corner(2_000_000.0, 2_001_000.0),
            corner(2_001_000.0, 2_001_000.0),
        ],
    );
    assert!(worst > half_diag, "UPS worst {worst} m");
    assert!(worst <= approx.max_error_m(), "UPS worst {worst} m");
}

#[test]
fn encode_decode_round_trips_within_bound() {
    for &(lat, lon, _) in REFS {
        let m = Mgrs::try_from_coordinate(c(lat, lon), MgrsPrecision::M1).unwrap();
        let back = m.to_coordinate();
        assert_within_geodesic_meters(back.value(), &c(lat, lon), back.max_error_m());
    }
}

#[test]
fn from_str_round_trips_the_canonical_string() {
    // UTM and UPS (zone letters A/B/Y/Z) references alike: parsing the
    // canonical string and re-reading `as_str` is the identity, and the parsed
    // value equals the encoded one.
    for &(lat, lon, s) in REFS {
        let parsed: Mgrs = s.parse().expect("valid MGRS");
        assert_eq!(parsed.as_str(), s);
        assert_eq!(parsed.precision(), MgrsPrecision::M1);
        let encoded = Mgrs::try_from_coordinate(c(lat, lon), MgrsPrecision::M1).unwrap();
        assert_eq!(encoded.as_str().parse::<Mgrs>().unwrap(), encoded);
    }
    // A coarse UPS reference keeps its precision through the round trip.
    let z: Mgrs = "ZAB000044".parse().expect("valid UPS ref");
    assert_eq!(z.precision(), MgrsPrecision::M100);
    assert_eq!(z.as_str().parse::<Mgrs>().unwrap(), z);
    // Non-canonical input (lowercase, spaces) parses to the canonical form,
    // which is then a fixed point.
    let loose: Mgrs = "z ab 0000 044542".parse().expect("normalized");
    assert_eq!(loose.as_str(), "ZAB0000044542");
    assert_eq!(loose.as_str().parse::<Mgrs>().unwrap(), loose);
}

#[test]
fn coarser_cells_have_larger_bounds() {
    let m = Mgrs::try_from_coordinate(c(48.8584, 2.2945), MgrsPrecision::Km1).unwrap();
    assert_eq!(m.precision(), MgrsPrecision::Km1);
    assert_close(
        m.to_coordinate().max_error_m(),
        1000.0 * core::f64::consts::FRAC_1_SQRT_2 / 0.9996,
        1e-9,
    );
}

#[test]
fn parsing_accepts_lowercase_and_spaces() {
    let m = Mgrs::try_from("18t wk 00000 27757").expect("normalized");
    assert_eq!(m.as_str(), "18TWK0000027757");
}

#[test]
fn decode_coarse_precision_scales_the_digits() {
    // 100 m precision (3+3 digits): the digit value must be multiplied by the
    // cell size, not divided — and the result lands within one cell.
    let m = Mgrs::try_from("31UDQ482119").expect("valid");
    assert_eq!(m.precision(), MgrsPrecision::M100);
    let approx = m.to_coordinate();
    assert_close(
        approx.max_error_m(),
        100.0 * core::f64::consts::FRAC_1_SQRT_2 / 0.9996,
        1e-10,
    );
    assert_within_geodesic_meters(approx.value(), &c(48.8584, 2.2945), 100.0);
    // Polar coarse decode (UPS zone A) likewise.
    let p = Mgrs::try_from("APL239455").expect("valid");
    assert_within_geodesic_meters(p.to_coordinate().value(), &c(-82.0, -100.0), 100.0);
}

#[test]
fn decode_bare_hundred_kilometer_square() {
    // No digits at all: a valid 100 km reference (the trailing length is
    // exactly the three letters — must not be rejected as too short).
    let m = Mgrs::try_from("18TWK").expect("valid 100 km ref");
    assert_eq!(m.precision(), MgrsPrecision::Km100);
    assert_within_geodesic_meters(m.to_coordinate().value(), &c(40.0, -75.0), 80_000.0);
}

#[test]
fn decode_accepts_single_digit_zone() {
    // Zone 4 written without the leading zero (split == 1 is valid).
    let m = Mgrs::try_from("4QFH0460911793").expect("valid");
    assert_within_geodesic_meters(m.to_coordinate().value(), &c(20.0, -158.0), 1.0);
}

#[test]
fn decode_rejects_three_digit_zone() {
    // A three-digit zone designator (split > 2) is malformed.
    assert_invalid_grid_ref(Mgrs::try_from("012TWK0000027757"));
}

/// Every malformed reference is rejected as `InvalidGridRef`.
#[track_caller]
fn assert_invalid_grid_ref(r: Result<Mgrs>) {
    assert!(matches!(r, Err(Error::InvalidGridRef(_))), "{r:?}");
}

#[test]
fn decode_near_the_band_upper_edge() {
    // 47.95°N sits just under band T's 48° ceiling; the latitude-band check
    // must keep its upper slack (`hi + 0.5`), or this block is rejected.
    let m = Mgrs::try_from("31TFP4933212678").expect("valid");
    assert_within_geodesic_meters(m.to_coordinate().value(), &c(47.95, 5.0), 1.0);
}

#[test]
fn parsing_rejects_malformed_references() {
    assert_invalid_grid_ref(Mgrs::try_from(""));
    assert_invalid_grid_ref(Mgrs::try_from("18")); // no square
    assert_invalid_grid_ref(Mgrs::try_from("99TWK0000027757")); // zone > 60
    assert_invalid_grid_ref(Mgrs::try_from("61NAA")); // zone 61
    assert_invalid_grid_ref(Mgrs::try_from("0NAA")); // zone 0
    assert_invalid_grid_ref(Mgrs::try_from("18IWK0000027757")); // I is not a band letter
    assert_invalid_grid_ref(Mgrs::try_from("18TWK000002775")); // odd digit count
    assert_invalid_grid_ref(Mgrs::try_from("QWK0000027757")); // Q is not a UPS zone letter
    assert_invalid_grid_ref(Mgrs::try_from("18TIK0000027757")); // I not a column letter
    assert_invalid_grid_ref(Mgrs::try_from("18TWK000000277577")); // more than 5+5 digits
}

#[test]
fn multibyte_references_are_invalid_grid_refs_not_panics() {
    // Multibyte characters in the band/column/row position used to land a
    // byte-index slice off a char boundary and panic instead of erroring.
    for s in ["1ÉÉ1", "1ÉÉ", "ÉÉÉ", "18TÉÉ00000", "AXÉ00"] {
        assert!(
            matches!(Mgrs::try_from(s), Err(Error::InvalidGridRef(_))),
            "{s:?}"
        );
    }
}

#[test]
fn band_row_mismatch_is_invalid_grid_ref() {
    // Row A in zone 31 never falls in band P: blocks 0-4 miss the band and
    // block 5 onward exceeds the UTM northing domain. That out-of-domain
    // candidate used to surface as `InvalidValue { field: "UTM" }`.
    for s in ["31PAA0000000000", "31PAA"] {
        assert!(
            matches!(Mgrs::try_from(s), Err(Error::InvalidGridRef(_))),
            "{s:?}: {:?}",
            Mgrs::try_from(s)
        );
    }
}

#[test]
fn multibyte_inputs_never_panic() {
    // Deterministic sweep: place a 2-, 3-, and 4-byte UTF-8 character at
    // each of the first few positions of otherwise-valid UTM and UPS
    // references. Each is rejected as `InvalidGridRef`; a panic or any other
    // variant fails the test.
    for template in ["18TWK0000027757", "ZAH0000000000"] {
        for ch in ['é', '€', '🌍'] {
            for pos in 0..6.min(template.chars().count()) {
                let s: String = template
                    .chars()
                    .enumerate()
                    .map(|(i, c)| if i == pos { ch } else { c })
                    .collect();
                assert_invalid_grid_ref(Mgrs::try_from(s.as_str()));
            }
        }
    }
}

#[test]
fn squares_across_the_antimeridian_decode_wrapped() {
    // C6: zone 1's westmost column (easting 100–200 km) at the equator lies
    // east of 180°. It decodes to a wrapped longitude instead of falling
    // through to `InvalidGridRef`.
    let west = Mgrs::try_from("1NAA5000050000").expect("valid");
    let west = west.to_coordinate();
    assert_close(west.value().lat(), 0.452, 1e-3);
    assert!(west.value().lon() > 179.8, "{:?}", west.value());
    // The mirror square in zone 60's eastmost column wraps west.
    let east = Mgrs::try_from("60NZF5000050000").expect("valid");
    let east = east.to_coordinate();
    assert!(east.value().lon() < -179.8, "{:?}", east.value());
}

#[cfg(feature = "serde")]
#[test]
fn serde_validates_and_uses_the_canonical_string() {
    // C9: a derived struct form let an invalid reference deserialize, and
    // `to_coordinate` then panicked on it. The wire form is the canonical
    // string, validated exactly as parsing is.
    let invalid_struct = r#"{"text":"99TWK","precision_m":1}"#;
    assert!(serde_json::from_str::<Mgrs>(invalid_struct).is_err());
    for bad in [r#""99TWK""#, r#""18TWK000""#, r#""""#] {
        assert!(serde_json::from_str::<Mgrs>(bad).is_err(), "{bad}");
    }
    let m: Mgrs = serde_json::from_str(r#""18t wk 00000 27757""#).unwrap();
    assert_eq!(m.as_str(), "18TWK0000027757");
    assert_eq!(serde_json::to_string(&m).unwrap(), r#""18TWK0000027757""#);
}
