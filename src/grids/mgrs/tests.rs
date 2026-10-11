
use super::*;
use crate::test_support::{assert_close, assert_within_geodesic_meters};

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon)
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
            Mgrs::try_from_coordinate(c(lat, lon), 1).unwrap().as_str(),
            expected,
            "({lat},{lon})"
        );
    }
}

#[test]
fn encode_precision_levels() {
    let m = Mgrs::try_from_coordinate(c(40.0, -75.0), 100).unwrap();
    assert_eq!(m.as_str(), "18TWK000277"); // 3+3 digits
    assert_eq!(m.precision_m(), 100);
    // A non-zero easting remainder confirms the digits are scaled (divided),
    // not multiplied: 48 252 m → "482" at 100 m precision.
    assert_eq!(
        Mgrs::try_from_coordinate(c(48.8584, 2.2945), 100)
            .unwrap()
            .as_str(),
        "31UDQ482119"
    );
    // 10 km square (1 digit each), and the bare 100 km square.
    assert_eq!(
        Mgrs::try_from_coordinate(c(40.0, -75.0), 10_000)
            .unwrap()
            .as_str(),
        "18TWK02"
    );
    assert_eq!(
        Mgrs::try_from_coordinate(c(40.0, -75.0), 100_000)
            .unwrap()
            .as_str(),
        "18TWK"
    );
    assert!(Mgrs::try_from_coordinate(c(40.0, -75.0), 250).is_err());
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
    assert_eq!(utm.precision_m(), 1000);
    let approx = utm.to_coordinate();
    let corner = |e: f64, n: f64| {
        Utm {
            zone: 31,
            hemisphere: Hemisphere::North,
            easting: e,
            northing: n,
        }
        .try_to_coordinate()
        .unwrap()
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
    assert_eq!(ups.precision_m(), 1000);
    let approx = ups.to_coordinate();
    let corner = |e: f64, n: f64| {
        Ups {
            hemisphere: Hemisphere::North,
            easting: e,
            northing: n,
        }
        .try_to_coordinate()
        .unwrap()
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
fn from_str_round_trips_through_encode() {
    for &(lat, lon, _) in REFS {
        let m = Mgrs::try_from_coordinate(c(lat, lon), 1).unwrap();
        let back = m.to_coordinate();
        assert_within_geodesic_meters(back.value(), &c(lat, lon), back.max_error_m());
    }
}

#[test]
fn coarser_cells_have_larger_bounds() {
    let m = Mgrs::try_from_coordinate(c(48.8584, 2.2945), 1000).unwrap();
    assert_eq!(m.precision_m(), 1000);
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
    assert_eq!(m.precision_m(), 100);
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
    assert_eq!(m.precision_m(), 100_000);
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
    assert!(Mgrs::try_from("012TWK0000027757").is_err());
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
    assert!(Mgrs::try_from("").is_err());
    assert!(Mgrs::try_from("18").is_err()); // no square
    assert!(Mgrs::try_from("99TWK0000027757").is_err()); // zone > 60
    assert!(Mgrs::try_from("18IWK0000027757").is_err()); // I is not a band letter
    assert!(Mgrs::try_from("18TWK000002775").is_err()); // odd digit count
    assert!(Mgrs::try_from("QWK0000027757").is_err()); // Q is not a UPS zone letter
    assert!(Mgrs::try_from("18TIK0000027757").is_err()); // I not a column letter
    assert!(Mgrs::try_from("18TWK000000277577").is_err()); // more than 5+5 digits
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
    // references. A panic (rather than a returned Err) fails the test.
    for template in ["18TWK0000027757", "ZAH0000000000"] {
        for ch in ['é', '€', '🌍'] {
            for pos in 0..6.min(template.chars().count()) {
                let s: String = template
                    .chars()
                    .enumerate()
                    .map(|(i, c)| if i == pos { ch } else { c })
                    .collect();
                let _ = Mgrs::try_from(s.as_str());
            }
        }
    }
}
