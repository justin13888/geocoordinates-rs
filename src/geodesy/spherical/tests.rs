use core::f64::consts::FRAC_PI_2;

use super::*;
use crate::test_support::assert_close;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
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
    assert_close(x.lat(), 0.0, 1e-7);
    assert_close(x.lon(), 0.0, 1e-7);
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

#[test]
fn is_coincident_is_strictly_below_the_threshold() {
    assert!(is_coincident(0.0));
    // The largest f64 below 1e-12 (`next_down` needs Rust 1.86; MSRV is 1.85).
    assert!(is_coincident(f64::from_bits(1e-12_f64.to_bits() - 1)));
    assert!(!is_coincident(1e-12));
}

#[test]
fn no_single_forward_intersection_boundaries() {
    // Same great circle: both sines strictly below 1e-12.
    assert!(no_single_forward_intersection(0.0, 0.0));
    assert!(no_single_forward_intersection(-5e-13, 5e-13 * 0.5));
    // Exactly 1e-12 on either side is no longer "the same circle".
    assert!(!no_single_forward_intersection(1e-12, 0.0));
    assert!(!no_single_forward_intersection(0.0, 1e-12));
    // One sine zero and the other not: a single crossing, whatever the sign.
    assert!(!no_single_forward_intersection(0.0, 0.5));
    assert!(!no_single_forward_intersection(0.5, 0.0));
    assert!(!no_single_forward_intersection(-0.5, 0.0));
    // Opposite signs: the ambiguous antipodal crossing.
    assert!(no_single_forward_intersection(-0.5, 0.25));
    assert!(!no_single_forward_intersection(0.5, 0.25));
}

#[test]
fn endpoint_bearings_take_the_west_assignment_on_a_shared_meridian() {
    let (a, b) = (0.25, 0.75);
    let east = (a, 2.0 * PI - b);
    let west = (2.0 * PI - a, b);
    assert_eq!(endpoint_bearings(a, b, 1e-300), east);
    assert_eq!(endpoint_bearings(a, b, -1e-300), west);
    // sin Δλ == 0 (a shared meridian) is not "east".
    assert_eq!(endpoint_bearings(a, b, 0.0), west);
}

#[test]
fn intersection_second_geometry_and_west_branch() {
    // A real great circle (east from (10,0)) meets the λ=10 meridian at a
    // positive latitude — pins the φ₃/λ₃ formulas away from the trivial origin.
    let x = intersection(&c(10.0, 0.0), 90.0, &c(0.0, 10.0), 0.0)
        .unwrap()
        .expect("intersection exists");
    assert_close(x.lat(), 9.851_076_116_583_906, 1e-6);
    assert_close(x.lon(), 10.0, 1e-6);
    // b west of a (Δλ < 0) takes the other azimuth-assignment branch.
    let w = intersection(&c(0.0, 10.0), 270.0, &c(-10.0, 0.0), 0.0)
        .unwrap()
        .expect("intersection exists");
    assert_close(w.lat(), 0.0, 1e-6);
    assert_close(w.lon(), 0.0, 1e-6);
}

#[test]
fn intersection_coincident_endpoints_short_circuit() {
    // Identical start points (Δ₁₂ ≈ 0) return that point regardless of bearings.
    let x = intersection(&c(5.0, 5.0), 10.0, &c(5.0, 5.0), 80.0)
        .unwrap()
        .expect("coincident point");
    assert_close(x.lat(), 5.0, 1e-12);
    assert_close(x.lon(), 5.0, 1e-12);
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
    assert_close(p.lat(), 15.0, 1e-4);
    assert_close(p.lon(), 25.0, 1e-4);
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
    assert_close(p.lat(), 7.192_933_677_582_753_5, 1e-4);
    assert_close(p.lon(), 12.626_340_872_961_284, 1e-4);
}

#[test]
fn haversine_references() {
    // R·π/180 for one degree of arc on the IUGG mean sphere.
    let one_deg = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians();
    let hav = |a: Coordinate, b: Coordinate| haversine_distance(&a, &b).unwrap().meters();
    assert_close(hav(c(0.0, 0.0), c(0.0, 1.0)), one_deg, 1e-6);
    // Across the antimeridian the short way is 1°, not 359°.
    assert_close(hav(c(0.0, 179.5), c(0.0, -179.5)), one_deg, 1e-6);
    // Antipodal points (and pole to pole) are half the circumference, R·π.
    let half = MEAN_EARTH_RADIUS_M * PI;
    assert_close(hav(c(0.0, 0.0), c(0.0, 180.0)), half, 1e-6);
    assert_close(hav(c(30.0, 40.0), c(-30.0, -140.0)), half, 1e-6);
    assert_close(hav(c(90.0, 0.0), c(-90.0, 0.0)), half, 1e-6);
    // Coincident points are exactly zero.
    assert_eq!(hav(c(12.0, 34.0), c(12.0, 34.0)), 0.0);
    // A slanted leg: R times the independently pinned angular distance.
    assert_close(
        hav(c(10.0, 20.0), c(20.0, 40.0)),
        MEAN_EARTH_RADIUS_M * 0.379_099_414_611_752_2,
        1e-3,
    );
}

#[test]
fn intersection_through_a_pole_returns_the_start() {
    // KNOWN WRONG (C1, flipped by #113): the meridian south from the north pole
    // along 10°E meets the equator at (0, 10), but the `phi1.cos()` division
    // degenerates at the pole and the start point (90, 10) comes back as `Ok`.
    let x = intersection(&c(90.0, 10.0), 180.0, &c(0.0, 0.0), 90.0)
        .unwrap()
        .expect("an intersection is returned");
    assert_close(x.lat(), 90.0, 1e-9);
    assert_close(x.lon(), 10.0, 1e-9);
}

#[test]
fn intersection_across_the_antimeridian() {
    // The equator east from 170°E meets the 175°W meridian north from 10°S.
    let x = intersection(&c(0.0, 170.0), 90.0, &c(-10.0, -175.0), 0.0)
        .unwrap()
        .expect("intersection exists");
    assert_close(x.lat(), 0.0, 1e-9);
    assert_close(x.lon(), -175.0, 1e-9);
}

#[test]
fn cross_and_along_track_across_the_antimeridian() {
    // Path east along the equator from 175°E to 175°W; a point 1° north at 180°.
    let (start, end, point) = (c(0.0, 175.0), c(0.0, -175.0), c(1.0, 180.0));
    let one_deg = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians();
    let xt = cross_track_distance(&point, &start, &end).unwrap().meters();
    assert_close(xt, -one_deg, 1e-3);
    let at = along_track_distance(&point, &start, &end).unwrap().meters();
    assert_close(at, 5.0 * one_deg, 1e-3);
    // A foot behind the start is negative.
    let behind = along_track_distance(&c(1.0, -5.0), &c(0.0, 0.0), &c(0.0, 10.0))
        .unwrap()
        .meters();
    assert_close(behind, -5.0 * one_deg, 1e-3);
}
