
use super::*;
use crate::test_support::{assert_close, assert_within_meters};

#[test]
fn encode_reference_vectors() {
    // Canonical Open Location Code test vectors.
    assert_eq!(
        PlusCode::encode(Coordinate::wgs84(20.375, 2.775), 6)
            .unwrap()
            .as_str(),
        "7FG49Q00+"
    );
    assert_eq!(
        PlusCode::encode(Coordinate::wgs84(47.0000625, 8.0000625), 10)
            .unwrap()
            .as_str(),
        "8FVC2222+22"
    );
}

#[test]
fn decode_known_code() {
    // 8FVC2222+22 is the length-10 cell whose center is the encode input.
    let area = PlusCode::try_from("8FVC2222+22").unwrap().decode();
    assert_close(area.lat, 47.0000625, 1e-7);
    assert_close(area.lon, 8.0000625, 1e-7);
    assert!(area.max_error_m() > 0.0 && area.max_error_m() < 20.0);
}

#[test]
fn round_trip_stays_within_bound() {
    // The original must lie within the decoded cell's reported error bound.
    let coords = [
        Coordinate::wgs84(40.7128, -74.006),
        Coordinate::wgs84(-33.8688, 151.2093),
        Coordinate::wgs84(0.0, 0.0),
        Coordinate::wgs84(51.5074, -0.1278),
        Coordinate::wgs84(89.9999, 179.9999), // near a pole / the antimeridian
    ];
    for c in coords {
        for length in [10, 11, 12] {
            let code = PlusCode::encode(c, length).unwrap();
            let area = code.decode();
            assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, area.max_error_m());
        }
    }
}

#[test]
fn antimeridian_and_pole_do_not_panic() {
    // lat 90 / lon 180 must clamp/wrap and round-trip without panicking.
    let code = PlusCode::encode(Coordinate::wgs84(90.0, 180.0), 10).unwrap();
    let area = code.decode();
    assert!(area.lat <= 90.0 && area.lon < 180.0);
}

#[test]
fn shorter_lengths_pad() {
    assert_eq!(
        PlusCode::encode(Coordinate::wgs84(47.0000625, 8.0000625), 4)
            .unwrap()
            .as_str(),
        "8FVC0000+"
    );
}

#[test]
fn validation_accepts_and_rejects() {
    assert!(PlusCode::try_from("87G7X2VV+2V").is_ok());
    assert!(PlusCode::try_from("87g7x2vv+2v").is_ok()); // lowercase normalized
    assert!(PlusCode::try_from("8FVC2222+").is_ok()); // length-8 full code
    assert!(PlusCode::try_from("INVALID").is_err()); // no separator
    assert!(PlusCode::try_from("8FVC2222").is_err()); // missing separator
    assert!(PlusCode::try_from("8FVC2222+2").is_err()); // one char after separator
    assert!(PlusCode::try_from("8FVC222+22").is_err()); // separator not at position 8
    assert!(PlusCode::try_from("8FVC222A+22").is_err()); // 'A' not in the alphabet
    assert!(PlusCode::try_from("").is_err()); // empty
}

#[test]
fn try_from_round_trips_canonical_string() {
    let code = PlusCode::encode(Coordinate::wgs84(40.7128, -74.006), 11).unwrap();
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
    let c = Coordinate::wgs84(0.0, 0.0);
    assert!(PlusCode::encode(c, 3).is_err());
    assert!(Geohash::encode(c, 0).is_err());
    assert!(Geohash::encode(c, 23).is_err());
    assert!(Maidenhead::encode(c, 0).is_err());
    assert!(PlusCode::encode(Coordinate::gcj02(0.0, 0.0), 10).is_err());
}

#[test]
fn latitude_90_is_nudged_below_max() {
    // Encoding latitude 90 nudges it just below the maximum — identical to
    // encoding the nudged value directly.
    let at_pole = PlusCode::encode(Coordinate::wgs84(90.0, 0.0), 10).unwrap();
    let nudged = PlusCode::encode(Coordinate::wgs84(90.0 - 0.000125, 0.0), 10).unwrap();
    assert_eq!(at_pole, nudged);
}

#[test]
fn decode_center_and_bound_precise() {
    let area = PlusCode::try_from("8FVC2222+22").unwrap().decode();
    assert_close(area.lat, 47.0000625, 1e-9);
    assert_close(area.lon, 8.0000625, 1e-9);
    // Cell is 0.000125° square; the bound is the half-diagonal in meters.
    let lat_m: f64 = 0.000125 * 111_320.0;
    let lon_m: f64 = 0.000125 * 111_320.0 * 47.0000625_f64.to_radians().cos();
    assert_close(area.max_error_m(), 0.5 * lat_m.hypot(lon_m), 1e-6);
}

#[test]
fn decode_grid_refinement() {
    // One grid digit (X = row 4, col 3) refines the length-10 cell.
    let area = PlusCode::try_from("8FVC2222+22X").unwrap().decode();
    assert_close(area.lat, 47.0001 + 0.000_025 / 2.0, 1e-9);
    assert_close(area.lon, 8.00009375 + 0.000_031_25 / 2.0, 1e-9);
    // The grid cell is far smaller than the length-10 cell (~8.4 m bound).
    assert!(area.max_error_m() < 4.0);
}

#[test]
fn decode_two_grid_digits() {
    // Two grid digits exercise the row/column grid place-value divisions.
    let area = PlusCode::try_from("8FVC2222+22XX").unwrap().decode();
    assert_close(area.lat, 47.0001225, 1e-7);
    assert_close(area.lon, 8.000121094, 1e-7);
}

#[test]
fn decode_center_within_absolute_bound() {
    // Coordinates with non-zero digits in every pair, checked against a
    // fixed ~14 m cell bound — so a broken pair place value (which would
    // also inflate the reported bound) can't hide behind it.
    for c in [
        Coordinate::wgs84(40.7128, -74.006),
        Coordinate::wgs84(-33.8688, 151.2093),
    ] {
        let area = PlusCode::encode(c, 10).unwrap().decode();
        assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, 20.0);
    }
}

#[test]
fn validation_padding_and_range_rules() {
    // A first longitude digit of 10 (longitude 20°E) is in range and valid.
    assert!(PlusCode::try_from("2G222222+").is_ok());
    // Padding must start on an even, non-zero index, be contiguous, and end
    // at the separator.
    assert!(PlusCode::try_from("8FVC0000+").is_ok()); // well-formed padded code
    assert!(PlusCode::try_from("00000000+").is_err()); // padding at index 0
    assert!(PlusCode::try_from("8FVCC000+").is_err()); // padding starts on an odd index
    assert!(PlusCode::try_from("8FVC0000+22").is_err()); // chars after a padded code
    // First latitude digit must be ≤ 8 and longitude ≤ 17 (range limits).
    assert!(PlusCode::try_from("F2222222+").is_err()); // first lat digit = 9
    assert!(PlusCode::try_from("2W222222+").is_err()); // first lon digit = 18
}

// --- Geohash ---

#[test]
fn geohash_reference_vectors() {
    assert_eq!(
        Geohash::encode(Coordinate::wgs84(42.6, -5.6), 5)
            .unwrap()
            .as_str(),
        "ezs42"
    );
    assert_eq!(
        Geohash::encode(Coordinate::wgs84(57.64911, 10.40744), 11)
            .unwrap()
            .as_str(),
        "u4pruydqqvj"
    );
}

#[test]
fn geohash_round_trip_within_bound() {
    for c in [
        Coordinate::wgs84(40.7128, -74.006),
        Coordinate::wgs84(-33.8688, 151.2093),
        Coordinate::wgs84(0.0, 0.0),
        Coordinate::wgs84(89.9, 179.9),
    ] {
        for len in [6, 8, 10] {
            let area = Geohash::encode(c, len).unwrap().decode();
            assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, area.max_error_m());
        }
    }
}

#[test]
fn geohash_decode_matches_known_cell() {
    let area = Geohash::try_from("ezs42").unwrap().decode();
    // Fixed ~5 km bound (the cell is ~5 km): a decode that fails to
    // alternate lon/lat would land thousands of km away.
    assert_within_meters(&Coordinate::wgs84(42.605, -5.603), &*area, 5000.0);
    assert!(area.max_error_m() > 0.0);
}

#[test]
fn geohash_validation() {
    assert!(Geohash::try_from("dr5regy").is_ok());
    assert!(Geohash::try_from("DR5REGY").is_ok()); // case-insensitive
    assert!(Geohash::try_from("ezs42a").is_err()); // 'a' is excluded
    assert!(Geohash::try_from("ezs4i").is_err()); // 'i' is excluded
    assert!(Geohash::try_from("").is_err());
}

// --- Maidenhead ---

#[test]
fn maidenhead_reference_and_decode() {
    assert_eq!(
        Maidenhead::encode(Coordinate::wgs84(40.5, -75.0), 2)
            .unwrap()
            .as_str(),
        "FN20"
    );
    let area = Maidenhead::try_from("FN20").unwrap().decode();
    assert_close(area.lat, 40.5, 1e-9);
    assert_close(area.lon, -75.0, 1e-9);
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
    assert_close(area.lat, 48.125 + (1.0 / 24.0) / 2.0, 1e-9);
    assert_close(area.lon, 10.0 + 19.0 / 12.0 + (2.0 / 24.0) / 2.0, 1e-9);
}

#[test]
fn maidenhead_three_pairs_round_trip() {
    let c = Coordinate::wgs84(48.146, 11.605);
    let mh = Maidenhead::encode(c, 3).unwrap();
    assert_eq!(mh.as_str().len(), 6);
    let area = mh.decode();
    assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, area.max_error_m());
}

#[test]
fn maidenhead_pole_does_not_panic() {
    let area = Maidenhead::encode(Coordinate::wgs84(90.0, 180.0), 3)
        .unwrap()
        .decode();
    assert!(area.lat <= 90.0 && area.lon < 180.0);
}

#[test]
fn maidenhead_validation() {
    assert!(Maidenhead::try_from("FN").is_ok()); // 2-char field-only locator
    assert!(Maidenhead::try_from("FN20").is_ok());
    assert!(Maidenhead::try_from("JN58td").is_ok());
    assert!(Maidenhead::try_from("fn20").is_ok()); // case-insensitive field
    assert!(Maidenhead::try_from("FN2").is_err()); // odd length
    assert!(Maidenhead::try_from("").is_err()); // empty
    // 2-char: each field validated (second field invalid here).
    assert!(Maidenhead::try_from("F0").is_err());
    assert!(Maidenhead::try_from("SN20").is_err()); // 'S' is past R
    // 6-char: each component validated individually.
    assert!(Maidenhead::try_from("0N58td").is_err()); // field 1 invalid
    assert!(Maidenhead::try_from("J058td").is_err()); // field 2 invalid
    assert!(Maidenhead::try_from("JNX8td").is_err()); // square 1 non-digit
    assert!(Maidenhead::try_from("JN5Xtd").is_err()); // square 2 non-digit
    assert!(Maidenhead::try_from("JN580d").is_err()); // sub 1 past x ('0')
    assert!(Maidenhead::try_from("JN58t0").is_err()); // sub 2 past x ('0')
    assert!(Maidenhead::try_from("FN20zz").is_err()); // 'z' is past x
}
