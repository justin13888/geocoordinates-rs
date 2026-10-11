use super::*;
use crate::test_support::assert_close;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon)
}

#[test]
fn zone_selection_with_exceptions() {
    assert_eq!(zone_for(40.0, -75.0), 18); // central meridian −75°
    assert_eq!(zone_for(48.8584, 2.2945), 31);
    assert_eq!(zone_for(-33.8568, 151.2153), 56);
    // Norway: 5°E across band V belongs to the widened zone 32.
    assert_eq!(zone_for(60.0, 5.0), 32);
    assert_eq!(zone_for(60.0, 2.0), 31); // west of the widening stays 31
    // Both predicates must hold (not either): a 5°E point south of band V
    // keeps its normal zone.
    assert_eq!(zone_for(30.0, 5.0), 31);
    // Svalbard: odd zones 31/33/37 widen, 32/34/36 vanish.
    assert_eq!(zone_for(78.0, 8.0), 31);
    assert_eq!(zone_for(78.0, 20.0), 33);
    assert_eq!(zone_for(72.5, 35.0), 37);
    // The Svalbard widening also needs both predicates: 20°E below the band
    // keeps its normal zone 34, not the Svalbard zone 33.
    assert_eq!(zone_for(40.0, 20.0), 34);
    // Antimeridian and prime-meridian extremes.
    assert_eq!(zone_for(0.0, -180.0), 1);
    assert_eq!(zone_for(0.0, 179.999), 60);
}

#[test]
fn central_meridian_of_zones() {
    assert_close(central_meridian_deg(1), -177.0, 1e-12);
    assert_close(central_meridian_deg(18), -75.0, 1e-12);
    assert_close(central_meridian_deg(31), 3.0, 1e-12);
    assert_close(central_meridian_deg(60), 177.0, 1e-12);
}

#[test]
fn utm_forward_reference_points() {
    // References from the `utm` Python library (sub-mm agreement).
    let u = Utm::try_from_coordinate(c(40.0, -75.0)).unwrap();
    assert_eq!((u.zone, u.hemisphere), (18, Hemisphere::North));
    assert_close(u.easting, 500_000.0, 1e-3); // on the central meridian
    assert_close(u.northing, 4_427_757.218_844_7, 1e-3);

    let u = Utm::try_from_coordinate(c(48.8584, 2.2945)).unwrap();
    assert_eq!((u.zone, u.hemisphere), (31, Hemisphere::North));
    assert_close(u.easting, 448_252.001_375_2, 1e-3);
    assert_close(u.northing, 5_411_954.910_279_2, 1e-3);

    let u = Utm::try_from_coordinate(c(-33.8568, 151.2153)).unwrap();
    assert_eq!((u.zone, u.hemisphere), (56, Hemisphere::South));
    assert_close(u.easting, 334_900.569_650_6, 1e-3);
    assert_close(u.northing, 6_252_288.752_863_7, 1e-3);

    // The equator is northern (lat == 0 is not "< 0"), so no 10 000 km false
    // northing is applied.
    let eq = Utm::try_from_coordinate(c(0.0, 3.0)).unwrap();
    assert_eq!(eq.hemisphere, Hemisphere::North);
    assert_close(eq.northing, 0.0, 1e-3);
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
            .try_to_coordinate()
            .unwrap();
        assert_close(back.lat, lat, 1e-9);
        assert_close(back.lon, lon, 1e-9);
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
                .try_to_coordinate()
                .unwrap();
            let dn = (back.lat - lat).to_radians() * WGS84_A;
            let de = (back.lon - lon).to_radians() * WGS84_A * lat.to_radians().cos();
            worst_m = worst_m.max(dn.hypot(de));
        }
    }
    assert!(worst_m < 5e-6, "round-trip drift {worst_m} m");
}

#[test]
fn utm_rejects_polar_latitudes() {
    // 84°N is UPS.
    assert_out_of_range(Utm::try_from_coordinate(c(84.0, 10.0)), 84.0, 10.0);
    assert_out_of_range(
        Utm::try_from_coordinate(c(-80.000_1, 10.0)),
        -80.000_1,
        10.0,
    );
    // Just under the cap is the widened Svalbard zone 33.
    let u = Utm::try_from_coordinate(c(83.999, 10.0)).unwrap();
    assert_eq!((u.zone, u.hemisphere), (33, Hemisphere::North));
    // 80°S itself is the last UTM latitude.
    let u = Utm::try_from_coordinate(c(-80.0, 10.0)).unwrap();
    assert_eq!((u.zone, u.hemisphere), (32, Hemisphere::South));
}

#[test]
fn ups_forward_reference_points() {
    // References from pyproj (EPSG:5041 north / 5042 south).
    let u = Ups::try_from_coordinate(c(85.0, 0.0)).unwrap();
    assert_eq!(u.hemisphere, Hemisphere::North);
    assert_close(u.easting, 2_000_000.0, 1e-3);
    assert_close(u.northing, 1_444_542.608_6, 1e-3);

    let u = Ups::try_from_coordinate(c(87.0, 45.0)).unwrap();
    assert_close(u.easting, 2_235_568.724_8, 1e-3);
    assert_close(u.northing, 1_764_431.275_2, 1e-3);

    let u = Ups::try_from_coordinate(c(-85.0, 30.0)).unwrap();
    assert_eq!(u.hemisphere, Hemisphere::South);
    assert_close(u.easting, 2_277_728.695_7, 1e-3);
    assert_close(u.northing, 2_481_040.211_7, 1e-3);
}

#[test]
fn ups_pole_is_the_false_origin() {
    let np = Ups::try_from_coordinate(c(90.0, 0.0)).unwrap();
    assert_close(np.easting, 2_000_000.0, 1e-6);
    assert_close(np.northing, 2_000_000.0, 1e-6);
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
            .try_to_coordinate()
            .unwrap();
        assert_close(back.lat, lat, 1e-9);
        assert_close(back.lon, lon, 1e-9);
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
                .try_to_coordinate()
                .unwrap();
            (back.lat - lat).abs().to_radians() * A_AXIS
        })
        .fold(0.0_f64, f64::max);
    assert!(worst_m < 6e-5, "UPS inverse error {worst_m} m");
}

#[test]
fn ups_rejects_non_polar_latitudes() {
    assert_out_of_range(Ups::try_from_coordinate(c(50.0, 0.0)), 50.0, 0.0);
    assert_out_of_range(Ups::try_from_coordinate(c(83.9, 0.0)), 83.9, 0.0); // still UTM
    assert_out_of_range(Ups::try_from_coordinate(c(-79.0, 0.0)), -79.0, 0.0);
    assert_out_of_range(Ups::try_from_coordinate(c(0.0, 0.0)), 0.0, 0.0); // equator is north
    assert_eq!(
        Ups::try_from_coordinate(c(84.0, 0.0)).unwrap().hemisphere,
        Hemisphere::North
    );
    assert_eq!(
        Ups::try_from_coordinate(c(-80.0, 0.0)).unwrap().hemisphere,
        Hemisphere::South
    );
}

#[test]
fn projections_reject_wrong_crs_and_invalid_records() {
    use crate::Crs::{Gcj02, Wgs84};
    assert!(matches!(
        Utm::try_from_coordinate(Coordinate::gcj02(40.0, -75.0)),
        Err(Error::CrsMismatch {
            expected: Wgs84,
            found: Gcj02,
        })
    ));
    assert!(matches!(
        Ups::try_from_coordinate(Coordinate::gcj02(85.0, 0.0)),
        Err(Error::CrsMismatch {
            expected: Wgs84,
            found: Gcj02,
        })
    ));
    assert_out_of_range(Utm::try_from_coordinate(c(91.0, 0.0)), 91.0, 0.0);
    assert_out_of_range(Ups::try_from_coordinate(c(91.0, 0.0)), 91.0, 0.0);
}

/// An out-of-range input is `OutOfRange` carrying exactly that position.
#[track_caller]
fn assert_out_of_range<T: core::fmt::Debug>(r: Result<T>, lat: f64, lon: f64) {
    assert!(
        matches!(&r, Err(Error::OutOfRange { lat: a, lon: o }) if *a == lat && *o == lon),
        "{r:?}"
    );
}

/// A record outside its projection's domain is `InvalidValue` on `field`.
#[track_caller]
fn assert_invalid_record(r: Result<Coordinate>, expected: &str) {
    assert!(
        matches!(&r, Err(Error::InvalidValue { field, .. }) if *field == expected),
        "{r:?}"
    );
}

fn utm(zone: u8, hemisphere: Hemisphere, easting: f64, northing: f64) -> Utm {
    Utm {
        zone,
        hemisphere,
        easting,
        northing,
    }
}

fn ups(hemisphere: Hemisphere, easting: f64, northing: f64) -> Ups {
    Ups {
        hemisphere,
        easting,
        northing,
    }
}

#[test]
fn utm_inverse_guards_each_field() {
    use Hemisphere::{North, South};
    let bad = |u: Utm| assert_invalid_record(u.try_to_coordinate(), "UTM");
    bad(utm(0, North, 500_000.0, 0.0));
    bad(utm(61, North, 500_000.0, 0.0));
    bad(utm(31, North, 99_999.0, 0.0));
    bad(utm(31, North, 900_001.0, 0.0));
    bad(utm(31, North, f64::INFINITY, 0.0));
    bad(utm(31, North, f64::NAN, 0.0));
    bad(utm(31, North, 500_000.0, -1.0));
    bad(utm(31, South, 500_000.0, 10_000_001.0));
    bad(utm(31, North, 500_000.0, f64::NAN));
    bad(utm(31, North, 500_000.0, f64::NEG_INFINITY));
    // The domain bounds themselves are accepted.
    let edge = utm(31, North, 100_000.0, 0.0).try_to_coordinate().unwrap();
    assert_close(edge.lon, -0.592_327_203, 1e-8);
    let edge = utm(31, North, 900_000.0, 0.0).try_to_coordinate().unwrap();
    assert_close(edge.lon, 6.592_327_203, 1e-8);
}

#[test]
fn ups_inverse_guards_each_field() {
    use Hemisphere::{North, South};
    let bad = |u: Ups| assert_invalid_record(u.try_to_coordinate(), "UPS");
    bad(ups(North, -1.0, 2_000_000.0));
    bad(ups(North, 4_000_001.0, 2_000_000.0));
    bad(ups(South, f64::NAN, 2_000_000.0));
    bad(ups(North, f64::INFINITY, 2_000_000.0));
    bad(ups(North, 2_000_000.0, -1.0));
    bad(ups(South, 2_000_000.0, 4_000_001.0));
    bad(ups(North, 2_000_000.0, f64::NAN));
    // The domain corner itself is accepted, far below the 84°N cap.
    let corner = ups(North, 4_000_000.0, 4_000_000.0)
        .try_to_coordinate()
        .unwrap();
    assert_close(corner.lat, 64.916_412_333, 1e-8);
    assert_close(corner.lon, 135.0, 1e-9);
}

#[test]
fn utm_inverse_does_not_wrap_the_antimeridian() {
    use Hemisphere::North;
    // KNOWN WRONG (C6, flipped by #114): zone 1 easting 150 km at the
    // equator lies at ≈179.856°E across the antimeridian, but the inverse
    // never wraps longitude, so it fails validation as lon ≈ −180.144.
    let r = utm(1, North, 150_000.0, 0.0).try_to_coordinate();
    assert!(
        matches!(r, Err(Error::OutOfRange { lat, lon })
            if lat == 0.0 && (lon + 180.143_772_343).abs() < 1e-8),
        "{r:?}"
    );
    // KNOWN WRONG (C6, flipped by #114): the mirror case in zone 60.
    let r = utm(60, North, 850_000.0, 0.0).try_to_coordinate();
    assert!(
        matches!(r, Err(Error::OutOfRange { lon, .. }) if (lon - 180.143_772_343).abs() < 1e-8),
        "{r:?}"
    );
    // Inside ±180 the same zone decodes normally.
    let p = utm(1, North, 170_000.0, 0.0).try_to_coordinate().unwrap();
    assert_close(p.lon, -179.964_294_459, 1e-8);
}

#[test]
fn utm_inverse_folds_northings_over_the_pole() {
    use Hemisphere::North;
    // KNOWN WRONG (C6, flipped by #114): a north northing of 9 999 000 m is
    // past the pole (≈9 997 964 m), yet it is accepted and folds over onto
    // the far meridian: zone 1 (−177°) lands on +3°.
    let p = utm(1, North, 500_000.0, 9_999_000.0)
        .try_to_coordinate()
        .unwrap();
    assert_close(p.lat, 89.990_729_391, 1e-8);
    assert_close(p.lon, 3.0, 1e-9);
    // In zone 31 the fold lands on 183°, which only validation catches.
    assert_out_of_range(
        utm(31, North, 500_000.0, 9_999_000.0).try_to_coordinate(),
        89.990_729_391_400_34,
        183.0,
    );
}

#[test]
fn zone_edges_of_the_norway_and_svalbard_exceptions() {
    // Svalbard (72°–84°N, 0°–42°E): boundaries at 9°, 21°, 33° and 42°E.
    assert_eq!(zone_for(72.0, 0.0), 31);
    assert_eq!(zone_for(72.0, 8.999), 31);
    assert_eq!(zone_for(72.0, 9.0), 33);
    assert_eq!(zone_for(72.0, 20.999), 33);
    assert_eq!(zone_for(72.0, 21.0), 35);
    assert_eq!(zone_for(72.0, 32.999), 35);
    assert_eq!(zone_for(72.0, 33.0), 37);
    assert_eq!(zone_for(72.0, 41.999), 37);
    assert_eq!(zone_for(72.0, 42.0), 38); // the exception ends at 42°E
    assert_eq!(zone_for(83.999, 9.0), 33);
    assert_eq!(zone_for(84.0, 9.0), 32); // 84°N is outside the exception
    assert_eq!(zone_for(71.999, 9.0), 32); // band W keeps its normal zone
    assert_eq!(zone_for(72.0, -0.001), 30); // west of 0°E is unaffected
    // Norway (56°–64°N, 3°–12°E) widens zone 32 westward.
    assert_eq!(zone_for(56.0, 3.0), 32);
    assert_eq!(zone_for(56.0, 2.999), 31);
    assert_eq!(zone_for(55.999, 3.0), 31);
    assert_eq!(zone_for(63.999, 3.0), 32);
    assert_eq!(zone_for(64.0, 3.0), 31); // 64°N is outside the exception
    assert_eq!(zone_for(56.0, 11.999), 32);
    assert_eq!(zone_for(56.0, 12.0), 33);
}
