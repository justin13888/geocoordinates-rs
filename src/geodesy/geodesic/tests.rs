use super::*;
use crate::test_support::assert_close;

/// Characterization of validation, poles and the antimeridian.
mod edges;

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
fn geodesic_distance_measures_other_datums_on_wgs84() {
    // Two Tokyo-datum points are measured on WGS-84, not on their own
    // Bessel 1841 ellipsoid, so the result differs from the Bessel geodesic
    // by the documented relative error (up to about 1.3e-4).
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
    assert_eq!(tokyo, on_wgs84);
    let rel = ((tokyo - on_bessel) / on_bessel).abs();
    assert!((1e-5..1.3e-4).contains(&rel), "relative error {rel}");
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
fn destination_rejects_non_wgs84() {
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
        intersection(&c(0.0, 0.0), f64::NAN, &c(1.0, 1.0), 0.0),
        "bearing",
    );
    assert_invalid_value(
        intersection(&c(0.0, 0.0), 0.0, &c(1.0, 1.0), f64::INFINITY),
        "bearing",
    );
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
