use super::*;
use crate::test_support::assert_close;

const WGS84_A: f64 = 6_378_137.0;
const WGS84_B: f64 = 6_356_752.314_245_18;

fn at(lat: f64, lon: f64, h: f64, crs: Crs) -> Coordinate {
    Coordinate::new(lat, lon, crs)
        .unwrap()
        .with_height(Height::Ellipsoidal(h))
        .unwrap()
}

fn ellipsoidal_height(c: Coordinate) -> f64 {
    match c.height() {
        Some(Height::Ellipsoidal(h)) => h,
        other => panic!("expected ellipsoidal height, got {other:?}"),
    }
}

fn distance_m(a: Ecef, b: Ecef) -> f64 {
    (a.x - b.x).hypot(a.y - b.y).hypot(a.z - b.z)
}

/// ECEF → geodetic → ECEF must return to `p` to round-off: a few ulps of the
/// largest magnitude involved, which near the center is the semi-major axis.
fn assert_ecef_round_trip(p: Ecef, crs: Crs) {
    let back = Ecef::try_from_coordinate(p.try_to_coordinate(crs).unwrap()).unwrap();
    let scale = p.x.abs().max(p.y.abs()).max(p.z.abs()).max(WGS84_A);
    let err = distance_m(back, p);
    assert!(err <= 8.0 * f64::EPSILON * scale, "{p:?}: {err} m");
}

#[test]
fn forward_reference_points() {
    let at = |lat, lon| Ecef::try_from_coordinate(Coordinate::wgs84(lat, lon).unwrap()).unwrap();
    let p = at(0.0, 0.0);
    assert_close(p.x, WGS84_A, 1e-3);
    assert_close(p.y, 0.0, 1e-3);
    assert_close(p.z, 0.0, 1e-3);
    let p = at(0.0, 90.0);
    assert_close(p.x, 0.0, 1e-3);
    assert_close(p.y, WGS84_A, 1e-3);
    let p = at(90.0, 0.0);
    assert_close(p.x, 0.0, 1e-3);
    assert_close(p.z, WGS84_B, 1e-3);
}

/// C15: each datum is placed on its own ellipsoid, so a NAD27 position is on
/// Clarke 1866 and lands about 70 m from where WGS 84 would put the same
/// latitude and longitude.
#[test]
fn each_datum_is_placed_on_its_own_ellipsoid() {
    for (crs, e) in [
        (Crs::Wgs84, Ellipsoid::WGS84),
        (Crs::Nad27, Ellipsoid::CLARKE_1866),
        (Crs::Tokyo, Ellipsoid::BESSEL_1841),
        (Crs::Pulkovo42, Ellipsoid::KRASOVSKY_1940),
    ] {
        let equator = Ecef::try_from_coordinate(Coordinate::new(0.0, 0.0, crs).unwrap()).unwrap();
        assert_eq!(equator, Ecef::new(e.semi_major_m(), 0.0, 0.0), "{crs:?}");
        let pole = Ecef::try_from_coordinate(Coordinate::new(90.0, 0.0, crs).unwrap()).unwrap();
        assert_close(pole.z, e.semi_minor_m(), 1e-6);
        let p = Ecef::try_from_coordinate(at(40.0, -100.0, 250.0, crs)).unwrap();
        let back = p.try_to_coordinate(crs).unwrap();
        assert_eq!(back.crs(), crs);
        assert_close(back.lat(), 40.0, 1e-12);
        assert_close(back.lon(), -100.0, 1e-12);
        assert_close(ellipsoidal_height(back), 250.0, 1e-8);
    }
    let nad27 = Ecef::try_from_coordinate(at(40.0, -100.0, 0.0, Crs::Nad27)).unwrap();
    let wgs84 = Ecef::try_from_coordinate(at(40.0, -100.0, 0.0, Crs::Wgs84)).unwrap();
    assert!(distance_m(nad27, wgs84) > 50.0);
}

#[test]
fn round_trip_geodetic() {
    for (lat, lon, h) in [
        (40.7128, -74.006, 100.0),
        (-33.8688, 151.2093, 0.0),
        (51.5074, -0.1278, 1500.0),
        (90.0, 0.0, 0.0),   // north pole
        (-90.0, 0.0, 10.0), // south pole
        (-89.9, 179.9, 50.0),
        (0.0, -180.0, -100.0),
    ] {
        let back = Ecef::try_from_coordinate(at(lat, lon, h, Crs::Wgs84))
            .unwrap()
            .try_to_coordinate(Crs::Wgs84)
            .unwrap();
        assert_close(back.lat(), lat, 1e-12);
        // Longitude is undefined at the exact pole; skip it there.
        if lat.abs() < 90.0 {
            assert_close(back.lon(), lon, 1e-12);
        }
        assert_close(ellipsoidal_height(back), h, 1e-8);
    }
}

/// C15: the inverse is exact at every altitude, where Bowring's single step
/// lost up to 0.3 m of latitude at GNSS and geostationary heights. A
/// geodetic → ECEF → geodetic round trip measures the inverse alone (the
/// forward is exact): its latitude and height errors are held to eight ulps
/// of the geocentric distance, about 1.1e-8 m on the ground. A flat 1e-9 m
/// is below `f64` resolution: one ulp of a mid-latitude in degrees is already
/// about 1.6e-9 m of ground.
#[test]
fn the_inverse_is_exact_at_every_altitude() {
    for h in [
        -10_000.0,
        0.0,
        10_000.0,
        100_000.0,
        1_000_000.0,
        20_000_000.0,
        36_000_000.0,
    ] {
        let bound = 8.0 * f64::EPSILON * (WGS84_A + h);
        for i in 0..=360 {
            let lat = 0.5 * f64::from(i) - 90.0;
            let back = Ecef::try_from_coordinate(at(lat, 10.0, h, Crs::Wgs84))
                .unwrap()
                .try_to_coordinate(Crs::Wgs84)
                .unwrap();
            let lat_err = (back.lat() - lat).abs().to_radians() * (WGS84_A + h);
            assert!(lat_err <= bound, "h {h}, lat {lat}: {lat_err} m");
            let h_err = (ellipsoidal_height(back) - h).abs();
            assert!(h_err <= bound, "h {h}, lat {lat}: height off {h_err} m");
        }
    }
}

/// Deep inside the Earth the inverse takes its other branches: inside the
/// evolute (the three-real-root case), a negative `u`, and the equatorial
/// plane where the general formulas are 0/0. Each still round-trips.
#[test]
fn the_inverse_round_trips_near_the_center() {
    for x in [0.0, 1.0, 1_000.0, 10_000.0, 30_000.0, 42_000.0, 60_000.0] {
        for z in [-30_000.0, -10_000.0, -1.0, 1.0, 1_000.0, 20_000.0, 30_000.0] {
            assert_ecef_round_trip(Ecef::new(x, 0.7 * x, z), Crs::Wgs84);
        }
    }
    for x in [1_000.0, 20_000.0, 1_000_000.0] {
        assert_ecef_round_trip(Ecef::new(x, 0.0, 0.0), Crs::Nad27);
    }
}

/// On the equatorial plane inside the evolute the nearest surface point is
/// off the equator, so the latitude follows the sign of `z`, even of a `z` too
/// small to register in the general formulas.
#[test]
fn the_equatorial_interior_takes_the_side_of_z() {
    let lat_of = |z| {
        Ecef::new(10_000.0, 0.0, z)
            .try_to_coordinate(Crs::Wgs84)
            .unwrap()
            .lat()
    };
    let north = lat_of(1e-200);
    let south = lat_of(-1e-200);
    assert!(north > 70.0, "{north}");
    assert_close(south, -north, 0.0);
    assert_close(lat_of(0.0), north, 0.0);
    // The center itself maps to a pole, at a height of minus the semi-minor
    // axis.
    let center = Ecef::new(0.0, 0.0, 0.0)
        .try_to_coordinate(Crs::Wgs84)
        .unwrap();
    assert_close(center.lat(), 90.0, 0.0);
    assert_close(ellipsoidal_height(center), -WGS84_B, 1e-6);
}

/// Far beyond the Earth the height is the distance to the center, and a
/// position whose height overflows is an error rather than an infinite value.
#[test]
fn distant_positions_keep_their_direction() {
    // Below the overflow the closed form answers; above it, the far-field
    // fallback does. Either way the direction and distance hold.
    assert_ecef_round_trip(Ecef::new(1e10, 0.0, 1e10), Crs::Wgs84);
    // From 1e20 m on, geodetic and geocentric latitude agree to 1e-9°.
    for d in [1e20, 1e30, 1e40, 1e45, 1e50, 1e100, 1e200, 1e300] {
        let far = Ecef::new(d, 0.0, d).try_to_coordinate(Crs::Wgs84).unwrap();
        assert_close(far.lat(), 45.0, 1e-9);
        assert_close(far.lon(), 0.0, 0.0);
        // The height is the distance less about the Earth's radius at 45°.
        let expected = 2f64.sqrt() * d - 6_367_490.0;
        assert_close(ellipsoidal_height(far), expected, 1e-12 * d + 10.0);
    }
    assert!(matches!(
        Ecef::new(f64::MAX, f64::MAX, 0.0).try_to_coordinate(Crs::Wgs84),
        Err(Error::InvalidValue {
            field: "height",
            ..
        })
    ));
}

#[test]
fn china_systems_have_no_ecef_position() {
    for crs in [Crs::Gcj02, Crs::Bd09] {
        let mismatch = Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: crs,
        });
        let c = Coordinate::new(39.9, 116.4, crs).unwrap();
        assert_eq!(
            Ecef::try_from_coordinate(c),
            mismatch.clone().map(|_: ()| Ecef::new(0.0, 0.0, 0.0))
        );
        assert_eq!(
            Ecef::new(WGS84_A, 0.0, 0.0).try_to_coordinate(crs),
            mismatch.map(|_: ()| c)
        );
    }
}

#[test]
fn invalid_inputs_and_orthometric_heights_are_rejected() {
    let orthometric = Coordinate::wgs84(0.0, 0.0)
        .unwrap()
        .with_height(Height::Orthometric(1.0))
        .unwrap();
    assert!(matches!(
        Ecef::try_from_coordinate(orthometric),
        Err(Error::InvalidValue {
            field: "height",
            ..
        })
    ));
    for p in [
        Ecef::new(f64::NAN, 0.0, 0.0),
        Ecef::new(0.0, f64::INFINITY, 0.0),
        Ecef::new(0.0, 0.0, f64::NEG_INFINITY),
    ] {
        assert!(matches!(
            p.try_to_coordinate(Crs::Wgs84),
            Err(Error::InvalidValue { field: "ECEF", .. })
        ));
    }
}
