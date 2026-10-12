use super::*;
use crate::test_support::assert_close;

/// Characterization of validation, poles and the antimeridian.
mod edges;
/// Property: every producer's output validates.
mod producers;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

#[test]
fn geodesic_distance_equator_reference() {
    // 1° along the equator on WGS-84 (a · π/180).
    assert_close(
        geodesic_distance(&c(0.0, 0.0), &c(0.0, 1.0))
            .unwrap()
            .meters(),
        111_319.49,
        0.5,
    );
}

#[test]
fn geodesic_distance_measures_each_datum_on_its_own_ellipsoid() {
    // C14: two Tokyo-datum points are measured on their own Bessel 1841
    // ellipsoid, not on WGS-84, which would be off by about 1e-4 of the distance.
    let bessel = crate::geodesy::Ellipsoid::BESSEL_1841;
    let (a, b) = ((35.0, 139.0), (36.0, 140.0));
    let tokyo = geodesic_distance(
        &Coordinate::new(a.0, a.1, Crs::Tokyo).unwrap(),
        &Coordinate::new(b.0, b.1, Crs::Tokyo).unwrap(),
    )
    .unwrap()
    .meters();
    let on_wgs84: f64 = Geodesic::wgs84().inverse(a.0, a.1, b.0, b.1);
    let on_bessel: f64 =
        Geodesic::new(bessel.semi_major_m(), bessel.flattening()).inverse(a.0, a.1, b.0, b.1);
    assert_eq!(tokyo, on_bessel);
    let rel = ((on_wgs84 - on_bessel) / on_bessel).abs();
    assert!((1e-5..1.3e-4).contains(&rel), "relative error {rel}");
}

#[test]
fn geodesic_distance_rejects_obfuscated_china_datums() {
    // C14: GCJ-02 / BD-09 positions are not on any ellipsoid.
    for (a, b) in [
        (
            Coordinate::gcj02(39.9, 116.4),
            Coordinate::gcj02(31.2, 121.5),
        ),
        (Coordinate::bd09(39.9, 116.4), Coordinate::bd09(31.2, 121.5)),
    ] {
        let (a, b) = (a.unwrap(), b.unwrap());
        assert!(
            matches!(
                geodesic_distance(&a, &b),
                Err(Error::CrsMismatch { expected: Crs::Wgs84, found }) if found == a.crs()
            ),
            "{a:?}"
        );
    }
}

#[test]
fn bearings_are_cardinal() {
    assert_close(
        initial_bearing(&c(0.0, 0.0), &c(0.0, 1.0)).unwrap(),
        90.0,
        1e-6,
    );
    assert_close(
        initial_bearing(&c(0.0, 0.0), &c(1.0, 0.0)).unwrap(),
        0.0,
        1e-6,
    );
    assert_close(
        final_bearing(&c(0.0, 0.0), &c(1.0, 0.0)).unwrap(),
        0.0,
        1e-6,
    );
}

#[test]
fn destination_inverts_distance_and_bearing() {
    let (start, end) = (c(40.0, -75.0), c(41.0, -73.0));
    let d = destination(
        &start,
        initial_bearing(&start, &end).unwrap(),
        geodesic_distance(&start, &end).unwrap(),
    )
    .unwrap();
    assert_close(d.lat(), 41.0, 1e-6);
    assert_close(d.lon(), -73.0, 1e-6);
}

#[test]
fn destination_rejects_an_obfuscated_datum() {
    assert!(matches!(
        destination(
            &Coordinate::gcj02(0.0, 0.0).unwrap(),
            90.0,
            Length::from_meters(100_000.0),
        ),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
}

#[test]
fn midpoint_and_intermediate() {
    let mid = midpoint(&c(0.0, 0.0), &c(0.0, 2.0)).unwrap();
    assert_close(mid.lat(), 0.0, 1e-6);
    assert_close(mid.lon(), 1.0, 1e-6);
    assert_close(
        intermediate(&c(0.0, 0.0), &c(0.0, 10.0), 0.25)
            .unwrap()
            .lon(),
        2.5,
        1e-6,
    );
}

// ----- Public-surface edge cases the round-trips miss -----

#[test]
fn final_bearing_is_not_constant_zero() {
    // The arrival azimuth on a NE geodesic is distinctly non-zero (≈45.4°),
    // so it cannot be conflated with the due-north `final_bearing == 0`.
    let fb = final_bearing(&c(0.0, 0.0), &c(10.0, 10.0)).unwrap();
    assert!(
        (40.0..50.0).contains(&fb),
        "final bearing {fb} out of expected band"
    );
}

#[test]
fn public_geodesics_reject_mixed_crs_and_invalid_parameters() {
    assert!(matches!(
        geodesic_distance(&c(0.0, 0.0), &Coordinate::gcj02(0.0, 1.0).unwrap()),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
    assert_invalid_value(intermediate(&c(0.0, 0.0), &c(0.0, 1.0), 1.1), "fraction");
    assert_invalid_value(intermediate(&c(0.0, 0.0), &c(0.0, 1.0), -0.1), "fraction");
    assert_invalid_value(
        intermediate(&c(0.0, 0.0), &c(0.0, 1.0), f64::NAN),
        "fraction",
    );
    let one_m = Length::from_meters(1.0);
    assert_invalid_value(destination(&c(0.0, 0.0), f64::NAN, one_m), "bearing");
    assert_invalid_value(
        destination(&c(0.0, 0.0), 0.0, Length::from_meters(f64::INFINITY)),
        "distance",
    );
    assert_invalid_value(
        rhumb_destination(&c(0.0, 0.0), 0.0, Length::from_meters(-1.0)),
        "distance",
    );
    assert_invalid_value(
        rhumb_destination(&c(0.0, 0.0), f64::INFINITY, one_m),
        "bearing",
    );
    assert_invalid_value(
        intersection_spherical(&c(0.0, 0.0), f64::NAN, &c(1.0, 1.0), 0.0),
        "bearing",
    );
    assert_invalid_value(
        intersection_spherical(&c(0.0, 0.0), 0.0, &c(1.0, 1.0), f64::INFINITY),
        "bearing",
    );
}

#[test]
fn coincident_points_have_no_bearing() {
    // The same place, including a pole named at two longitudes.
    for (a, b) in [
        (c(12.0, 34.0), c(12.0, 34.0)),
        (c(90.0, 0.0), c(90.0, 120.0)),
        (c(-90.0, 10.0), c(-90.0, -80.0)),
    ] {
        assert_invalid_value(initial_bearing(&a, &b), "points");
        assert_invalid_value(final_bearing(&a, &b), "points");
    }
    // A distinct point however close is a bearing.
    assert!(initial_bearing(&c(0.0, 0.0), &c(0.0, 1e-9)).is_ok());
}

#[test]
fn bearings_from_a_pole_follow_the_given_meridian() {
    // From (90, 10), (0, 10) lies straight down meridian 10°E (bearing 180°)
    // and (0, -170) down the opposite meridian (bearing 0°).
    assert_close(
        initial_bearing(&c(90.0, 10.0), &c(0.0, 10.0)).unwrap(),
        180.0,
        1e-9,
    );
    assert_close(
        initial_bearing(&c(90.0, 10.0), &c(0.0, -170.0)).unwrap(),
        0.0,
        1e-9,
    );
}

#[test]
fn ellipsoidal_functions_reject_obfuscated_datums() {
    for crs in [Crs::Gcj02, Crs::Bd09] {
        let p = |lat, lon| Coordinate::new(lat, lon, crs).unwrap();
        let (a, b) = (p(30.0, 110.0), p(31.0, 111.0));
        let mismatch = |r: Result<f64>| {
            assert!(
                matches!(r, Err(Error::CrsMismatch { expected: Crs::Wgs84, found }) if found == crs),
                "{crs:?}: {r:?}"
            );
        };
        mismatch(initial_bearing(&a, &b));
        mismatch(final_bearing(&a, &b));
        mismatch(destination(&a, 0.0, Length::from_meters(1.0)).map(|p| p.lat()));
        mismatch(midpoint(&a, &b).map(|p| p.lat()));
        mismatch(intermediate(&a, &b, 0.25).map(|p| p.lat()));
    }
}

#[test]
fn producers_keep_a_classic_datum_and_its_ellipsoid() {
    // A Tokyo-datum destination is computed on Bessel 1841 and stays Tokyo.
    let bessel = crate::geodesy::Ellipsoid::BESSEL_1841;
    let g = Geodesic::new(bessel.semi_major_m(), bessel.flattening());
    let start = Coordinate::new(35.0, 139.0, Crs::Tokyo).unwrap();
    let d = destination(&start, 30.0, Length::from_meters(100_000.0)).unwrap();
    let (lat, lon): (f64, f64) = g.direct(35.0, 139.0, 30.0, 100_000.0);
    assert_eq!((d.lat(), d.crs()), (lat, Crs::Tokyo));
    assert_close(d.lon(), lon, 1e-12);
    // …and a NAD27 midpoint on Clarke 1866 stays NAD27.
    let clarke = crate::geodesy::Ellipsoid::CLARKE_1866;
    let g = Geodesic::new(clarke.semi_major_m(), clarke.flattening());
    let (a, b) = (
        Coordinate::new(40.0, -100.0, Crs::Nad27).unwrap(),
        Coordinate::new(45.0, -90.0, Crs::Nad27).unwrap(),
    );
    let m = midpoint(&a, &b).unwrap();
    let (s12, azi1, _, _): (f64, f64, f64, f64) = g.inverse(40.0, -100.0, 45.0, -90.0);
    let (lat, lon): (f64, f64) = g.direct(40.0, -100.0, azi1, s12 / 2.0);
    assert_eq!((m.lat(), m.crs()), (lat, Crs::Nad27));
    // Wrapping the longitude into [-180, 180) may move it by an ulp.
    assert_close(m.lon(), lon, 1e-12);
    // Bearings on NAD27 are Clarke 1866's too.
    let (azi1, azi2): (f64, f64) = {
        let (_, a1, a2, _): (f64, f64, f64, f64) = g.inverse(40.0, -100.0, 45.0, -90.0);
        (a1, a2)
    };
    assert_eq!(initial_bearing(&a, &b).unwrap(), azi1);
    assert_eq!(final_bearing(&a, &b).unwrap(), azi2);
}

#[test]
fn producers_drop_the_input_height() {
    let start = c(10.0, 20.0)
        .with_height(crate::coord::Height::Ellipsoidal(500.0))
        .unwrap();
    let end = c(11.0, 21.0);
    assert_eq!(
        destination(&start, 0.0, Length::from_meters(10.0))
            .unwrap()
            .height(),
        None
    );
    assert_eq!(midpoint(&start, &end).unwrap().height(), None);
    assert_eq!(intermediate(&start, &end, 0.0).unwrap().height(), None);
}

#[test]
fn finish_wraps_the_longitude_and_validates() {
    let p = finish(10.0, 180.0, Crs::Wgs84).unwrap();
    assert_eq!((p.lat(), p.lon()), (10.0, -180.0));
    let p = finish(-5.0, 190.0, Crs::Tokyo).unwrap();
    assert_close(p.lon(), -170.0, 1e-12);
    assert_eq!(p.crs(), Crs::Tokyo);
    assert_out_of_range(finish(90.5, 0.0, Crs::Wgs84), 90.5, 0.0);
    assert!(finish(f64::NAN, 0.0, Crs::Wgs84).is_err());
}

/// A rejected parameter is `InvalidValue` naming exactly `expected`.
#[track_caller]
fn assert_invalid_value<T: core::fmt::Debug>(r: Result<T>, expected: &str) {
    assert!(
        matches!(&r, Err(Error::InvalidValue { field, .. }) if *field == expected),
        "{r:?}"
    );
}

/// An out-of-range position is `OutOfRange` carrying its own components.
#[track_caller]
fn assert_out_of_range<T: core::fmt::Debug>(r: Result<T>, lat: f64, lon: f64) {
    assert!(
        matches!(&r, Err(Error::OutOfRange { lat: a, lon: o }) if *a == lat && *o == lon),
        "{r:?}"
    );
}
