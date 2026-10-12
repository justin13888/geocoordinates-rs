//! Encoders never emit a code their own validator rejects (C8), deserializing
//! never admits a code `TryFrom<&str>` rejects (C9), and a decoded cell's
//! `max_error_m` is a true upper bound on the distance from its center to any
//! point of the cell (C13).

use proptest::prelude::*;

use super::*;
use crate::test_support::assert_within_geodesic_meters;

/// The last representable longitude before the antimeridian.
const LON_JUST_WEST_OF_180: f64 = 179.999_999_999_999_97;

/// Assert every corner of the `[lat_lo, lat_hi] x [lon_lo, lon_hi]` cell lies
/// within the decoded bound of its center.
#[track_caller]
fn assert_corners_within_bound(
    area: &Approx<Coordinate>,
    (lat_lo, lat_hi): (f64, f64),
    (lon_lo, lon_hi): (f64, f64),
) {
    for (lat, lon) in [
        (lat_lo, lon_lo),
        (lat_lo, lon_hi),
        (lat_hi, lon_lo),
        (lat_hi, lon_hi),
    ] {
        assert_within_geodesic_meters(&wgs(lat, lon), area.value(), area.max_error_m());
    }
}

#[test]
fn plus_code_encoder_output_always_validates() {
    // C8: latitude rounding up to 90 gave first lat digit 'F' (index 9), and a
    // longitude rounding up to 180 gave first lon digit 'W' (index 18).
    for (lat, lon) in [
        (89.999_999_99, 0.0),
        (0.0, LON_JUST_WEST_OF_180),
        (89.999_999_99, LON_JUST_WEST_OF_180),
    ] {
        for length in [2, 4, 6, 8, 10, 11, 15] {
            let code = PlusCode::encode(wgs(lat, lon), length).unwrap();
            assert_eq!(
                PlusCode::try_from(code.as_str()).as_ref(),
                Ok(&code),
                "({lat}, {lon}) at length {length}"
            );
        }
    }
}

#[test]
fn maidenhead_encoder_output_always_validates() {
    // C8: a longitude rounding up to 180 gave field 'S'.
    for (lat, lon) in [(0.0, LON_JUST_WEST_OF_180), (90.0, LON_JUST_WEST_OF_180)] {
        for pairs in 1..=3 {
            let code = Maidenhead::encode(wgs(lat, lon), pairs).unwrap();
            assert_eq!(
                Maidenhead::try_from(code.as_str()).as_ref(),
                Ok(&code),
                "({lat}, {lon}) at {pairs} pairs"
            );
        }
    }
}

#[test]
fn geohash_longer_than_twelve_is_rejected() {
    // Twelve characters already resolve ~19 mm x 37 mm; a longer string
    // carries bits an `f64` degree cannot hold.
    assert!(Geohash::try_from("u4pruydqqvjz").is_ok());
    assert_invalid_grid_ref(Geohash::try_from("u4pruydqqvjzz"));
    assert_invalid_value(Geohash::encode(wgs(0.0, 0.0), 13), "geohash length");
}

#[cfg(feature = "serde")]
#[test]
fn deserializing_an_overlong_geohash_is_rejected() {
    // C9: the serde route validates exactly as `TryFrom<&str>` does.
    assert!(serde_json::from_str::<Geohash>(r#""u4pruydqqvjzz""#).is_err());
}

#[test]
fn tall_polar_cells_bound_every_corner() {
    // C13: scaling by cos(center latitude) understates cells that reach the
    // pole, and 111,320 m/deg understates meridian arcs near the poles.
    assert_corners_within_bound(
        &Geohash::try_from("b").unwrap().decode(),
        (45.0, 90.0),
        (-180.0, -135.0),
    );
    assert_corners_within_bound(
        &Maidenhead::try_from("AR").unwrap().decode(),
        (80.0, 90.0),
        (-180.0, -160.0),
    );
    assert_corners_within_bound(
        &PlusCode::try_from("C2000000+").unwrap().decode(),
        (70.0, 90.0),
        (-180.0, -160.0),
    );
    // A small cell near the pole, where a meridian degree is ~111.69 km.
    assert_corners_within_bound(
        &Maidenhead::try_from("AR09ax").unwrap().decode(),
        (89.0 + 23.0 / 24.0, 90.0),
        (-180.0, -180.0 + 2.0 / 24.0),
    );
}

/// Any valid WGS-84 position, biased toward the poles and the antimeridian.
fn position() -> impl Strategy<Value = Coordinate> {
    let lat = prop_oneof![
        3 => -90.0..=90.0_f64,
        1 => prop::sample::select(vec![-90.0, 90.0, 89.999_999_99, -89.999_999_99]),
    ];
    let lon = prop_oneof![
        3 => -180.0..=180.0_f64,
        1 => prop::sample::select(vec![-180.0, 180.0, LON_JUST_WEST_OF_180, -LON_JUST_WEST_OF_180]),
    ];
    (lat, lon).prop_map(|(lat, lon)| wgs(lat, lon))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn plus_code_round_trips_through_its_validator(
        c in position(),
        length in prop::sample::select(vec![2, 4, 6, 8, 10, 11, 12, 13, 14, 15]),
    ) {
        let code = PlusCode::encode(c, length).unwrap();
        prop_assert_eq!(PlusCode::try_from(code.as_str()).unwrap(), code.clone());
        let area = code.decode();
        assert_within_geodesic_meters(&c, area.value(), area.max_error_m());
    }

    #[test]
    fn geohash_round_trips_through_its_validator(c in position(), length in 1..=12_u8) {
        let code = Geohash::encode(c, length.into()).unwrap();
        prop_assert_eq!(Geohash::try_from(code.as_str()).unwrap(), code.clone());
        let area = code.decode();
        assert_within_geodesic_meters(&c, area.value(), area.max_error_m());
    }

    #[test]
    fn maidenhead_round_trips_through_its_validator(c in position(), pairs in 1..=3_u8) {
        let code = Maidenhead::encode(c, pairs.into()).unwrap();
        prop_assert_eq!(Maidenhead::try_from(code.as_str()).unwrap(), code.clone());
        let area = code.decode();
        assert_within_geodesic_meters(&c, area.value(), area.max_error_m());
    }
}
