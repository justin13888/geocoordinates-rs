use super::*;

// ----- TEST-003: every entry point validates both positions -----

#[test]
fn measurements_reject_an_out_of_range_position_on_either_side() {
    let ok = c(0.0, 0.0);
    for (bad, lat, lon) in [
        (c(91.0, 0.0), 91.0, 0.0),
        (c(-90.5, 0.0), -90.5, 0.0),
        (c(0.0, 181.0), 0.0, 181.0),
        (c(0.0, -180.5), 0.0, -180.5),
    ] {
        assert_out_of_range(geodesic_distance(&bad, &ok), lat, lon);
        assert_out_of_range(geodesic_distance(&ok, &bad), lat, lon);
        assert_out_of_range(haversine_distance(&bad, &ok), lat, lon);
        assert_out_of_range(haversine_distance(&ok, &bad), lat, lon);
        assert_out_of_range(initial_bearing(&bad, &ok), lat, lon);
        assert_out_of_range(initial_bearing(&ok, &bad), lat, lon);
        assert_out_of_range(final_bearing(&bad, &ok), lat, lon);
        assert_out_of_range(final_bearing(&ok, &bad), lat, lon);
        assert_out_of_range(rhumb_distance(&bad, &ok), lat, lon);
        assert_out_of_range(rhumb_distance(&ok, &bad), lat, lon);
        assert_out_of_range(rhumb_bearing(&bad, &ok), lat, lon);
        assert_out_of_range(rhumb_bearing(&ok, &bad), lat, lon);
        let end = c(0.0, 10.0);
        assert_out_of_range(cross_track_distance(&bad, &ok, &end), lat, lon);
        assert_out_of_range(cross_track_distance(&end, &ok, &bad), lat, lon);
        assert_out_of_range(along_track_distance(&bad, &ok, &end), lat, lon);
        assert_out_of_range(along_track_distance(&end, &ok, &bad), lat, lon);
    }
    // NaN components are out of range too (the range check is not `!(x > 90)`).
    assert!(matches!(
        geodesic_distance(&c(f64::NAN, 0.0), &ok),
        Err(Error::OutOfRange { .. })
    ));
}

#[test]
fn producers_reject_an_out_of_range_position() {
    let one_km = Length::from_meters(1_000.0);
    let bad = c(91.0, 0.0);
    assert_out_of_range(destination(&bad, 0.0, one_km), 91.0, 0.0);
    assert_out_of_range(rhumb_destination(&bad, 0.0, one_km), 91.0, 0.0);
    assert_out_of_range(midpoint(&bad, &c(0.0, 0.0)), 91.0, 0.0);
    assert_out_of_range(intermediate(&c(0.0, 0.0), &bad, 0.5), 91.0, 0.0);
    assert_out_of_range(intersection(&c(0.0, 0.0), 0.0, &bad, 90.0), 91.0, 0.0);
}

// ----- TEST-004: haversine -----

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
fn ellipsoidal_measurements_across_the_antimeridian_and_antipode() {
    // a·π/180 along the equator, taken the short way across ±180°.
    assert_close(
        geodesic_distance(&c(0.0, 179.5), &c(0.0, -179.5))
            .unwrap()
            .meters(),
        111_319.490_793,
        1e-3,
    );
    assert_close(
        initial_bearing(&c(0.0, 179.5), &c(0.0, -179.5)).unwrap(),
        90.0,
        1e-9,
    );
    // Equatorial antipodes: the geodesic runs over a pole, length 2·(meridian
    // quadrant) on WGS-84.
    assert_close(
        geodesic_distance(&c(0.0, 0.0), &c(0.0, 180.0))
            .unwrap()
            .meters(),
        20_003_931.458_625,
        1e-3,
    );
    // Over the pole: leave due north, arrive due south.
    assert_close(
        initial_bearing(&c(89.0, 0.0), &c(89.0, 180.0)).unwrap(),
        0.0,
        1e-9,
    );
    assert_close(
        final_bearing(&c(89.0, 0.0), &c(89.0, 180.0)).unwrap(),
        180.0,
        1e-9,
    );
}

// ----- TEST-002: producers near the poles and across the antimeridian -----

#[test]
fn destination_over_a_pole_and_across_the_antimeridian() {
    let l = Length::from_meters;
    // 50 km due north from 89.9°N passes over the pole onto the 180° meridian.
    let p = destination(&c(89.9, 0.0), 0.0, l(50_000.0)).unwrap();
    assert_close(p.lat, 89.652_348_254, 1e-8);
    assert_eq!(p.lon.abs(), 180.0);
    let p = destination(&c(89.9, 30.0), 10.0, l(50_000.0)).unwrap();
    assert_close(p.lat, 89.650_397_515, 1e-8);
    assert_close(p.lon, -162.847_011_805, 1e-8);
    // One equatorial degree east from 179.5° wraps to −179.5°.
    let p = destination(&c(0.0, 179.5), 90.0, l(111_319.49)).unwrap();
    assert_close(p.lat, 0.0, 1e-12);
    assert_close(p.lon, -179.5, 1e-7);
    // …and west from −179.9° wraps to the positive side.
    let p = destination(&c(10.0, -179.9), 270.0, l(50_000.0)).unwrap();
    assert_close(p.lat, 9.999_687_606, 1e-8);
    assert_close(p.lon, 179.643_959_703, 1e-8);
}

#[test]
fn midpoint_and_intermediate_across_the_antimeridian_and_over_a_pole() {
    // The short way between 170°E and 170°W crosses 180°.
    let m = midpoint(&c(0.0, 170.0), &c(0.0, -170.0)).unwrap();
    assert_close(m.lat, 0.0, 1e-12);
    assert_close(m.lon.abs(), 180.0, 1e-9);
    let m = midpoint(&c(10.0, 179.0), &c(-10.0, -179.0)).unwrap();
    assert_close(m.lat, 0.0, 1e-9);
    assert_close(m.lon.abs(), 180.0, 1e-9);
    let q = intermediate(&c(0.0, 170.0), &c(0.0, -170.0), 0.75).unwrap();
    assert_close(q.lat, 0.0, 1e-12);
    assert_close(q.lon, -175.0, 1e-9);
    // Two points on opposite meridians at 80°N: the geodesic runs over the
    // north pole, so the midpoint is the pole and a quarter is still at lon 0.
    let m = midpoint(&c(80.0, 0.0), &c(80.0, 180.0)).unwrap();
    assert_close(m.lat, 90.0, 1e-9);
    let q = intermediate(&c(80.0, 0.0), &c(80.0, 180.0), 0.25).unwrap();
    assert_close(q.lat, 85.000_381_965, 1e-8);
    assert_close(q.lon, 0.0, 1e-9);
    // …and symmetrically over the south pole.
    let m = midpoint(&c(-89.0, 45.0), &c(-89.0, -135.0)).unwrap();
    assert_close(m.lat, -90.0, 1e-9);
    // The endpoints of `intermediate` are the inputs.
    let (a, b) = (c(10.0, 20.0), c(30.0, 40.0));
    let p0 = intermediate(&a, &b, 0.0).unwrap();
    let p1 = intermediate(&a, &b, 1.0).unwrap();
    assert_close(p0.lat, 10.0, 1e-12);
    assert_close(p0.lon, 20.0, 1e-12);
    assert_close(p1.lat, 30.0, 1e-12);
    assert_close(p1.lon, 40.0, 1e-12);
}

#[test]
fn intersection_through_a_pole_returns_the_start() {
    // KNOWN WRONG (C1, flipped by #113): the meridian south from the north pole
    // along 10°E meets the equator at (0, 10), but the `phi1.cos()` division
    // degenerates at the pole and the start point (90, 10) comes back as `Ok`.
    let x = intersection(&c(90.0, 10.0), 180.0, &c(0.0, 0.0), 90.0)
        .unwrap()
        .expect("an intersection is returned");
    assert_close(x.lat, 90.0, 1e-9);
    assert_close(x.lon, 10.0, 1e-9);
}

#[test]
fn intersection_across_the_antimeridian() {
    // The equator east from 170°E meets the 175°W meridian north from 10°S.
    let x = intersection(&c(0.0, 170.0), 90.0, &c(-10.0, -175.0), 0.0)
        .unwrap()
        .expect("intersection exists");
    assert_close(x.lat, 0.0, 1e-9);
    assert_close(x.lon, -175.0, 1e-9);
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

// ----- TEST-005: rhumb boundaries -----

#[test]
fn rhumb_across_the_antimeridian_takes_the_short_way() {
    let (a, b) = (c(10.0, 179.5), c(10.0, -179.5));
    assert_close(rhumb_bearing(&a, &b).unwrap(), 90.0, 1e-9);
    // One degree of longitude along the 10° parallel on the sphere.
    let expected = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians() * 10.0_f64.to_radians().cos();
    assert_close(rhumb_distance(&a, &b).unwrap().meters(), expected, 1e-6);
    let d = rhumb_destination(&a, 90.0, Length::from_meters(110_000.0)).unwrap();
    assert_close(d.lat, 10.0, 1e-12);
    assert_close(d.lon, -179.495_486_787, 1e-8);
}

#[test]
fn rhumb_cardinal_bearings_and_meridian_distance() {
    assert_close(
        rhumb_bearing(&c(10.0, 0.0), &c(0.0, 0.0)).unwrap(),
        180.0,
        1e-9,
    );
    assert_close(
        rhumb_bearing(&c(0.0, 10.0), &c(0.0, 0.0)).unwrap(),
        270.0,
        1e-9,
    );
    // Pole to pole along a meridian is half a great circle.
    assert_close(
        rhumb_distance(&c(-90.0, 0.0), &c(90.0, 0.0))
            .unwrap()
            .meters(),
        MEAN_EARTH_RADIUS_M * PI,
        1e-6,
    );
}

#[test]
fn rhumb_distance_branches_at_the_stretched_latitude_threshold() {
    // |Δψ| ≤ 1e-12: the east-west branch, q = cos φ₁ — one degree of the 40°
    // parallel even though the latitudes differ by 1e-13°.
    let parallel = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians() * 40.0_f64.to_radians().cos();
    assert_close(
        rhumb_distance(&c(40.0, 0.0), &c(40.0 + 1e-13, 1.0))
            .unwrap()
            .meters(),
        parallel,
        1e-6,
    );
    // |Δψ| > 1e-12: the ratio branch, q = Δφ/Δψ, pinned on a long oblique leg.
    assert_close(
        rhumb_distance(&c(40.0, -75.0), &c(51.5, 0.0))
            .unwrap()
            .meters(),
        5_928_370.877_680,
        1e-3,
    );
    assert_close(
        rhumb_bearing(&c(40.0, -75.0), &c(51.5, 0.0)).unwrap(),
        77.543_463_128,
        1e-8,
    );
}

#[test]
fn rhumb_destination_due_north_onto_the_pole_keeps_its_meridian() {
    // Ten degrees of arc due north from 80°N lands on the pole without
    // crossing it, so there is no reflection and no 180° longitude flip.
    let ten_deg = Length::from_meters(10.0_f64.to_radians() * MEAN_EARTH_RADIUS_M);
    let p = rhumb_destination(&c(80.0, 0.0), 0.0, ten_deg).unwrap();
    assert_close(p.lat, 90.0, 1e-9);
    assert_eq!(p.lon, 0.0);
}

#[test]
fn rhumb_destination_oblique_past_the_pole_reflects() {
    // KNOWN WRONG (C7, flipped by #115): a non-meridional rhumb line spirals
    // into the pole and never crosses it, but the spherical routine reflects
    // over it as if it were a meridian and returns an arbitrary position.
    let p = rhumb_destination(&c(80.0, 0.0), 45.0, Length::from_meters(2_000_000.0)).unwrap();
    assert_close(p.lat, 87.281_689_447, 1e-8);
    assert_close(p.lon, -105.233_022_646, 1e-8);
    // KNOWN WRONG (C7, flipped by #115): landing on the pole along 45° divides
    // by Δψ → ∞, so the longitude is round-off noise; only its range is pinned.
    let to_pole =
        Length::from_meters(10.0_f64.to_radians() * MEAN_EARTH_RADIUS_M / FRAC_PI_4.cos());
    let p = rhumb_destination(&c(80.0, 0.0), 45.0, to_pole).unwrap();
    assert_close(p.lat, 90.0, 1e-9);
    assert!(p.validate().is_ok(), "{p:?}");
}
