//! #183: `Utm::new` / `Ups::new` accept only the forward image, so every
//! value that constructs decodes to a point that projects back to it.

use super::*;

#[test]
fn a_point_within_the_slack_of_a_band_edge_takes_the_side_its_zone_has() {
    use UtmHemisphere::North;
    // Zone 32 ends at 72°N (Svalbard drops it): a decoded point a hair north
    // of 72° is moved just south of it, where zone 32 exists.
    assert_eq!(
        utm_image(32, North, 72.0 + 1e-10, 10.0),
        Ok((below(72.0), 10.0))
    );
    assert_eq!(utm_image(32, North, 72.0 + 1e-8, 10.0), Err("easting"));
    // Zone 31 east of 3°E exists only outside Norway's 56°–64°N band.
    assert_eq!(
        utm_image(31, North, 56.0 + 1e-10, 4.0),
        Ok((below(56.0), 4.0))
    );
    assert_eq!(utm_image(31, North, below(64.0), 4.0), Ok((64.0, 4.0)));
}

#[test]
fn a_longitude_an_ulp_west_of_a_zone_meridian_keeps_the_western_zone() {
    // East of 0°, `lon + 180.0` rounds an ulp below a meridian up onto it
    // (5.999…° + 180 is 186), naming the zone east of the point.
    for k in 1..60u8 {
        let meridian = -180.0 + 6.0 * f64::from(k);
        assert_eq!(zone(0.0, below(meridian)), k, "west of {meridian}");
        assert_eq!(zone(0.0, meridian), k + 1, "at {meridian}");
    }
}

/// `utm` decodes to a point whose forward projection is `utm` again: the same
/// zone and hemisphere, and the same easting and northing to 1 mm.
#[track_caller]
fn assert_utm_round_trips(utm: Utm) {
    let back = Utm::try_from_coordinate(utm.to_coordinate())
        .unwrap_or_else(|e| panic!("{utm:?} decodes outside the UTM image: {e}"));
    assert_eq!(
        (back.zone(), back.hemisphere()),
        (utm.zone(), utm.hemisphere())
    );
    assert_close(back.easting_m(), utm.easting_m(), 1e-3);
    assert_close(back.northing_m(), utm.northing_m(), 1e-3);
}

/// As [`assert_utm_round_trips`], for UPS.
#[track_caller]
fn assert_ups_round_trips(ups: Ups) {
    let back = Ups::try_from_coordinate(ups.to_coordinate())
        .unwrap_or_else(|e| panic!("{ups:?} decodes outside the UPS image: {e}"));
    assert_eq!(back.hemisphere(), ups.hemisphere());
    assert_close(back.easting_m(), ups.easting_m(), 1e-3);
    assert_close(back.northing_m(), ups.northing_m(), 1e-3);
}

/// A latitude or longitude near one of `edges`, offset by `d · scale`, where
/// the scales run from a millidegree down to below an ulp.
fn near_edge(edges: Vec<f64>) -> impl proptest::strategy::Strategy<Value = f64> {
    use proptest::strategy::Strategy;
    (
        proptest::sample::select(edges),
        -1.0f64..1.0,
        proptest::sample::select(vec![1e-3, 1e-6, 1e-9, 1e-12, 1e-15, 0.0]),
    )
        .prop_map(|(edge, d, scale)| edge + d * scale)
}

proptest::proptest! {
    /// Every `Utm` that constructs round-trips through the forward
    /// projection.
    #[test]
    fn every_constructible_utm_round_trips(
        zone in 1u8..=60,
        south in proptest::bool::ANY,
        easting in 100_000.0f64..=900_000.0,
        northing in 0.0f64..=10_000_000.0,
    ) {
        let hemisphere = if south { UtmHemisphere::South } else { UtmHemisphere::North };
        if let Ok(u) = Utm::new(zone, hemisphere, easting, northing) {
            assert_utm_round_trips(u);
        }
    }

    /// Every `Ups` that constructs round-trips through the forward
    /// projection.
    #[test]
    fn every_constructible_ups_round_trips(
        south in proptest::bool::ANY,
        easting in 0.0f64..=4_000_000.0,
        northing in 0.0f64..=4_000_000.0,
    ) {
        let hemisphere = if south { UtmHemisphere::South } else { UtmHemisphere::North };
        if let Ok(u) = Ups::new(hemisphere, easting, northing) {
            assert_ups_round_trips(u);
        }
    }

    /// Every value the forward projection produces, at the image's edges
    /// included (80°S, 84°N, the equator, the Norway / Svalbard band edges,
    /// and every zone meridian), is accepted by `new`, which keeps serde
    /// lossless, and round-trips.
    #[test]
    fn every_forward_result_constructs_and_round_trips(
        lat in near_edge(vec![-80.0, -45.0, 0.0, 56.0, 64.0, 72.0, 84.0]),
        lon in near_edge(
            (0..=60).map(|k| -180.0 + 6.0 * f64::from(k)).chain([3.0, 9.0, 21.0, 33.0, 42.0]).collect(),
        ),
    ) {
        let coord = c(lat.clamp(-90.0, 90.0), lon.clamp(-180.0, 180.0));
        if let Ok(u) = Utm::try_from_coordinate(coord) {
            let rebuilt = Utm::new(u.zone(), u.hemisphere(), u.easting_m(), u.northing_m());
            assert_eq!(rebuilt, Ok(u));
            assert_utm_round_trips(u);
        }
    }

    /// As above, for UPS at the cap edges and the poles.
    #[test]
    fn every_ups_forward_result_constructs_and_round_trips(
        lat in near_edge(vec![-90.0, -80.0, 84.0, 90.0]),
        lon in -180.0f64..=180.0,
    ) {
        if let Ok(u) = Ups::try_from_coordinate(c(lat.clamp(-90.0, 90.0), lon)) {
            let rebuilt = Ups::new(u.hemisphere(), u.easting_m(), u.northing_m());
            assert_eq!(rebuilt, Ok(u));
            assert_ups_round_trips(u);
        }
    }
}
