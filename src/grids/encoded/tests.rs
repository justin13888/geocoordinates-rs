use super::*;
use crate::Crs;
use crate::test_support::{assert_close, assert_within_meters};

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

/// Every malformed code string is rejected as `InvalidGridRef`.
#[track_caller]
fn assert_invalid_grid_ref<T: core::fmt::Debug>(r: Result<T>) {
    assert!(matches!(r, Err(Error::InvalidGridRef(_))), "{r:?}");
}

/// An out-of-domain encode length or pair count is `InvalidValue` on `field`.
#[track_caller]
fn assert_invalid_value<T: core::fmt::Debug>(r: Result<T>, expected: &str) {
    assert!(
        matches!(&r, Err(Error::InvalidValue { field, .. }) if *field == expected),
        "{r:?}"
    );
}

#[test]
fn encode_reference_vectors() {
    // Canonical Open Location Code test vectors.
    assert_eq!(
        PlusCode::encode(wgs(20.375, 2.775), 6).unwrap().as_str(),
        "7FG49Q00+"
    );
    assert_eq!(
        PlusCode::encode(wgs(47.0000625, 8.0000625), 10)
            .unwrap()
            .as_str(),
        "8FVC2222+22"
    );
}

#[test]
fn decode_known_code() {
    // 8FVC2222+22 is the length-10 cell whose center is the encode input.
    let area = PlusCode::try_from("8FVC2222+22").unwrap().decode();
    assert_close(area.lat(), 47.0000625, 1e-7);
    assert_close(area.lon(), 8.0000625, 1e-7);
    assert!(area.max_error_m() > 0.0 && area.max_error_m() < 20.0);
}

#[test]
fn round_trip_stays_within_bound() {
    // The original must lie within the decoded cell's reported error bound.
    let coords = [
        wgs(40.7128, -74.006),
        wgs(-33.8688, 151.2093),
        wgs(0.0, 0.0),
        wgs(51.5074, -0.1278),
        wgs(89.9999, 179.9999), // near a pole / the antimeridian
    ];
    for c in coords {
        for length in [10, 11, 12] {
            let code = PlusCode::encode(c, length).unwrap();
            let area = code.decode();
            assert_within_meters(&c, &*area, area.max_error_m());
        }
    }
}

#[test]
fn antimeridian_and_pole_do_not_panic() {
    // lat 90 / lon 180 must clamp/wrap and round-trip without panicking.
    let code = PlusCode::encode(wgs(90.0, 180.0), 10).unwrap();
    let area = code.decode();
    assert!(area.lat() <= 90.0 && area.lon() < 180.0);
}

#[test]
fn shorter_lengths_pad() {
    assert_eq!(
        PlusCode::encode(wgs(47.0000625, 8.0000625), 4)
            .unwrap()
            .as_str(),
        "8FVC0000+"
    );
}

#[test]
fn validation_accepts_and_rejects() {
    assert_eq!(
        PlusCode::try_from("87G7X2VV+2V").unwrap().as_str(),
        "87G7X2VV+2V"
    );
    // Lowercase input is canonicalized to uppercase.
    assert_eq!(
        PlusCode::try_from("87g7x2vv+2v").unwrap().as_str(),
        "87G7X2VV+2V"
    );
    // Surrounding whitespace is trimmed before validation.
    assert_eq!(
        PlusCode::try_from("  8fvc2222+22 ").unwrap().as_str(),
        "8FVC2222+22"
    );
    assert_eq!(
        PlusCode::try_from("8FVC2222+").unwrap().as_str(),
        "8FVC2222+"
    ); // length-8 full code
    assert_invalid_grid_ref(PlusCode::try_from("INVALID")); // no separator
    assert_invalid_grid_ref(PlusCode::try_from("8FVC2222")); // missing separator
    assert_invalid_grid_ref(PlusCode::try_from("8FVC2222+2")); // one char after separator
    assert_invalid_grid_ref(PlusCode::try_from("8FVC222+22")); // separator not at position 8
    assert_invalid_grid_ref(PlusCode::try_from("8FVC222A+22")); // 'A' not in the alphabet
    assert_invalid_grid_ref(PlusCode::try_from("")); // empty
}

#[test]
fn from_str_matches_try_from() {
    // `FromStr` is thin sugar over `TryFrom<&str>`: same canonical value, same
    // rejection variant.
    assert_eq!(
        "87g7x2vv+2v".parse::<PlusCode>().unwrap(),
        PlusCode::try_from("87G7X2VV+2V").unwrap()
    );
    assert_eq!("DR5REGY".parse::<Geohash>().unwrap().as_str(), "dr5regy");
    assert_eq!("jn58TD".parse::<Maidenhead>().unwrap().as_str(), "JN58td");
    assert_invalid_grid_ref("8FVC2222".parse::<PlusCode>());
    assert_invalid_grid_ref("ezs42a".parse::<Geohash>());
    assert_invalid_grid_ref("FN2".parse::<Maidenhead>());
}

#[test]
fn try_from_round_trips_canonical_string() {
    let code = PlusCode::encode(wgs(40.7128, -74.006), 11).unwrap();
    let reparsed = PlusCode::try_from(code.as_str()).unwrap();
    assert_eq!(code, reparsed);
}

#[test]
fn latitude_precision_values() {
    assert_close(latitude_precision(2), 20.0, 1e-9);
    assert_close(latitude_precision(8), 0.0025, 1e-9);
    assert_close(latitude_precision(10), 0.000125, 1e-12);
    assert_close(latitude_precision(11), 0.000125 / 5.0, 1e-12);
    assert_close(latitude_precision(15), 0.000125 / 3125.0, 1e-15);
}

#[test]
fn invalid_encoding_configuration_is_rejected() {
    let c = wgs(0.0, 0.0);
    assert_invalid_value(PlusCode::encode(c, 3), "Plus Code length");
    assert_invalid_value(PlusCode::encode(c, 9), "Plus Code length");
    assert_invalid_value(PlusCode::encode(c, 16), "Plus Code length");
    assert_invalid_value(Geohash::encode(c, 0), "geohash length");
    assert_invalid_value(Geohash::encode(c, 23), "geohash length");
    assert_invalid_value(Maidenhead::encode(c, 0), "Maidenhead pairs");
    assert_invalid_value(Maidenhead::encode(c, 4), "Maidenhead pairs");
    // Every encoder demands WGS-84 and rejects an out-of-range input first.
    let gcj = Coordinate::gcj02(0.0, 0.0).unwrap();
    let mismatch = |r: Result<String>| {
        matches!(
            r,
            Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                found: Crs::Gcj02,
            })
        )
    };
    assert!(mismatch(
        PlusCode::encode(gcj, 10).map(|p| p.as_str().into())
    ));
    assert!(mismatch(Geohash::encode(gcj, 5).map(|g| g.as_str().into())));
    assert!(mismatch(
        Maidenhead::encode(gcj, 2).map(|m| m.as_str().into())
    ));
    // An out-of-range input can no longer reach the encoders: construction
    // itself rejects it with that exact position.
    let bad = || Coordinate::wgs84(91.0, 0.0);
    assert!(matches!(
        bad().and_then(|c| Geohash::encode(c, 5)),
        Err(Error::OutOfRange { lat, lon }) if lat == 91.0 && lon == 0.0
    ));
    assert!(matches!(
        bad().and_then(|c| PlusCode::encode(c, 10)),
        Err(Error::OutOfRange { .. })
    ));
    assert!(matches!(
        bad().and_then(|c| Maidenhead::encode(c, 2)),
        Err(Error::OutOfRange { .. })
    ));
}

#[test]
fn latitude_90_is_nudged_below_max() {
    // Encoding latitude 90 nudges it just below the maximum — identical to
    // encoding the nudged value directly.
    let at_pole = PlusCode::encode(wgs(90.0, 0.0), 10).unwrap();
    let nudged = PlusCode::encode(wgs(90.0 - 0.000125, 0.0), 10).unwrap();
    assert_eq!(at_pole, nudged);
}

#[test]
fn decode_center_and_bound_precise() {
    let area = PlusCode::try_from("8FVC2222+22").unwrap().decode();
    assert_close(area.lat(), 47.0000625, 1e-9);
    assert_close(area.lon(), 8.0000625, 1e-9);
    // Cell is 0.000125° square; the bound is the half-diagonal in meters.
    let lat_m: f64 = 0.000125 * 111_320.0;
    let lon_m: f64 = 0.000125 * 111_320.0 * 47.0000625_f64.to_radians().cos();
    assert_close(area.max_error_m(), 0.5 * lat_m.hypot(lon_m), 1e-6);
}

#[test]
fn decode_grid_refinement() {
    // One grid digit (X = row 4, col 3) refines the length-10 cell.
    let area = PlusCode::try_from("8FVC2222+22X").unwrap().decode();
    assert_close(area.lat(), 47.0001 + 0.000_025 / 2.0, 1e-9);
    assert_close(area.lon(), 8.00009375 + 0.000_031_25 / 2.0, 1e-9);
    // The grid cell is far smaller than the length-10 cell (~8.4 m bound).
    assert!(area.max_error_m() < 4.0);
}

#[test]
fn decode_two_grid_digits() {
    // Two grid digits exercise the row/column grid place-value divisions.
    let area = PlusCode::try_from("8FVC2222+22XX").unwrap().decode();
    assert_close(area.lat(), 47.0001225, 1e-7);
    assert_close(area.lon(), 8.000121094, 1e-7);
}

#[test]
fn decode_center_within_absolute_bound() {
    // Coordinates with non-zero digits in every pair, checked against a
    // fixed ~14 m cell bound — so a broken pair place value (which would
    // also inflate the reported bound) can't hide behind it.
    for c in [wgs(40.7128, -74.006), wgs(-33.8688, 151.2093)] {
        let area = PlusCode::encode(c, 10).unwrap().decode();
        assert_within_meters(&c, &*area, 20.0);
    }
}

#[test]
fn validation_padding_and_range_rules() {
    // A first longitude digit of 10 (longitude 20°E) is in range and valid.
    assert_eq!(
        PlusCode::try_from("2G222222+").unwrap().as_str(),
        "2G222222+"
    );
    // Padding must start on an even, non-zero index, be contiguous, and end
    // at the separator.
    assert_eq!(
        PlusCode::try_from("8fvc0000+").unwrap().as_str(),
        "8FVC0000+"
    ); // well-formed padded code
    assert_invalid_grid_ref(PlusCode::try_from("00000000+")); // padding at index 0
    assert_invalid_grid_ref(PlusCode::try_from("8FVCC000+")); // padding starts on an odd index
    assert_invalid_grid_ref(PlusCode::try_from("8FVC0000+22")); // chars after a padded code
    // First latitude digit must be ≤ 8 and longitude ≤ 17 (range limits).
    assert_invalid_grid_ref(PlusCode::try_from("F2222222+")); // first lat digit = 9
    assert_invalid_grid_ref(PlusCode::try_from("2W222222+")); // first lon digit = 18
}

// --- Geohash ---

#[test]
fn geohash_reference_vectors() {
    assert_eq!(
        Geohash::encode(wgs(42.6, -5.6), 5).unwrap().as_str(),
        "ezs42"
    );
    assert_eq!(
        Geohash::encode(wgs(57.64911, 10.40744), 11)
            .unwrap()
            .as_str(),
        "u4pruydqqvj"
    );
}

#[test]
fn geohash_round_trip_within_bound() {
    for c in [
        wgs(40.7128, -74.006),
        wgs(-33.8688, 151.2093),
        wgs(0.0, 0.0),
        wgs(89.9, 179.9),
    ] {
        for len in [6, 8, 10] {
            let area = Geohash::encode(c, len).unwrap().decode();
            assert_within_meters(&c, &*area, area.max_error_m());
        }
    }
}

#[test]
fn geohash_decode_matches_known_cell() {
    let area = Geohash::try_from("ezs42").unwrap().decode();
    // Fixed ~5 km bound (the cell is ~5 km): a decode that fails to
    // alternate lon/lat would land thousands of km away.
    assert_within_meters(&wgs(42.605, -5.603), &*area, 5000.0);
    assert!(area.max_error_m() > 0.0);
}

#[test]
fn geohash_validation() {
    assert_eq!(Geohash::try_from("dr5regy").unwrap().as_str(), "dr5regy");
    // Case-insensitive: the canonical form is lowercase.
    assert_eq!(Geohash::try_from("DR5REGY").unwrap().as_str(), "dr5regy");
    // Surrounding whitespace is trimmed.
    assert_eq!(Geohash::try_from(" Ezs42\t").unwrap().as_str(), "ezs42");
    assert_invalid_grid_ref(Geohash::try_from("ezs42a")); // 'a' is excluded
    assert_invalid_grid_ref(Geohash::try_from("ezs4i")); // 'i' is excluded
    assert_invalid_grid_ref(Geohash::try_from("ezs 42")); // inner space
    assert_invalid_grid_ref(Geohash::try_from(""));
    assert_invalid_grid_ref(Geohash::try_from("   ")); // blank after trimming
}

// --- Maidenhead ---

#[test]
fn maidenhead_reference_and_decode() {
    assert_eq!(
        Maidenhead::encode(wgs(40.5, -75.0), 2).unwrap().as_str(),
        "FN20"
    );
    let area = Maidenhead::try_from("FN20").unwrap().decode();
    assert_close(area.lat(), 40.5, 1e-9);
    assert_close(area.lon(), -75.0, 1e-9);
    // The half-diagonal bound of the 2°×1° cell, pinned exactly.
    let lat_half: f64 = 0.5 * 111_320.0;
    let lon_half: f64 = 111_320.0 * 40.5_f64.to_radians().cos();
    assert_close(area.max_error_m(), lat_half.hypot(lon_half), 1e-6);
}

#[test]
fn maidenhead_subsquare_decode() {
    // JN58td: field J/N, square 5/8, subsquare t(19)/d(3).
    // SW corner (48.125, 11.583…); subsquare cell (2/24°)×(1/24°).
    let area = Maidenhead::try_from("JN58td").unwrap().decode();
    assert_close(area.lat(), 48.125 + (1.0 / 24.0) / 2.0, 1e-9);
    assert_close(area.lon(), 10.0 + 19.0 / 12.0 + (2.0 / 24.0) / 2.0, 1e-9);
}

#[test]
fn maidenhead_three_pairs_round_trip() {
    let c = wgs(48.146, 11.605);
    let mh = Maidenhead::encode(c, 3).unwrap();
    assert_eq!(mh.as_str().len(), 6);
    let area = mh.decode();
    assert_within_meters(&c, &*area, area.max_error_m());
}

#[test]
fn maidenhead_pole_does_not_panic() {
    let area = Maidenhead::encode(wgs(90.0, 180.0), 3).unwrap().decode();
    assert!(area.lat() <= 90.0 && area.lon() < 180.0);
}

#[test]
fn maidenhead_validation() {
    // Canonical form: field uppercase, square digits, subsquare lowercase.
    assert_eq!(Maidenhead::try_from("FN").unwrap().as_str(), "FN"); // 2-char field-only locator
    assert_eq!(Maidenhead::try_from("FN20").unwrap().as_str(), "FN20");
    assert_eq!(Maidenhead::try_from("JN58td").unwrap().as_str(), "JN58td");
    assert_eq!(Maidenhead::try_from("fn20").unwrap().as_str(), "FN20"); // case-insensitive field
    assert_eq!(Maidenhead::try_from("jn58TD").unwrap().as_str(), "JN58td");
    assert_eq!(Maidenhead::try_from(" jN58Td ").unwrap().as_str(), "JN58td"); // trimmed
    assert_invalid_grid_ref(Maidenhead::try_from("FN2")); // odd length
    assert_invalid_grid_ref(Maidenhead::try_from("FN20td00")); // 8 chars is out of scope
    assert_invalid_grid_ref(Maidenhead::try_from("")); // empty
    // 2-char: each field validated (second field invalid here).
    assert_invalid_grid_ref(Maidenhead::try_from("F0"));
    assert_invalid_grid_ref(Maidenhead::try_from("SN20")); // 'S' is past R
    // 6-char: each component validated individually.
    assert_invalid_grid_ref(Maidenhead::try_from("0N58td")); // field 1 invalid
    assert_invalid_grid_ref(Maidenhead::try_from("J058td")); // field 2 invalid
    assert_invalid_grid_ref(Maidenhead::try_from("JNX8td")); // square 1 non-digit
    assert_invalid_grid_ref(Maidenhead::try_from("JN5Xtd")); // square 2 non-digit
    assert_invalid_grid_ref(Maidenhead::try_from("JN580d")); // sub 1 past x ('0')
    assert_invalid_grid_ref(Maidenhead::try_from("JN58t0")); // sub 2 past x ('0')
    assert_invalid_grid_ref(Maidenhead::try_from("FN20zz")); // 'z' is past x
}
