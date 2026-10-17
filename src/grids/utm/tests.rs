use super::*;
use crate::test_support::assert_close;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

/// The UTM zone of a WGS-84 point inside the UTM band.
#[track_caller]
fn zone(lat: f64, lon: f64) -> u8 {
    utm_zone_for(c(lat, lon)).unwrap()
}

/// A UTM coordinate known to be inside the domain.
#[track_caller]
fn utm(zone: u8, hemisphere: UtmHemisphere, easting: f64, northing: f64) -> Utm {
    Utm::new(zone, hemisphere, easting, northing).unwrap()
}

/// A UPS coordinate known to be inside the domain.
#[track_caller]
fn ups(hemisphere: UtmHemisphere, easting: f64, northing: f64) -> Ups {
    Ups::new(hemisphere, easting, northing).unwrap()
}

/// A valid point outside `operation`'s domain is `OutsideDomain` carrying
/// exactly that operation and position.
#[track_caller]
fn assert_outside<T: core::fmt::Debug>(r: Result<T>, operation: &str, lat: f64, lon: f64) {
    assert!(
        matches!(&r, Err(Error::OutsideDomain { operation: op, lat: a, lon: o })
            if *op == operation && *a == lat && *o == lon),
        "{r:?}"
    );
}

/// A part outside its projection's domain is `InvalidValue` on `field`.
#[track_caller]
fn assert_invalid<T: core::fmt::Debug>(r: Result<T>, expected: &str) {
    assert!(
        matches!(&r, Err(Error::InvalidValue { field, .. }) if *field == expected),
        "{r:?}"
    );
}

#[test]
fn zone_selection_with_exceptions() {
    assert_eq!(zone(40.0, -75.0), 18); // central meridian −75°
    assert_eq!(zone(48.8584, 2.2945), 31);
    assert_eq!(zone(-33.8568, 151.2153), 56);
    // Norway: 5°E across band V belongs to the widened zone 32.
    assert_eq!(zone(60.0, 5.0), 32);
    assert_eq!(zone(60.0, 2.0), 31); // west of the widening stays 31
    // Both predicates must hold (not either): a 5°E point south of band V
    // keeps its normal zone.
    assert_eq!(zone(30.0, 5.0), 31);
    // Svalbard: odd zones 31/33/37 widen, 32/34/36 vanish.
    assert_eq!(zone(78.0, 8.0), 31);
    assert_eq!(zone(78.0, 20.0), 33);
    assert_eq!(zone(72.5, 35.0), 37);
    // The Svalbard widening also needs both predicates: 20°E below the band
    // keeps its normal zone 34, not the Svalbard zone 33.
    assert_eq!(zone(40.0, 20.0), 34);
    // Antimeridian and prime-meridian extremes.
    assert_eq!(zone(0.0, -180.0), 1);
    assert_eq!(zone(0.0, 179.999), 60);
}

#[test]
fn central_meridian_of_zones() {
    assert_close(utm_central_meridian_deg(1).unwrap(), -177.0, 1e-12);
    assert_close(utm_central_meridian_deg(18).unwrap(), -75.0, 1e-12);
    assert_close(utm_central_meridian_deg(31).unwrap(), 3.0, 1e-12);
    assert_close(utm_central_meridian_deg(60).unwrap(), 177.0, 1e-12);
    // Zones 0 and 61 do not exist (zone 0 used to report −183°).
    assert_invalid(utm_central_meridian_deg(0), "zone");
    assert_invalid(utm_central_meridian_deg(61), "zone");
}

#[test]
fn utm_zone_for_rejects_points_outside_the_utm_band() {
    // 84°N itself is UPS territory, as is everything south of 80°S.
    assert_outside(utm_zone_for(c(84.0, 9.0)), "UTM", 84.0, 9.0);
    assert_outside(utm_zone_for(c(-80.000_1, 9.0)), "UTM", -80.000_1, 9.0);
    assert_outside(utm_zone_for(c(90.0, 0.0)), "UTM", 90.0, 0.0);
    assert_eq!(zone(-80.0, 9.0), 32);
    assert_eq!(zone(83.999, 9.0), 33);
    assert!(matches!(
        utm_zone_for(Coordinate::gcj02(40.0, -75.0).unwrap()),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
}

#[test]
fn utm_forward_reference_points() {
    // References from the `utm` Python library (sub-mm agreement).
    let u = Utm::try_from_coordinate(c(40.0, -75.0)).unwrap();
    assert_eq!((u.zone(), u.hemisphere()), (18, UtmHemisphere::North));
    assert_close(u.easting_m(), 500_000.0, 1e-3); // on the central meridian
    assert_close(u.northing_m(), 4_427_757.218_844_7, 1e-3);

    let u = Utm::try_from_coordinate(c(48.8584, 2.2945)).unwrap();
    assert_eq!((u.zone(), u.hemisphere()), (31, UtmHemisphere::North));
    assert_close(u.easting_m(), 448_252.001_375_2, 1e-3);
    assert_close(u.northing_m(), 5_411_954.910_279_2, 1e-3);

    let u = Utm::try_from_coordinate(c(-33.8568, 151.2153)).unwrap();
    assert_eq!((u.zone(), u.hemisphere()), (56, UtmHemisphere::South));
    assert_close(u.easting_m(), 334_900.569_650_6, 1e-3);
    assert_close(u.northing_m(), 6_252_288.752_863_7, 1e-3);

    // The equator is northern (lat == 0 is not "< 0"), so no 10 000 km false
    // northing is applied.
    let eq = Utm::try_from_coordinate(c(0.0, 3.0)).unwrap();
    assert_eq!(eq.hemisphere(), UtmHemisphere::North);
    assert_close(eq.northing_m(), 0.0, 1e-3);
}

#[test]
fn utm_round_trips_sub_millimeter() {
    for &(lat, lon) in &[
        (40.0, -75.0),
        (48.8584, 2.2945),
        (-33.8568, 151.2153),
        (60.0, 5.0),
        (-80.0 + 1e-6, -179.0),
        (83.9, 100.0),
    ] {
        let back = Utm::try_from_coordinate(c(lat, lon))
            .unwrap()
            .to_coordinate();
        assert_close(back.lat(), lat, 1e-9);
        assert_close(back.lon(), lon, 1e-9);
    }
}

#[test]
fn utm_round_trip_drift_is_a_few_micrometers() {
    // Sweep zone 18 (78°W–72°W, no Norway/Svalbard exceptions) from 80°S
    // to 84°N and measure the round-trip drift as a ground distance.
    const WGS84_A: f64 = 6_378_137.0;
    let mut worst_m = 0.0_f64;
    for i in 0..=82 {
        let lat = (f64::from(i) * 2.0 - 80.0).min(83.9);
        for j in 0..12 {
            let lon = -78.0 + 0.5 * f64::from(j) + 0.001;
            let back = Utm::try_from_coordinate(c(lat, lon))
                .unwrap()
                .to_coordinate();
            let dn = (back.lat() - lat).to_radians() * WGS84_A;
            let de = (back.lon() - lon).to_radians() * WGS84_A * lat.to_radians().cos();
            worst_m = worst_m.max(dn.hypot(de));
        }
    }
    assert!(worst_m < 5e-6, "round-trip drift {worst_m} m");
}

#[test]
fn utm_rejects_polar_latitudes() {
    // 84°N is UPS.
    assert_outside(Utm::try_from_coordinate(c(84.0, 10.0)), "UTM", 84.0, 10.0);
    assert_outside(
        Utm::try_from_coordinate(c(-80.000_1, 10.0)),
        "UTM",
        -80.000_1,
        10.0,
    );
    // Just under the cap is the widened Svalbard zone 33.
    let u = Utm::try_from_coordinate(c(83.999, 10.0)).unwrap();
    assert_eq!((u.zone(), u.hemisphere()), (33, UtmHemisphere::North));
    // 80°S itself is the last UTM latitude.
    let u = Utm::try_from_coordinate(c(-80.0, 10.0)).unwrap();
    assert_eq!((u.zone(), u.hemisphere()), (32, UtmHemisphere::South));
}

#[test]
fn ups_forward_reference_points() {
    // References from pyproj (EPSG:5041 north / 5042 south).
    let u = Ups::try_from_coordinate(c(85.0, 0.0)).unwrap();
    assert_eq!(u.hemisphere(), UtmHemisphere::North);
    assert_close(u.easting_m(), 2_000_000.0, 1e-3);
    assert_close(u.northing_m(), 1_444_542.608_6, 1e-3);

    let u = Ups::try_from_coordinate(c(87.0, 45.0)).unwrap();
    assert_close(u.easting_m(), 2_235_568.724_8, 1e-3);
    assert_close(u.northing_m(), 1_764_431.275_2, 1e-3);

    let u = Ups::try_from_coordinate(c(-85.0, 30.0)).unwrap();
    assert_eq!(u.hemisphere(), UtmHemisphere::South);
    assert_close(u.easting_m(), 2_277_728.695_7, 1e-3);
    assert_close(u.northing_m(), 2_481_040.211_7, 1e-3);
}

#[test]
fn ups_pole_is_the_false_origin() {
    let np = Ups::try_from_coordinate(c(90.0, 0.0)).unwrap();
    assert_close(np.easting_m(), 2_000_000.0, 1e-6);
    assert_close(np.northing_m(), 2_000_000.0, 1e-6);
}

#[test]
fn ups_round_trips() {
    for &(lat, lon) in &[
        (85.0, 0.0),
        (87.0, 45.0),
        (84.5, 170.0),
        (-85.0, 30.0),
        (-88.0, -120.0),
    ] {
        let back = Ups::try_from_coordinate(c(lat, lon))
            .unwrap()
            .to_coordinate();
        assert_close(back.lat(), lat, 1e-9);
        assert_close(back.lon(), lon, 1e-9);
    }
}

#[test]
fn ups_inverse_series_error_matches_the_documented_bound() {
    // The forward is closed-form, so a round trip measures the truncated
    // inverse series alone. Sweep both polar caps: the documented figure is
    // about 0.05 mm of latitude.
    let worst_m = (0..=6000)
        .flat_map(|i| {
            let t = f64::from(i) * 0.001;
            [84.0 + t, -80.0 - t * 10.0 / 6.0]
        })
        .map(|lat| {
            let back = Ups::try_from_coordinate(c(lat, 37.0))
                .unwrap()
                .to_coordinate();
            (back.lat() - lat).abs().to_radians() * A_AXIS
        })
        .fold(0.0_f64, f64::max);
    assert!(worst_m < 6e-5, "UPS inverse error {worst_m} m");
}

#[test]
fn ups_rejects_non_polar_latitudes() {
    assert_outside(Ups::try_from_coordinate(c(50.0, 0.0)), "UPS", 50.0, 0.0);
    assert_outside(Ups::try_from_coordinate(c(83.9, 0.0)), "UPS", 83.9, 0.0); // still UTM
    assert_outside(Ups::try_from_coordinate(c(-79.0, 0.0)), "UPS", -79.0, 0.0);
    assert_outside(Ups::try_from_coordinate(c(0.0, 0.0)), "UPS", 0.0, 0.0); // equator is north
    assert_eq!(
        Ups::try_from_coordinate(c(84.0, 0.0)).unwrap().hemisphere(),
        UtmHemisphere::North
    );
    assert_eq!(
        Ups::try_from_coordinate(c(-80.0, 0.0))
            .unwrap()
            .hemisphere(),
        UtmHemisphere::South
    );
}

#[test]
fn projections_reject_wrong_crs_and_out_of_range_input_is_unconstructible() {
    use crate::Crs::{Gcj02, Wgs84};
    assert!(matches!(
        Utm::try_from_coordinate(Coordinate::gcj02(40.0, -75.0).unwrap()),
        Err(Error::CrsMismatch {
            expected: Wgs84,
            found: Gcj02,
        })
    ));
    assert!(matches!(
        Ups::try_from_coordinate(Coordinate::gcj02(85.0, 0.0).unwrap()),
        Err(Error::CrsMismatch {
            expected: Wgs84,
            found: Gcj02,
        })
    ));
    // An out-of-range input can no longer reach the projections: construction
    // itself rejects it with that exact position.
    let bad = || Coordinate::wgs84(91.0, 0.0);
    assert!(matches!(
        bad().and_then(Utm::try_from_coordinate),
        Err(Error::OutOfRange {
            lat: 91.0,
            lon: 0.0
        })
    ));
    assert!(matches!(
        bad().and_then(Ups::try_from_coordinate),
        Err(Error::OutOfRange {
            lat: 91.0,
            lon: 0.0
        })
    ));
}

#[test]
fn utm_new_guards_each_field() {
    use UtmHemisphere::{North, South};
    assert_invalid(Utm::new(0, North, 500_000.0, 0.0), "zone");
    assert_invalid(Utm::new(61, North, 500_000.0, 0.0), "zone");
    assert_invalid(Utm::new(31, North, 99_999.0, 0.0), "easting");
    assert_invalid(Utm::new(31, North, 900_001.0, 0.0), "easting");
    assert_invalid(Utm::new(31, North, f64::INFINITY, 0.0), "easting");
    assert_invalid(Utm::new(31, North, f64::NAN, 0.0), "easting");
    assert_invalid(Utm::new(31, North, 500_000.0, -1.0), "northing");
    assert_invalid(Utm::new(31, South, 500_000.0, 10_000_001.0), "northing");
    assert_invalid(Utm::new(31, North, 500_000.0, f64::NAN), "northing");
    assert_invalid(
        Utm::new(31, North, 500_000.0, f64::NEG_INFINITY),
        "northing",
    );
    // The zone is checked first, then the easting, then the northing.
    assert_invalid(Utm::new(0, North, f64::NAN, f64::NAN), "zone");
    assert_invalid(Utm::new(31, North, f64::NAN, f64::NAN), "easting");
    // The getters return exactly the validated parts.
    let u = utm(31, South, 234_567.0, 7_654_321.0);
    assert_eq!(
        (u.zone(), u.hemisphere(), u.easting_m(), u.northing_m()),
        (31, South, 234_567.0, 7_654_321.0)
    );
    // The numeric easting bounds are only a pre-filter: at the equator they
    // decode to ≈ 0.59° beyond zone 31 on either side, outside its image.
    assert_invalid(Utm::new(31, North, 100_000.0, 0.0), "easting");
    assert_invalid(Utm::new(31, North, 900_000.0, 0.0), "easting");
}

#[test]
fn utm_new_accepts_only_the_forward_image() {
    use UtmHemisphere::{North, South};
    // #183: inside every numeric range, but ≈ 89°N, which the forward
    // direction refuses (UPS territory).
    assert_invalid(Utm::new(18, North, 500_000.0, 9_900_000.0), "northing");
    // 84°N on the central meridian lies between 9 328 and 9 330 km: past it is UPS.
    assert_invalid(Utm::new(18, North, 500_000.0, 9_330_000.0), "northing");
    let u = utm(18, North, 500_000.0, 9_328_000.0);
    assert!(u.to_coordinate().lat() < 84.0);
    // 80°S there lies between 1 117 and 1 119 km: south of it is UPS.
    assert_invalid(Utm::new(18, South, 500_000.0, 1_117_000.0), "northing");
    utm(18, South, 500_000.0, 1_119_000.0);
    // Svalbard drops zone 32 above 72°N, so no zone-32 value lies there.
    assert_invalid(Utm::new(32, North, 500_000.0, 8_100_000.0), "easting");
    // Norway widens zone 32 west to 3°E in 56°–64°N, so 330 km easting at
    // 60°N (≈ 3.6°E) is zone 32, and zone 31 ends at 3°E there.
    utm(32, North, 330_000.0, 6_650_000.0);
    assert_invalid(Utm::new(31, North, 640_000.0, 6_650_000.0), "easting");
}

#[test]
fn utm_new_rejects_eastings_across_the_antimeridian() {
    use UtmHemisphere::North;
    // C6 / #183: zone 1 easting 150 km at the equator lies at ≈179.856°E,
    // zone 60's longitude; it used to decode there, wrapped, and is now
    // outside zone 1's image. Likewise zone 60's mirror easting.
    assert_invalid(Utm::new(1, North, 150_000.0, 0.0), "easting");
    assert_invalid(Utm::new(60, North, 850_000.0, 0.0), "easting");
    // Inside ±180 the same zone decodes normally.
    let p = utm(1, North, 170_000.0, 0.0).to_coordinate();
    assert_close(p.lon(), -179.964_294_459, 1e-8);
}

#[test]
fn ups_new_guards_each_field() {
    use UtmHemisphere::{North, South};
    assert_invalid(Ups::new(North, -1.0, 2_000_000.0), "easting");
    assert_invalid(Ups::new(North, 4_000_001.0, 2_000_000.0), "easting");
    assert_invalid(Ups::new(South, f64::NAN, 2_000_000.0), "easting");
    assert_invalid(Ups::new(North, f64::INFINITY, 2_000_000.0), "easting");
    assert_invalid(Ups::new(North, 2_000_000.0, -1.0), "northing");
    assert_invalid(Ups::new(South, 2_000_000.0, 4_000_001.0), "northing");
    assert_invalid(Ups::new(North, 2_000_000.0, f64::NAN), "northing");
    let u = ups(South, 2_277_728.5, 2_481_040.25);
    assert_eq!(
        (u.hemisphere(), u.easting_m(), u.northing_m()),
        (South, 2_277_728.5, 2_481_040.25)
    );
}

#[test]
fn ups_new_accepts_only_the_polar_cap() {
    use UtmHemisphere::{North, South};
    // #183: the grid's corners decode to ≈ 64.9°, far equatorward of either
    // cap; the farther offset from the false origin is blamed, the northing
    // on a tie.
    assert_invalid(Ups::new(North, 0.0, 0.0), "northing");
    assert_invalid(Ups::new(South, 4_000_000.0, 4_000_000.0), "northing");
    assert_invalid(Ups::new(North, 0.0, 2_000_000.0), "easting");
    assert_invalid(Ups::new(South, 2_000_000.0, 3_200_000.0), "northing");
    // 84°N lies ≈ 667 km from the north pole, 80°S ≈ 1 113 km from the south.
    ups(North, 2_000_000.0, 2_666_000.0);
    assert_invalid(Ups::new(North, 2_000_000.0, 2_668_000.0), "northing");
    ups(South, 3_112_000.0, 2_000_000.0);
    assert_invalid(Ups::new(South, 3_115_000.0, 2_000_000.0), "easting");
}

#[test]
fn utm_inverse_rejects_northings_past_the_pole() {
    use UtmHemisphere::North;
    // C6: a north northing of 9 999 000 m is past the pole (≈9 997 964 m);
    // it must be rejected, not folded over onto the far meridian.
    let r = Utm::new(1, North, 500_000.0, 9_999_000.0);
    assert!(
        matches!(
            r,
            Err(Error::InvalidValue {
                field: "northing",
                ..
            })
        ),
        "{r:?}"
    );
}

#[test]
fn utm_northing_domain_ends_exactly_at_each_pole() {
    use UtmHemisphere::{North, South};
    // The pole's grid northing is the K0-scaled quarter meridian.
    assert_close(POLE_NORTHING, 9_997_964.943, 1e-3);
    // The numeric grid domain (what MGRS decoding probes) stops at the pole:
    // the pole itself decodes to 90°N; a centimeter past it would fold over
    // and is rejected. `Utm::new` rejects the pole too: it is UPS.
    let grid = |h, n| Utm::from_grid(31, h, 500_000.0, n);
    assert_close(
        grid(North, POLE_NORTHING).unwrap().to_coordinate().lat(),
        90.0,
        1e-9,
    );
    assert_invalid(grid(North, POLE_NORTHING + 0.01), "northing");
    assert_invalid(Utm::new(31, North, 500_000.0, POLE_NORTHING), "northing");
    // South: the same distance below the 10 000 km false northing.
    let south_pole = FALSE_NORTHING_SOUTH - POLE_NORTHING;
    assert_close(
        grid(South, south_pole).unwrap().to_coordinate().lat(),
        -90.0,
        1e-9,
    );
    assert_invalid(grid(South, south_pole - 0.01), "northing");
    assert_invalid(Utm::new(31, South, 500_000.0, south_pole), "northing");
    // The equator bounds each hemisphere from the other side. A southern
    // 10 000 000 m decodes to the equator, which is northern, so it is moved
    // a hair south and the forward keeps it southern.
    let p = utm(31, South, 500_000.0, FALSE_NORTHING_SOUTH).to_coordinate();
    assert!(p.lat() < 0.0 && p.lat() > -1e-300, "{p:?}");
    let back = Utm::try_from_coordinate(p).unwrap();
    assert_eq!(back, utm(31, South, 500_000.0, FALSE_NORTHING_SOUTH));
    assert_close(
        utm(31, North, 500_000.0, 0.0).to_coordinate().lat(),
        0.0,
        1e-12,
    );
    // Near the pole but off the central meridian the inverse stays on the
    // zone's side of the pole (|Δλ| < 90°) instead of folding.
    let p = Utm::from_grid(31, North, 900_000.0, POLE_NORTHING - 1.0)
        .unwrap()
        .to_coordinate();
    assert!(p.lat() > 86.0 && (3.0..93.0).contains(&p.lon()), "{p:?}");
}

#[test]
fn every_forward_result_is_inside_the_constructor_domain() {
    // The forward builds `Utm` / `Ups` without `new`; sweep the band edges,
    // the Norway / Svalbard widenings, and the antimeridian to confirm it
    // never produces a value `new` would reject.
    for lat in [-80.0, -45.0, -0.000_1, 0.0, 56.0, 63.999, 72.0, 83.999] {
        for lon in [
            -180.0, -177.0, 0.0, 2.999, 3.0, 8.999, 41.999, 179.999, 180.0,
        ] {
            let u = Utm::try_from_coordinate(c(lat, lon)).unwrap();
            let rebuilt = Utm::new(u.zone(), u.hemisphere(), u.easting_m(), u.northing_m());
            assert_eq!(rebuilt, Ok(u), "lat={lat}, lon={lon}");
        }
    }
    for lat in [84.0, 87.0, 90.0, -80.000_1, -85.0, -90.0] {
        for lon in [-180.0, -135.0, -45.0, 0.0, 45.0, 135.0, 180.0] {
            let u = Ups::try_from_coordinate(c(lat, lon)).unwrap();
            let rebuilt = Ups::new(u.hemisphere(), u.easting_m(), u.northing_m());
            assert_eq!(rebuilt, Ok(u), "lat={lat}, lon={lon}");
        }
    }
}

#[test]
fn inverse_longitude_is_always_wrapped_into_range() {
    // Zone 60 at the antimeridian decodes just west of 180°, not to −180°
    // (zone 1), so it re-encodes to zone 60; the UPS pole (atan2 of ±0) is
    // wrapped into range.
    let u = Utm::try_from_coordinate(c(0.0, 180.0)).unwrap();
    assert_eq!(u.zone(), 60);
    let back = u.to_coordinate();
    assert!(back.lon() < 180.0 && back.lon() > 180.0 - 1e-9, "{back:?}");
    assert_eq!(Utm::try_from_coordinate(back).unwrap().zone(), 60);
    let p = Ups::try_from_coordinate(c(90.0, 0.0))
        .unwrap()
        .to_coordinate();
    assert_close(p.lat(), 90.0, 1e-12);
    assert!((-180.0..180.0).contains(&p.lon()), "{p:?}");
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_rejects_out_of_domain_records() {
    let u = utm(31, UtmHemisphere::North, 448_252.0, 5_411_954.0);
    let json = serde_json::to_string(&u).unwrap();
    assert_eq!(
        json,
        r#"{"zone":31,"hemisphere":"North","easting":448252.0,"northing":5411954.0}"#
    );
    assert_eq!(serde_json::from_str::<Utm>(&json).unwrap(), u);
    let bad = r#"{"zone":61,"hemisphere":"North","easting":448252.0,"northing":5411954.0}"#;
    assert!(serde_json::from_str::<Utm>(bad).is_err());
    let past_pole = r#"{"zone":1,"hemisphere":"North","easting":500000.0,"northing":9999000.0}"#;
    assert!(serde_json::from_str::<Utm>(past_pole).is_err());

    let p = ups(UtmHemisphere::South, 2_277_728.5, 2_481_040.25);
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(serde_json::from_str::<Ups>(&json).unwrap(), p);
    let bad = r#"{"hemisphere":"South","easting":-1.0,"northing":2000000.0}"#;
    assert!(serde_json::from_str::<Ups>(bad).is_err());
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

#[test]
fn zone_edges_of_the_norway_and_svalbard_exceptions() {
    // Svalbard (72°–84°N, 0°–42°E): boundaries at 9°, 21°, 33° and 42°E.
    assert_eq!(zone(72.0, 0.0), 31);
    assert_eq!(zone(72.0, 8.999), 31);
    assert_eq!(zone(72.0, 9.0), 33);
    assert_eq!(zone(72.0, 20.999), 33);
    assert_eq!(zone(72.0, 21.0), 35);
    assert_eq!(zone(72.0, 32.999), 35);
    assert_eq!(zone(72.0, 33.0), 37);
    assert_eq!(zone(72.0, 41.999), 37);
    assert_eq!(zone(72.0, 42.0), 38); // the exception ends at 42°E
    assert_eq!(zone(83.999, 9.0), 33);
    // 84°N is outside the exception and the UTM band (it is UPS).
    assert_outside(utm_zone_for(c(84.0, 9.0)), "UTM", 84.0, 9.0);
    assert_eq!(zone(71.999, 9.0), 32); // band W keeps its normal zone
    assert_eq!(zone(72.0, -0.001), 30); // west of 0°E is unaffected
    // Norway (56°–64°N, 3°–12°E) widens zone 32 westward.
    assert_eq!(zone(56.0, 3.0), 32);
    assert_eq!(zone(56.0, 2.999), 31);
    assert_eq!(zone(55.999, 3.0), 31);
    assert_eq!(zone(63.999, 3.0), 32);
    assert_eq!(zone(64.0, 3.0), 31); // 64°N is outside the exception
    assert_eq!(zone(56.0, 11.999), 32);
    assert_eq!(zone(56.0, 12.0), 33);
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
    /// #183: every `Utm` that constructs round-trips through the forward
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

    /// #183: every `Ups` that constructs round-trips through the forward
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
