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
    assert!(Utm::try_from_coordinate(c(84.0, 10.0)).is_err()); // 84°N is UPS
    assert!(Utm::try_from_coordinate(c(-80.000_1, 10.0)).is_err());
    assert!(Utm::try_from_coordinate(c(83.999, 10.0)).is_ok());
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
    assert!(Ups::try_from_coordinate(c(50.0, 0.0)).is_err());
    assert!(Ups::try_from_coordinate(c(83.9, 0.0)).is_err()); // still UTM
    assert!(Ups::try_from_coordinate(c(-79.0, 0.0)).is_err());
    assert!(Ups::try_from_coordinate(c(84.0, 0.0)).is_ok());
    assert!(Ups::try_from_coordinate(c(-80.0, 0.0)).is_ok());
}

#[test]
fn projections_reject_wrong_crs_and_invalid_records() {
    assert!(Utm::try_from_coordinate(Coordinate::gcj02(40.0, -75.0)).is_err());
    assert!(
        Utm {
            zone: 0,
            hemisphere: Hemisphere::North,
            easting: 500_000.0,
            northing: 0.0,
        }
        .try_to_coordinate()
        .is_err()
    );
    assert!(
        Ups {
            hemisphere: Hemisphere::North,
            easting: f64::NAN,
            northing: 2_000_000.0,
        }
        .try_to_coordinate()
        .is_err()
    );
}
