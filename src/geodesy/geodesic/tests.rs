
use super::*;
use crate::test_support::assert_close;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon)
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
        &Coordinate::new(a.0, a.1, Crs::Tokyo),
        &Coordinate::new(b.0, b.1, Crs::Tokyo),
    )
    .unwrap()
    .meters();
    let on_wgs84: f64 = Geodesic::wgs84().inverse(a.0, a.1, b.0, b.1);
    let on_bessel: f64 =
        Geodesic::new(bessel.semi_major_m, bessel.flattening()).inverse(a.0, a.1, b.0, b.1);
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
    assert_close(d.lat, 41.0, 1e-6);
    assert_close(d.lon, -73.0, 1e-6);
}

#[test]
fn destination_rejects_non_wgs84() {
    assert!(
        destination(
            &Coordinate::gcj02(0.0, 0.0),
            90.0,
            Length::from_meters(100_000.0),
        )
        .is_err()
    );
}

#[test]
fn midpoint_and_intermediate() {
    let mid = midpoint(&c(0.0, 0.0), &c(0.0, 2.0)).unwrap();
    assert_close(mid.lat, 0.0, 1e-6);
    assert_close(mid.lon, 1.0, 1e-6);
    assert_close(
        intermediate(&c(0.0, 0.0), &c(0.0, 10.0), 0.25).unwrap().lon,
        2.5,
        1e-6,
    );
}

#[test]
fn rhumb_references() {
    // Along the equator the rhumb line is the equator (q = cos 0 = 1).
    assert_close(
        rhumb_distance(&c(0.0, 0.0), &c(0.0, 1.0)).unwrap().meters(),
        111_195.0,
        1.0,
    );
    assert_close(
        rhumb_bearing(&c(0.0, 0.0), &c(0.0, 10.0)).unwrap(),
        90.0,
        1e-9,
    );
    assert_close(
        rhumb_bearing(&c(0.0, 0.0), &c(10.0, 0.0)).unwrap(),
        0.0,
        1e-9,
    );
}

#[test]
fn rhumb_destination_round_trip() {
    let (start, end) = (c(40.0, -75.0), c(45.0, -70.0));
    let d = rhumb_destination(
        &start,
        rhumb_bearing(&start, &end).unwrap(),
        rhumb_distance(&start, &end).unwrap(),
    )
    .unwrap();
    assert_close(d.lat, 45.0, 1e-6);
    assert_close(d.lon, -70.0, 1e-6);
}

#[test]
fn cross_and_along_track() {
    // Path east along the equator; a point 1° north at lon 5.
    let (start, end, point) = (c(0.0, 0.0), c(0.0, 10.0), c(1.0, 5.0));
    let xt = cross_track_distance(&point, &start, &end).unwrap().meters();
    assert!(xt < 0.0); // north of an eastward path is to the left
    assert_close(xt.abs(), 111_195.0, 2_000.0);
    let at = along_track_distance(&point, &start, &end).unwrap().meters();
    assert_close(at, 5.0 * 111_195.0, 2_000.0); // foot at lon 5
}

#[test]
fn intersection_of_equator_and_meridian() {
    // Equator (east from (0,-10)) meets the prime meridian (north from
    // (-10,0)) at (0, 0); neither endpoint lies on the other path.
    let x = intersection(&c(0.0, -10.0), 90.0, &c(-10.0, 0.0), 0.0)
        .unwrap()
        .expect("intersection exists");
    assert_close(x.lat, 0.0, 1e-7);
    assert_close(x.lon, 0.0, 1e-7);
}

#[test]
fn coincident_great_circles_have_no_intersection() {
    // Both paths run east along the equator — the same great circle.
    assert!(
        intersection(&c(0.0, 0.0), 90.0, &c(0.0, 5.0), 90.0)
            .unwrap()
            .is_none()
    );
}

// ----- Spherical helper references (independent textbook formulas) -----

#[test]
fn shortest_d_lon_is_signed_and_antimeridian_safe() {
    // Plain eastward difference (well under a half-turn).
    assert_close(
        shortest_d_lon(&c(0.0, 0.0), &c(0.0, 10.0)),
        10.0_f64.to_radians(),
        1e-12,
    );
    // Across the antimeridian the short way is +20° east, not −340°.
    assert_close(
        shortest_d_lon(&c(0.0, 170.0), &c(0.0, -170.0)),
        20.0_f64.to_radians(),
        1e-12,
    );
    // …and signed the other way when reversed.
    assert_close(
        shortest_d_lon(&c(0.0, -170.0), &c(0.0, 170.0)),
        -20.0_f64.to_radians(),
        1e-12,
    );
    // Exactly a half-turn is left as +π (the `> PI` guard is strict).
    assert_close(shortest_d_lon(&c(0.0, 0.0), &c(0.0, 180.0)), PI, 1e-12);
}

#[test]
fn stretched_lat_diff_reference() {
    use core::f64::consts::{FRAC_PI_3, FRAC_PI_6};
    // ln(tan(3π/8)) from the equator to 45°.
    assert_close(
        stretched_lat_diff(0.0, FRAC_PI_4),
        0.881_373_587_019_542_9,
        1e-12,
    );
    // A non-zero lower latitude exercises both Mercator terms.
    assert_close(
        stretched_lat_diff(FRAC_PI_6, FRAC_PI_3),
        0.767_651_752_590_761_5,
        1e-12,
    );
    // Identical latitudes stretch to nothing.
    assert_close(stretched_lat_diff(FRAC_PI_6, FRAC_PI_6), 0.0, 1e-12);
}

#[test]
fn spherical_bearing_rad_reference() {
    // Cardinals are exact.
    assert_close(
        spherical_bearing_rad(&c(0.0, 0.0), &c(0.0, 1.0)),
        FRAC_PI_2,
        1e-12,
    ); // east
    assert_close(
        spherical_bearing_rad(&c(0.0, 0.0), &c(1.0, 0.0)),
        0.0,
        1e-12,
    ); // north
    // Off-equator, off-meridian — pins every term of the azimuth formula.
    assert_close(
        spherical_bearing_rad(&c(10.0, 20.0), &c(20.0, 40.0)),
        1.052_036_642_170_326,
        1e-12,
    );
    assert_close(
        spherical_bearing_rad(&c(0.0, 0.0), &c(1.0, 1.0)),
        0.785_322_005_176_158_1,
        1e-12,
    );
}

#[test]
fn angular_distance_rad_reference() {
    // 1° of arc, and a quarter turn to the pole.
    assert_close(
        angular_distance_rad(&c(0.0, 0.0), &c(0.0, 1.0)),
        1.0_f64.to_radians(),
        1e-12,
    );
    assert_close(
        angular_distance_rad(&c(0.0, 0.0), &c(90.0, 0.0)),
        FRAC_PI_2,
        1e-9,
    );
    // A slanted leg pins the haversine cross term.
    assert_close(
        angular_distance_rad(&c(10.0, 20.0), &c(20.0, 40.0)),
        0.379_099_414_611_752_2,
        1e-12,
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
fn rhumb_destination_due_east_holds_latitude() {
    // A due-east rhumb keeps latitude fixed (d_psi → 0, so q = cos φ₁).
    let d = rhumb_destination(&c(40.0, 0.0), 90.0, Length::from_meters(100_000.0)).unwrap();
    assert_close(d.lat, 40.0, 1e-9);
    assert_close(d.lon, 1.173_979_358_250_968, 1e-9);
}

#[test]
fn rhumb_destination_reflects_over_the_poles() {
    // Pushing north past the pole reflects the latitude back down the far side.
    let n = rhumb_destination(&c(80.0, 0.0), 0.0, Length::from_meters(1_668_000.0)).unwrap();
    assert_close(n.lat, 84.999_336_333_074_71, 1e-9);
    assert_close(n.lon, -180.0, 1e-9);
    // …and symmetrically over the south pole (the negative reflection arm).
    let s = rhumb_destination(&c(-80.0, 0.0), 180.0, Length::from_meters(1_668_000.0)).unwrap();
    assert_close(s.lat, -84.999_336_333_074_71, 1e-9);
    assert_close(s.lon, -180.0, 1e-9);
}

#[test]
fn intersection_second_geometry_and_west_branch() {
    // A real great circle (east from (10,0)) meets the λ=10 meridian at a
    // positive latitude — pins the φ₃/λ₃ formulas away from the trivial origin.
    let x = intersection(&c(10.0, 0.0), 90.0, &c(0.0, 10.0), 0.0)
        .unwrap()
        .expect("intersection exists");
    assert_close(x.lat, 9.851_076_116_583_906, 1e-6);
    assert_close(x.lon, 10.0, 1e-6);
    // b west of a (Δλ < 0) takes the other azimuth-assignment branch.
    let w = intersection(&c(0.0, 10.0), 270.0, &c(-10.0, 0.0), 0.0)
        .unwrap()
        .expect("intersection exists");
    assert_close(w.lat, 0.0, 1e-6);
    assert_close(w.lon, 0.0, 1e-6);
}

#[test]
fn intersection_coincident_endpoints_short_circuit() {
    // Identical start points (Δ₁₂ ≈ 0) return that point regardless of bearings.
    let x = intersection(&c(5.0, 5.0), 10.0, &c(5.0, 5.0), 80.0)
        .unwrap()
        .expect("coincident point");
    assert_close(x.lat, 5.0, 1e-12);
    assert_close(x.lon, 5.0, 1e-12);
}

#[test]
fn intersection_behind_a_bearing_is_none() {
    // The crossing of these great circles lies *behind* the westward path, so
    // the forward intersection is the ambiguous antipode → None.
    assert!(
        intersection(&c(10.0, 0.0), 270.0, &c(0.0, 10.0), 0.0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn intersection_with_two_oblique_bearings() {
    // Both paths aim at (15, 25) from off-axis starts, so *both* bearings are
    // oblique (neither cardinal nor zero) — exercises the second-azimuth and
    // angular-excess terms a due-north second path leaves untouched.
    let p = intersection(
        &c(-20.0, 0.0),
        36.954_284_175_619_3,
        &c(40.0, 60.0),
        240.745_319_307_363_2,
    )
    .unwrap()
    .expect("oblique intersection exists");
    assert_close(p.lat, 15.0, 1e-4);
    assert_close(p.lon, 25.0, 1e-4);
}

#[test]
fn intersection_with_shared_start_meridian() {
    // Both starts sit on λ = 0, so Δλ — and `Δλ.sin()` — is exactly zero: the
    // boundary of the east/west azimuth-assignment split. Asymmetric bearings
    // (60° from the equator, 150° from 30°N) make the two assignment branches
    // diverge, so the forward crossing ≈(7.19°, 12.63°) pins the correct one.
    let p = intersection(&c(0.0, 0.0), 60.0, &c(30.0, 0.0), 150.0)
        .unwrap()
        .expect("crossing exists");
    assert_close(p.lat, 7.192_933_677_582_753_5, 1e-4);
    assert_close(p.lon, 12.626_340_872_961_284, 1e-4);
}

#[test]
fn public_geodesics_reject_mixed_crs_and_invalid_parameters() {
    assert!(geodesic_distance(&Coordinate::wgs84(0.0, 0.0), &Coordinate::gcj02(0.0, 1.0)).is_err());
    assert!(intermediate(&c(0.0, 0.0), &c(0.0, 1.0), 1.1).is_err());
    assert!(destination(&c(0.0, 0.0), f64::NAN, Length::from_meters(1.0)).is_err());
    assert!(rhumb_destination(&c(0.0, 0.0), 0.0, Length::from_meters(-1.0)).is_err());
}
