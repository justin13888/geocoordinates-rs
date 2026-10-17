use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use proptest::prelude::*;

use super::*;
use crate::coord::Crs;
use crate::geodesy::{
    Ellipsoid, cross_track_distance_spherical, destination, final_bearing, geodesic_distance,
};

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn assert_invalid_value<T: core::fmt::Debug>(r: Result<T>, expected: &str) {
    match r {
        Err(Error::InvalidValue { field, .. }) => assert_eq!(field, expected),
        other => panic!("expected InvalidValue({expected}), got {other:?}"),
    }
}

fn crossing(r: Result<GeodesicIntersection>) -> Coordinate {
    match r {
        Ok(GeodesicIntersection::Crossing(p)) => p,
        other => panic!("expected a crossing, got {other:?}"),
    }
}

/// Whether `p` lies on the geodesic leaving `start` on `bearing`, to 1 µm.
fn on_path(p: &Coordinate, start: &Coordinate, bearing: f64) -> bool {
    let ahead = destination(*start, bearing, Length::from_meters(1_000_000.0)).unwrap();
    cross_track_distance(*p, *start, ahead)
        .unwrap()
        .meters()
        .abs()
        < 1e-6
}

// ----- cross-track / along-track -----

#[test]
fn equatorial_path_feet_are_meridian_arcs() {
    // Meridians are geodesics that meet the equator at right angles, so the
    // foot of (lat, lon) on an equatorial path is (0, lon): the cross-track
    // distance is the meridian arc, the along-track distance a · Δλ.
    let (start, end) = (c(0.0, 0.0), c(0.0, 10.0));
    for (lat, lon) in [
        (1.0, 5.0),
        (-30.0, 40.0),
        (60.0, -20.0),
        (-85.0, 170.0),
        (0.0, 3.0),
    ] {
        let p = c(lat, lon);
        let arc = geodesic_distance(c(0.0, lon), p).unwrap().meters();
        let xt = cross_track_distance(p, start, end).unwrap().meters();
        // North of an eastward path is to its left.
        assert!(
            (xt + arc.copysign(lat)).abs() < 1e-6,
            "{lat},{lon}: {xt} vs {arc}"
        );
        let at = along_track_distance(p, start, end).unwrap().meters();
        assert!(
            (at - 6_378_137.0 * lon.to_radians()).abs() < 1e-6,
            "{lat},{lon}: {at}"
        );
    }
}

#[test]
fn the_foot_is_where_the_perpendicular_meets_the_path() {
    let g = Geodesic::wgs84();
    let (start, end) = (c(40.7, -74.0), c(51.5, -0.1));
    let (azi, _, _): (f64, f64, f64) = g.inverse(start.lat(), start.lon(), end.lat(), end.lon());
    for p in [
        c(60.0, -40.0),
        c(30.0, -30.0),
        c(45.0, 20.0),
        c(-10.0, -100.0),
    ] {
        let xt = cross_track_distance(p, start, end).unwrap().meters();
        let at = along_track_distance(p, start, end).unwrap().meters();
        let (lat, lon, foot_azi): (f64, f64, f64) = g.direct(start.lat(), start.lon(), azi, at);
        let (s, azi_p, _, _): (f64, f64, f64, f64) = g.inverse(lat, lon, p.lat(), p.lon());
        assert!((s - xt.abs()).abs() < 1e-6, "{p:?}");
        // A right angle at the foot: the geodesic to the point leaves square.
        let gamma = (azi_p - foot_azi).to_radians();
        assert!(s * gamma.cos().abs() < 1e-6, "{p:?}: {gamma}");
        assert_eq!(xt > 0.0, gamma.sin() > 0.0, "right of the path is positive");
    }
}

#[test]
fn ellipsoidal_and_spherical_cross_track_agree_to_the_flattening() {
    let (start, end) = (c(10.0, 20.0), c(30.0, 50.0));
    let p = c(35.0, 25.0);
    let exact = cross_track_distance(p, start, end).unwrap().meters();
    let sphere = cross_track_distance_spherical(p, start, end)
        .unwrap()
        .meters();
    assert!(
        ((exact - sphere) / exact).abs() < 0.01,
        "{exact} vs {sphere}"
    );
    assert!(exact != sphere);
}

#[test]
fn along_track_is_negative_behind_the_start_and_zero_at_it() {
    let (start, end) = (c(0.0, 0.0), c(0.0, 10.0));
    let behind = along_track_distance(c(2.0, -3.0), start, end).unwrap();
    assert!((behind.meters() + 6_378_137.0 * 3f64.to_radians()).abs() < 1e-6);
    let at_start = along_track_distance(start, start, end).unwrap().meters();
    let xt_start = cross_track_distance(start, start, end).unwrap().meters();
    assert_eq!((at_start, xt_start), (0.0, 0.0));
    assert!(at_start.is_sign_positive() && xt_start.is_sign_positive());
}

#[test]
fn across_the_antimeridian() {
    // A path east from 170° E over the antimeridian; the point is north of 175° W.
    let (start, end) = (c(0.0, 170.0), c(0.0, -170.0));
    let at = along_track_distance(c(5.0, -175.0), start, end).unwrap();
    assert!((at.meters() - 6_378_137.0 * 15f64.to_radians()).abs() < 1e-6);
}

#[test]
fn a_pole_of_an_equatorial_path_has_a_cross_track_but_no_along_track() {
    let (start, end) = (c(0.0, 0.0), c(0.0, 10.0));
    let quarter = geodesic_distance(c(0.0, 0.0), c(90.0, 0.0))
        .unwrap()
        .meters();
    for (pole, sign) in [(c(90.0, 0.0), -1.0), (c(-90.0, 77.0), 1.0)] {
        let xt = cross_track_distance(pole, start, end).unwrap().meters();
        assert!((xt - sign * quarter).abs() < 1e-6, "{pole:?}: {xt}");
        assert_invalid_value(along_track_distance(pole, start, end), "point");
    }
}

#[test]
fn a_point_just_off_the_pole_still_finds_its_meridian() {
    // About 1 cm, 1 m and 100 m from the north pole on the 37° E meridian,
    // the perpendicular to an equatorial path is still that meridian, though
    // the foot is only defined to about 1e-9 m / M (M ≈ 1.6e-9 per cm).
    let (start, end) = (c(0.0, 0.0), c(0.0, 10.0));
    for (lat, tolerance) in [(90.0 - 1e-7, 2.0), (90.0 - 1e-5, 0.02), (90.0 - 1e-3, 2e-4)] {
        let at = along_track_distance(c(lat, 37.0), start, end)
            .unwrap()
            .meters();
        let error = (at - 6_378_137.0 * 37f64.to_radians()).abs();
        assert!(error < tolerance, "{lat}: {at} off by {error}");
    }
}

#[test]
fn a_meridional_path_reaches_over_the_pole() {
    // North up the prime meridian: a point on the far meridian beyond the pole
    // is ahead, past the quarter meridian.
    let (start, end) = (c(0.0, 0.0), c(45.0, 0.0));
    let quarter = geodesic_distance(c(0.0, 0.0), c(90.0, 0.0))
        .unwrap()
        .meters();
    let past = c(80.0, 180.0);
    let beyond = geodesic_distance(c(90.0, 0.0), past).unwrap().meters();
    let at = along_track_distance(past, start, end).unwrap().meters();
    assert!((at - quarter - beyond).abs() < 1e-6, "{at}");
    assert!(
        cross_track_distance(past, start, end)
            .unwrap()
            .meters()
            .abs()
            < 1e-6
    );
}

#[test]
fn degenerate_paths_and_datums_are_rejected() {
    let (p, start) = (c(1.0, 1.0), c(0.0, 0.0));
    for end in [c(0.0, 0.0), c(0.0, 180.0), c(-0.0, -180.0)] {
        assert_invalid_value(cross_track_distance(p, start, end), "path");
        assert_invalid_value(along_track_distance(p, start, end), "path");
    }
    let poles = (c(90.0, 0.0), c(-90.0, 45.0));
    assert_invalid_value(cross_track_distance(p, poles.0, poles.1), "path");

    let tokyo = Coordinate::new(1.0, 1.0, Crs::Tokyo).unwrap();
    for r in [
        cross_track_distance(tokyo, start, c(0.0, 10.0)),
        cross_track_distance(p, start, tokyo),
        along_track_distance(p, tokyo, c(0.0, 10.0)),
    ] {
        assert!(matches!(r, Err(Error::CrsMismatch { .. })), "{r:?}");
    }
    let gcj = |lat, lon| Coordinate::new(lat, lon, Crs::Gcj02).unwrap();
    let r = cross_track_distance(gcj(31.0, 121.0), gcj(30.0, 120.0), gcj(32.0, 122.0));
    assert!(matches!(r, Err(Error::CrsMismatch { .. })), "{r:?}");
}

#[test]
fn each_datum_is_measured_on_its_own_ellipsoid() {
    let bessel = Ellipsoid::BESSEL_1841;
    let tokyo = |lat, lon| Coordinate::new(lat, lon, Crs::Tokyo).unwrap();
    let at = along_track_distance(tokyo(1.0, 5.0), tokyo(0.0, 0.0), tokyo(0.0, 10.0))
        .unwrap()
        .meters();
    assert!((at - bessel.semi_major().meters() * 5f64.to_radians()).abs() < 1e-6);
}

// ----- intersection -----

#[test]
fn an_equator_and_a_meridian_cross_exactly() {
    let p = crossing(intersection(c(0.0, 0.0), 90.0, c(-10.0, 10.0), 0.0));
    assert!(
        p.lat().abs() < 1e-12 && (p.lon() - 10.0).abs() < 1e-12,
        "{p:?}"
    );
    assert_eq!((p.crs(), p.height()), (Crs::Wgs84, None));
    // Across the antimeridian, wrapped into [-180, 180).
    let p = crossing(intersection(c(0.0, 170.0), 90.0, c(10.0, -175.0), 180.0));
    assert!(
        p.lat().abs() < 1e-12 && (p.lon() + 175.0).abs() < 1e-12,
        "{p:?}"
    );
}

#[test]
fn meridians_heading_north_cross_at_the_pole() {
    let p = crossing(intersection(c(0.0, 0.0), 0.0, c(0.0, 30.0), 0.0));
    assert!((p.lat() - 90.0).abs() < 1e-9, "{p:?}");
}

#[test]
fn oblique_crossings_lie_on_both_geodesics() {
    for (a, ba, b, bb) in [
        (c(40.7, -74.0), 45.0, c(38.7, -9.1), 315.0),
        (c(-33.9, 151.2), 100.0, c(-41.3, 174.8), 340.0),
        (c(60.0, 10.0), 200.0, c(50.0, -30.0), 120.0),
        (c(0.0, 0.0), 45.0, c(0.0, 20.0), 315.0),
    ] {
        let p = crossing(intersection(a, ba, b, bb));
        assert!(on_path(&p, &a, ba) && on_path(&p, &b, bb), "{p:?}");
        // The ellipsoid moves the crossing off the sphere's.
        let spherical = crate::geodesy::intersection_spherical(a, ba, b, bb).unwrap();
        let crate::geodesy::Intersection::Crossing(q) = spherical else {
            panic!("{spherical:?}");
        };
        assert!(geodesic_distance(p, q).unwrap().meters() > 1.0);
    }
    // Mirror-image paths cross on their mirror line.
    let p = crossing(intersection(c(0.0, 0.0), 45.0, c(0.0, 20.0), 315.0));
    assert!((p.lon() - 10.0).abs() < 1e-12 && p.lat() > 9.0, "{p:?}");
}

#[test]
fn a_crossing_at_a_start_counts_as_ahead() {
    let p = crossing(intersection(c(0.0, 0.0), 0.0, c(0.0, 30.0), 270.0));
    assert!(p.lat().abs() < 1e-9 && p.lon().abs() < 1e-9, "{p:?}");
}

#[test]
fn paths_along_one_geodesic_are_the_same_geodesic() {
    use GeodesicIntersection::SameGeodesic;
    // The equator, a meridian, and an oblique geodesic, each walked both ways.
    assert_eq!(
        intersection(c(0.0, 0.0), 90.0, c(0.0, 30.0), 270.0),
        Ok(SameGeodesic)
    );
    assert_eq!(
        intersection(c(10.0, 5.0), 0.0, c(50.0, 5.0), 0.0),
        Ok(SameGeodesic)
    );
    let a = c(20.0, 30.0);
    let b = destination(a, 50.0, Length::from_meters(3_000_000.0)).unwrap();
    let onward = final_bearing(a, b).unwrap();
    assert_eq!(intersection(a, 50.0, b, onward), Ok(SameGeodesic));
    assert_eq!(intersection(a, 50.0, b, onward + 180.0), Ok(SameGeodesic));
}

#[test]
fn one_great_circle_is_two_geodesics_on_the_ellipsoid() {
    // b lies on a's great circle, heading along it: the sphere calls these one
    // path, but on the ellipsoid they are distinct geodesics meeting at a
    // shallow angle. Either a crossing on both is found or the angle is
    // reported as too small; never a false SameGeodesic.
    let a = c(20.0, 30.0);
    let b = c(41.0, 70.0);
    let sphere = |p: &Coordinate, q: &Coordinate| {
        let (p1, l1, p2, l2) = (
            p.lat().to_radians(),
            p.lon().to_radians(),
            q.lat().to_radians(),
            q.lon().to_radians(),
        );
        let y = (l2 - l1).sin() * p2.cos();
        let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * (l2 - l1).cos();
        y.atan2(x).to_degrees()
    };
    let (ba, bb) = (sphere(&a, &b), sphere(&b, &a) + 180.0);
    match intersection(a, ba, b, bb) {
        Ok(GeodesicIntersection::Crossing(p)) => {
            assert!(on_path(&p, &a, ba) && on_path(&p, &b, bb))
        }
        Ok(GeodesicIntersection::NoForwardCrossing) => {}
        Err(e) => assert!(
            matches!(e, Error::InvalidValue { field: "paths", .. }),
            "{e:?}"
        ),
        Ok(GeodesicIntersection::SameGeodesic) => panic!("distinct geodesics called one"),
    }
}

#[test]
fn backward_crossings_are_not_forward() {
    use GeodesicIntersection::NoForwardCrossing;
    // Meridians, one north and one south.
    assert_eq!(
        intersection(c(0.0, 0.0), 0.0, c(0.0, 30.0), 180.0),
        Ok(NoForwardCrossing)
    );
    // An equator path east and a meridian path north that it already passed.
    assert_eq!(
        intersection(c(0.0, 20.0), 90.0, c(-10.0, 10.0), 0.0),
        Ok(NoForwardCrossing)
    );
}

#[test]
fn intersection_rejects_bad_bearings_and_datums() {
    let (a, b) = (c(0.0, 0.0), c(0.0, 10.0));
    for (ba, bb) in [(f64::NAN, 0.0), (0.0, f64::INFINITY)] {
        assert_invalid_value(intersection(a, ba, b, bb), "bearing");
    }
    let tokyo = Coordinate::new(0.0, 10.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        intersection(a, 0.0, tokyo, 315.0),
        Err(Error::CrsMismatch { .. })
    ));
    let bd = |lat, lon| Coordinate::new(lat, lon, Crs::Bd09).unwrap();
    let r = intersection(bd(30.0, 120.0), 0.0, bd(30.0, 121.0), 315.0);
    assert!(matches!(r, Err(Error::CrsMismatch { .. })), "{r:?}");
}

#[test]
fn a_classic_datum_crossing_keeps_its_datum() {
    let nad = |lat, lon| Coordinate::new(lat, lon, Crs::Nad27).unwrap();
    let p = crossing(intersection(nad(40.0, -100.0), 90.0, nad(35.0, -90.0), 0.0));
    assert_eq!(p.crs(), Crs::Nad27);
    assert!((p.lon() + 90.0).abs() < 1e-9);
}

#[test]
fn parallel_geodesics_a_millimetre_apart_still_cross() {
    // b sits 1 mm to the right of a's geodesic, heading along it: the two
    // cross about a quarter meridian away at an angle near 1.6e-10 rad. The
    // crossing slides along them with round-off, but lies on both.
    let a = c(20.0, 30.0);
    let on_a = destination(a, 50.0, Length::from_meters(100_000.0)).unwrap();
    let onward = final_bearing(a, on_a).unwrap();
    for offset in [1e-3, 1.0] {
        let b = destination(on_a, onward + 90.0, Length::from_meters(offset)).unwrap();
        let p = crossing(intersection(a, 50.0, b, onward));
        assert!(
            on_path(&p, &a, 50.0) && on_path(&p, &b, onward),
            "{offset}: {p:?}"
        );
    }
}

#[test]
fn an_unrefined_seed_ahead_on_the_sphere_is_an_error() {
    let ahead = (Some((1.0, 2.0, true)), false);
    let behind = (Some((3.0, 4.0, false)), true);
    assert_eq!(first_crossing_ahead([behind, ahead]), Ok(Some((1.0, 2.0))));
    assert_eq!(
        first_crossing_ahead([(None, true), ahead]),
        Ok(Some((1.0, 2.0)))
    );
    assert_eq!(first_crossing_ahead([behind, (None, false)]), Ok(None));
    assert_invalid_value(first_crossing_ahead([(None, true), behind]), "paths");
    assert_invalid_value(first_crossing_ahead([(None, false), (None, true)]), "paths");
}

#[test]
fn a_crossing_far_ahead_is_found() {
    // East along the equator to the 120° E meridian, 13,358 km ahead of a.
    let p = crossing(intersection(c(0.0, 0.0), 90.0, c(-60.0, 120.0), 0.0));
    assert!(
        p.lat().abs() < 1e-9 && (p.lon() - 120.0).abs() < 1e-9,
        "{p:?}"
    );
    // And due west, from a start whose bearing points the other way round.
    let p = crossing(intersection(c(0.0, 0.0), 270.0, c(60.0, -120.0), 180.0));
    assert!(
        p.lat().abs() < 1e-9 && (p.lon() + 120.0).abs() < 1e-9,
        "{p:?}"
    );
}

#[test]
fn a_point_on_the_path_has_a_positive_zero_cross_track() {
    // The geodesic from a point to itself leaves at an arbitrary azimuth, so
    // the sign of zero is set explicitly, whichever way the path runs.
    for end in [c(0.0, 10.0), c(0.0, -10.0), c(10.0, 0.0), c(-10.0, 0.0)] {
        let start = c(0.0, 0.0);
        let xt = cross_track_distance(start, start, end).unwrap().meters();
        assert!(xt == 0.0 && xt.is_sign_positive(), "{end:?}: {xt:?}");
    }
}

// ----- the thresholds, at their boundaries -----

/// The largest f64 below `x` (positive `x`; `next_down` needs Rust 1.86).
fn below(x: f64) -> f64 {
    f64::from_bits(x.to_bits() - 1)
}

#[test]
fn thresholds_hold_at_their_boundaries() {
    assert!(perpendiculars_meet(below(1e-12)) && !perpendiculars_meet(1e-12));
    assert!(negligible(below(1e-6)) && negligible(-below(1e-6)));
    assert!(!negligible(1e-6) && !negligible(-1e-6));
    let round_off = 4.0 * f64::EPSILON;
    assert!(square_to_round_off(round_off) && square_to_round_off(-round_off));
    assert!(!square_to_round_off(f64::from_bits(
        round_off.to_bits() + 1
    )));
    assert!(on_line(below(1e-5), below(1e-10)) && on_line(-below(1e-5), -below(1e-10)));
    assert!(!on_line(1e-5, 0.0) && !on_line(0.0, 1e-10) && !on_line(-1e-5, 0.0));
    // The line 3x + 4y ± 5 = 0 passes 1 from the origin.
    for l in [[3.0, 4.0, 5.0], [-3.0, 4.0, -5.0]] {
        assert!(!passes_within(l, 1.0) && passes_within(l, f64::from_bits(1f64.to_bits() + 1)));
    }
    assert!(is_ahead(-below(1e-5), below(1e-6)) && is_ahead(1e7, 0.0));
    assert!(!is_ahead(-1e-5, 0.0) && !is_ahead(1e7, 1e-6));
}

// ----- the gnomonic projection -----

#[test]
fn the_gnomonic_projection_is_exact_on_a_sphere() {
    // With no flattening, a point σ from the centre projects to radius a·tan σ
    // along its azimuth, and the read-back inverts it exactly.
    let a = 6_371_000.0;
    let sphere = Geodesic::new(a, 0.0);
    for (azi, sigma) in [(90.0_f64, 30.0_f64), (0.0, 60.0), (225.0, 1.0)] {
        let (lat, lon): (f64, f64) = sphere.direct(0.0, 0.0, azi, a * sigma.to_radians());
        let (x, y) = gnomonic_forward(&sphere, 0.0, 0.0, (lat, lon)).unwrap();
        let rho = a * sigma.to_radians().tan();
        let (sin, cos) = azi.to_radians().sin_cos();
        assert!(
            (x - rho * sin).abs() < 1e-6 && (y - rho * cos).abs() < 1e-6,
            "{azi},{sigma}"
        );
        let back = gnomonic_back(&sphere, 0.0, 0.0, x, y);
        let miss: f64 = sphere.inverse(back.0, back.1, lat, lon);
        assert!(miss < 1e-6, "{azi},{sigma}: {miss}");
    }
    // A point beyond the projection's hemisphere has no image.
    let wgs84 = Geodesic::wgs84();
    assert!(gnomonic_forward(&wgs84, 35.0, 139.0, (-35.0, -41.0)).is_none());
    assert!(gnomonic_forward(&wgs84, 0.0, 0.0, (0.0, 90.5)).is_none());
}

#[test]
fn a_crossing_near_the_pole_lies_on_both_paths() {
    // Found by the property below: the crossing, about 7,000 km and 10,000 km
    // along the two paths near the south pole, once stopped 1.03 µm off one.
    let (a, ba) = (c(-22.650585410257776, 68.02949354908361), 174.6008046704343);
    let (b, bb) = (c(0.0, -138.81624026636084), 185.8234302643924);
    let p = crossing(intersection(a, ba, b, bb));
    assert!(on_path(&p, &a, ba) && on_path(&p, &b, bb), "{p:?}");
    assert!(p.lat() < -84.0, "{p:?}");
}

fn latitude() -> impl Strategy<Value = f64> {
    prop_oneof![-89.0..=89.0_f64, Just(0.0)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Every crossing found lies on both geodesics and ahead of both starts.
    #[test]
    fn found_crossings_lie_ahead_on_both_paths(
        (lat_a, lon_a, lat_b, lon_b) in (latitude(), -180.0..180.0_f64, latitude(), -180.0..180.0_f64),
        ba in 0.0..360.0_f64,
        bb in 0.0..360.0_f64,
    ) {
        let (a, b) = (c(lat_a, lon_a), c(lat_b, lon_b));
        if let Ok(GeodesicIntersection::Crossing(p)) = intersection(a, ba, b, bb) {
            prop_assert!(on_path(&p, &a, ba) && on_path(&p, &b, bb), "{p:?}");
            let g = Geodesic::wgs84();
            for (start, bearing) in [(a, ba), (b, bb)] {
                let (s, azi, _, _): (f64, f64, f64, f64) =
                    g.inverse(start.lat(), start.lon(), p.lat(), p.lon());
                prop_assert!(s < 1e-5 || (azi - bearing).to_radians().cos() > 0.0);
            }
        }
    }

    /// The foot is square to the path, at the cross-track distance.
    #[test]
    fn feet_are_square_to_the_path(
        (lat_s, lon_s, lat_e, lon_e) in (latitude(), -180.0..180.0_f64, latitude(), -180.0..180.0_f64),
        (lat_p, lon_p) in (latitude(), -180.0..180.0_f64),
    ) {
        let (start, end, p) = (c(lat_s, lon_s), c(lat_e, lon_e), c(lat_p, lon_p));
        prop_assume!(geodesic_distance(start, end).unwrap().meters() > 1.0);
        if let (Ok(xt), Ok(at)) = (
            cross_track_distance(p, start, end),
            along_track_distance(p, start, end),
        ) {
            let g = Geodesic::wgs84();
            let (azi, _, _): (f64, f64, f64) = g.inverse(lat_s, lon_s, lat_e, lon_e);
            let (lat, lon, foot_azi): (f64, f64, f64) = g.direct(lat_s, lon_s, azi, at.meters());
            let (s, azi_p, _, _): (f64, f64, f64, f64) = g.inverse(lat, lon, lat_p, lon_p);
            prop_assert!((s - xt.meters().abs()).abs() < 1e-6);
            prop_assert!(s * (azi_p - foot_azi).to_radians().cos().abs() < 1e-5);
        }
    }
}
