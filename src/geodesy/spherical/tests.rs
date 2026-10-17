use core::f64::consts::FRAC_PI_2;

use super::*;
use crate::test_support::assert_close;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

/// The crossing point of an [`Intersection`], if it has one.
trait CrossingPoint {
    fn crossing(self) -> Option<Coordinate>;
}

impl CrossingPoint for Intersection {
    fn crossing(self) -> Option<Coordinate> {
        match self {
            Intersection::Crossing(p) => Some(p),
            Intersection::SameGreatCircle | Intersection::NoForwardCrossing => None,
        }
    }
}

#[test]
fn cross_and_along_track() {
    // Path east along the equator; a point 1° north at lon 5.
    let (start, end, point) = (c(0.0, 0.0), c(0.0, 10.0), c(1.0, 5.0));
    let xt = cross_track_distance_spherical(point, start, end)
        .unwrap()
        .meters();
    assert!(xt < 0.0); // north of an eastward path is to the left
    assert_close(xt.abs(), 111_195.0, 2_000.0);
    let at = along_track_distance_spherical(point, start, end)
        .unwrap()
        .meters();
    assert_close(at, 5.0 * 111_195.0, 2_000.0); // foot at lon 5
}

#[test]
fn intersection_of_equator_and_meridian() {
    // Equator (east from (0,-10)) meets the prime meridian (north from
    // (-10,0)) at (0, 0); neither endpoint lies on the other path.
    let x = intersection_spherical(c(0.0, -10.0), 90.0, c(-10.0, 0.0), 0.0)
        .unwrap()
        .crossing()
        .expect("intersection exists");
    assert_close(x.lat(), 0.0, 1e-7);
    assert_close(x.lon(), 0.0, 1e-7);
}

#[test]
fn coincident_great_circles_have_no_intersection() {
    // Both paths run east along the equator — the same great circle — and so
    // do two that run along it in opposite directions.
    assert_eq!(
        intersection_spherical(c(0.0, 0.0), 90.0, c(0.0, 5.0), 90.0).unwrap(),
        Intersection::SameGreatCircle
    );
    assert_eq!(
        intersection_spherical(c(0.0, 0.0), 90.0, c(0.0, 5.0), 270.0).unwrap(),
        Intersection::SameGreatCircle
    );
    // One start and one bearing twice over is one great circle too.
    assert_eq!(
        intersection_spherical(c(5.0, 5.0), 30.0, c(5.0, 5.0), 30.0).unwrap(),
        Intersection::SameGreatCircle
    );
}

#[test]
fn spherical_bearing_rad_reference() {
    // Cardinals are exact.
    assert_close(
        spherical_bearing_rad(c(0.0, 0.0), c(0.0, 1.0)),
        FRAC_PI_2,
        1e-12,
    ); // east
    assert_close(spherical_bearing_rad(c(0.0, 0.0), c(1.0, 0.0)), 0.0, 1e-12); // north
    // Off-equator, off-meridian — pins every term of the azimuth formula.
    assert_close(
        spherical_bearing_rad(c(10.0, 20.0), c(20.0, 40.0)),
        1.052_036_642_170_326,
        1e-12,
    );
    assert_close(
        spherical_bearing_rad(c(0.0, 0.0), c(1.0, 1.0)),
        0.785_322_005_176_158_1,
        1e-12,
    );
}

#[test]
fn angular_distance_rad_reference() {
    // 1° of arc, and a quarter turn to the pole.
    assert_close(
        angular_distance_rad(c(0.0, 0.0), c(0.0, 1.0)),
        1.0_f64.to_radians(),
        1e-12,
    );
    assert_close(
        angular_distance_rad(c(0.0, 0.0), c(90.0, 0.0)),
        FRAC_PI_2,
        1e-9,
    );
    // A slanted leg pins the haversine cross term.
    assert_close(
        angular_distance_rad(c(10.0, 20.0), c(20.0, 40.0)),
        0.379_099_414_611_752_2,
        1e-12,
    );
}

#[test]
fn is_parallel_is_strictly_below_the_threshold() {
    assert!(is_parallel(0.0));
    // The largest f64 below 1e-12 (`next_down` needs Rust 1.86; MSRV is 1.85).
    assert!(is_parallel(f64::from_bits(1e-12_f64.to_bits() - 1)));
    assert!(!is_parallel(1e-12));
}

#[test]
fn is_forward_spans_the_start_to_just_short_of_the_antipode() {
    let below = |x: f64| f64::from_bits(x.to_bits() - 1); // next toward zero
    assert!(is_forward(0.0));
    assert!(is_forward(1.0));
    // Rounding slack behind the start is still the start.
    assert!(is_forward(-below(1e-12)));
    assert!(!is_forward(-1e-12));
    assert!(!is_forward(-0.5));
    // The start's antipode is reached by every direction, so it is not ahead.
    assert!(is_forward(below(PI - 1e-12)));
    assert!(!is_forward(PI - 1e-12));
    assert!(!is_forward(PI));
}

#[test]
fn heading_and_n_vector_reference() {
    // At (0, 0): the n-vector is +x, north is +z and east is +y.
    let o = c(0.0, 0.0);
    let v = n_vector(o);
    assert_close(v[0], 1.0, 1e-15);
    assert_close(v[1], 0.0, 1e-15);
    assert_close(v[2], 0.0, 1e-15);
    let north = heading(o, 0.0);
    assert_close(north[2], 1.0, 1e-15);
    let east = heading(o, 90.0);
    assert_close(east[1], 1.0, 1e-15);
    // At (30, 60), heading 45°: pins every term of the north and east axes.
    let d = heading(c(30.0, 60.0), 45.0);
    let (s, h) = (core::f64::consts::FRAC_1_SQRT_2, 0.5);
    assert_close(d[0], s * (-h * 0.5) + s * -(3.0_f64.sqrt() / 2.0), 1e-15);
    assert_close(d[1], s * (-h * 3.0_f64.sqrt() / 2.0) + s * 0.5, 1e-15);
    assert_close(d[2], s * 3.0_f64.sqrt() / 2.0, 1e-15);
    // Each heading is a unit vector perpendicular to its position.
    let p = c(-35.0, 120.0);
    let (pv, dv) = (n_vector(p), heading(p, 200.0));
    assert_close(dot(pv, pv), 1.0, 1e-15);
    assert_close(dot(dv, dv), 1.0, 1e-15);
    assert_close(dot(pv, dv), 0.0, 1e-15);
}

#[test]
fn cross_and_dot_reference() {
    assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
    assert_eq!(cross([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]);
    assert_eq!(cross([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    assert_eq!(cross([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), [-3.0, 6.0, -3.0]);
    assert_eq!(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0);
}

#[test]
fn along_angle_is_signed() {
    // From (0, 0) heading east: (0, 10) is 10° ahead, (0, -10) 10° behind.
    let (p, d) = (n_vector(c(0.0, 0.0)), heading(c(0.0, 0.0), 90.0));
    let ten = 10.0_f64.to_radians();
    assert_close(along_angle(p, d, n_vector(c(0.0, 10.0))), ten, 1e-12);
    assert_close(along_angle(p, d, n_vector(c(0.0, -10.0))), -ten, 1e-12);
    assert_close(
        along_angle(p, d, n_vector(c(0.0, 120.0))),
        12.0 * ten,
        1e-12,
    );
}

#[test]
fn intersection_second_geometry_and_west_branch() {
    // A real great circle (east from (10,0)) meets the λ=10 meridian at a
    // positive latitude — pins the crossing away from the trivial origin.
    let x = intersection_spherical(c(10.0, 0.0), 90.0, c(0.0, 10.0), 0.0)
        .unwrap()
        .crossing()
        .expect("intersection exists");
    assert_close(x.lat(), 9.851_076_116_583_906, 1e-6);
    assert_close(x.lon(), 10.0, 1e-6);
    // b west of a (Δλ < 0).
    let w = intersection_spherical(c(0.0, 10.0), 270.0, c(-10.0, 0.0), 0.0)
        .unwrap()
        .crossing()
        .expect("intersection exists");
    assert_close(w.lat(), 0.0, 1e-6);
    assert_close(w.lon(), 0.0, 1e-6);
}

#[test]
fn intersection_at_a_shared_start_is_that_start_without_its_height() {
    // Identical start points on different bearings cross at that point, which
    // is returned without the input's height.
    let start = c(5.0, 5.0)
        .with_height(crate::coord::Height::Ellipsoidal(120.0))
        .unwrap();
    let x = intersection_spherical(start, 10.0, start, 80.0)
        .unwrap()
        .crossing()
        .expect("coincident point");
    assert_close(x.lat(), 5.0, 1e-12);
    assert_close(x.lon(), 5.0, 1e-12);
    assert_eq!(x.height(), None);
}

#[test]
fn intersection_behind_a_bearing_is_no_forward_crossing() {
    // The crossing of these great circles lies *behind* the westward path, and
    // its antipode beyond half a great circle along the northward one.
    assert_eq!(
        intersection_spherical(c(10.0, 0.0), 270.0, c(0.0, 10.0), 0.0).unwrap(),
        Intersection::NoForwardCrossing
    );
    // Antipodal starts: each crossing is a start, half a circle from the other.
    assert_eq!(
        intersection_spherical(c(0.0, 0.0), 0.0, c(0.0, 180.0), 90.0).unwrap(),
        Intersection::NoForwardCrossing
    );
}

#[test]
fn intersection_with_two_oblique_bearings() {
    // Both paths aim at (15, 25) from off-axis starts, so *both* bearings are
    // oblique (neither cardinal nor zero).
    let p = intersection_spherical(
        c(-20.0, 0.0),
        36.954_284_175_619_3,
        c(40.0, 60.0),
        240.745_319_307_363_2,
    )
    .unwrap()
    .crossing()
    .expect("oblique intersection exists");
    assert_close(p.lat(), 15.0, 1e-4);
    assert_close(p.lon(), 25.0, 1e-4);
}

#[test]
fn intersection_with_shared_start_meridian() {
    // Both starts sit on λ = 0. Asymmetric bearings (60° from the equator,
    // 150° from 30°N) pin the forward crossing ≈(7.19°, 12.63°).
    let p = intersection_spherical(c(0.0, 0.0), 60.0, c(30.0, 0.0), 150.0)
        .unwrap()
        .crossing()
        .expect("crossing exists");
    assert_close(p.lat(), 7.192_933_677_582_753_5, 1e-4);
    assert_close(p.lon(), 12.626_340_872_961_284, 1e-4);
}

/// The haversine bound its doc states: against the WGS-84 geodesic, over a
/// global grid of pairs, the relative error peaks just under 0.56%.
#[test]
fn haversine_relative_error_against_the_geodesic_is_bounded() {
    let mut worst: f64 = 0.0;
    let lats = (-18..=18).map(|k| f64::from(k) * 5.0);
    for lat1 in lats.clone() {
        for lat2 in lats.clone() {
            for dlon in (0..=36).map(|k| f64::from(k) * 5.0) {
                let (a, b) = (c(lat1, 0.0), c(lat2, dlon));
                let geo = crate::geodesy::geodesic_distance(a, b).unwrap().meters();
                if geo > 1_000.0 {
                    let hav = haversine_distance(a, b).unwrap().meters();
                    worst = worst.max(((hav - geo) / geo).abs());
                }
            }
        }
    }
    assert!((0.0055..0.0056).contains(&worst), "{worst}");
}

#[test]
fn haversine_references() {
    // R·π/180 for one degree of arc on the IUGG mean sphere.
    let one_deg = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians();
    let hav = |a: Coordinate, b: Coordinate| haversine_distance(a, b).unwrap().meters();
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
fn intersection_from_a_pole_follows_the_given_meridian() {
    // C1: the meridian south from the north pole along 10°E meets the equator
    // at (0, 10). At a pole the bearing is taken against the meridian of the
    // given longitude, so 180° from (90, 10) runs down 10°E.
    let x = intersection_spherical(c(90.0, 10.0), 180.0, c(0.0, 0.0), 90.0)
        .unwrap()
        .crossing()
        .expect("an intersection is returned");
    assert_close(x.lat(), 0.0, 1e-9);
    assert_close(x.lon(), 10.0, 1e-9);
    // Bearing 0° from the pole runs down the opposite meridian, 170°W.
    let x = intersection_spherical(c(90.0, 10.0), 0.0, c(0.0, -175.0), 90.0)
        .unwrap()
        .crossing()
        .expect("an intersection is returned");
    assert_close(x.lat(), 0.0, 1e-9);
    assert_close(x.lon(), -170.0, 1e-9);
    // From the south pole, 0° runs up the given meridian.
    let x = intersection_spherical(c(-90.0, -60.0), 0.0, c(0.0, -70.0), 90.0)
        .unwrap()
        .crossing()
        .expect("an intersection is returned");
    assert_close(x.lat(), 0.0, 1e-9);
    assert_close(x.lon(), -60.0, 1e-9);
}

#[test]
fn intersection_at_a_pole() {
    // Two meridians leaving the equator northward cross at the north pole.
    let x = intersection_spherical(c(0.0, 20.0), 0.0, c(0.0, 100.0), 0.0)
        .unwrap()
        .crossing()
        .expect("the meridians meet at the pole");
    assert_close(x.lat(), 90.0, 1e-9);
}

#[test]
fn intersection_rejects_obfuscated_datums_and_keeps_classic_ones() {
    let gcj = Coordinate::gcj02(30.0, 110.0).unwrap();
    assert!(matches!(
        intersection_spherical(gcj, 90.0, gcj, 0.0),
        Err(Error::CrsMismatch {
            expected: crate::Crs::Wgs84,
            found: crate::Crs::Gcj02,
        })
    ));
    let tokyo = |lat, lon| Coordinate::new(lat, lon, crate::Crs::Tokyo).unwrap();
    let x = intersection_spherical(tokyo(0.0, -10.0), 90.0, tokyo(-10.0, 0.0), 0.0)
        .unwrap()
        .crossing()
        .expect("intersection exists");
    assert_eq!(x.crs(), crate::Crs::Tokyo);
    assert_close(x.lon(), 0.0, 1e-9);
}

#[test]
fn cross_and_along_track_reject_a_degenerate_path() {
    let point = c(1.0, 5.0);
    for (start, end) in [
        (c(0.0, 0.0), c(0.0, 0.0)),    // coincident
        (c(90.0, 0.0), c(90.0, 45.0)), // one pole, two longitudes
        (c(0.0, 0.0), c(0.0, 180.0)),  // antipodal
        (c(30.0, 40.0), c(-30.0, -140.0)),
    ] {
        for r in [
            cross_track_distance_spherical(point, start, end),
            along_track_distance_spherical(point, start, end),
        ] {
            assert!(
                matches!(&r, Err(Error::InvalidValue { field: "path", .. })),
                "{start:?} → {end:?}: {r:?}"
            );
        }
    }
}

#[test]
fn along_track_rejects_a_pole_of_the_path() {
    // Each path's great circle has the two listed points as its poles.
    for (start, end, poles) in [
        (c(0.0, 0.0), c(0.0, 10.0), [c(90.0, 0.0), c(-90.0, 123.0)]),
        (c(0.0, 0.0), c(10.0, 0.0), [c(0.0, -90.0), c(0.0, 90.0)]),
        (
            c(0.0, 170.0),
            c(0.0, -170.0),
            [c(90.0, 45.0), c(-90.0, 0.0)],
        ),
    ] {
        for pole in poles {
            let r = along_track_distance_spherical(pole, start, end);
            assert!(
                matches!(&r, Err(Error::InvalidValue { field: "point", .. })),
                "{pole:?} on {start:?} → {end:?}: {r:?}"
            );
            // The cross-track distance there is still defined: a quarter circle.
            let xt = cross_track_distance_spherical(pole, start, end).unwrap();
            assert_close(xt.meters().abs(), MEAN_EARTH_RADIUS_M * FRAC_PI_2, 1e-3);
        }
    }
    // A point 1 m from the pole still has an answer.
    let near = c(90.0 - 1.0 / 111_195.0, 30.0);
    let at = along_track_distance_spherical(near, c(0.0, 0.0), c(0.0, 10.0)).unwrap();
    assert_close(
        at.meters(),
        30.0_f64.to_radians() * MEAN_EARTH_RADIUS_M,
        1.0,
    );
}

#[test]
fn along_track_matches_the_foot_all_round_the_circle() {
    // On the equator the foot of (1, λ) is (0, λ): along-track is λ·R, wrapped
    // into [-180°, 180°] and signed by the side of `start` it lies on.
    let (start, end) = (c(0.0, 0.0), c(0.0, 10.0));
    for lon in [
        -179.0, -135.0, -91.0, -45.0, -1.0, 1.0, 45.0, 91.0, 135.0, 179.0,
    ] {
        let at = along_track_distance_spherical(c(1.0, lon), start, end).unwrap();
        assert_close(at.meters(), lon.to_radians() * MEAN_EARTH_RADIUS_M, 1e-6);
    }
}

#[test]
fn cross_and_along_track_from_the_start_are_positive_zero() {
    // A point at the path's start is on the path and at its origin: both
    // components are zero, and never -0.0 (start (0, 0), end (0, 10) gave it).
    for lat in (-80..=80).step_by(10) {
        for lon in (-170..=170).step_by(10) {
            let (lat, lon) = (f64::from(lat), f64::from(lon));
            let start = c(lat, lon);
            for (d_lat, d_lon) in [(0.0, 10.0), (5.0, 5.0), (-5.0, 5.0), (5.0, 0.0)] {
                let end = c(lat + d_lat, lon + d_lon);
                for d in [
                    cross_track_distance_spherical(start, start, end).unwrap(),
                    along_track_distance_spherical(start, start, end).unwrap(),
                ] {
                    let m = d.meters();
                    assert!(m.abs() < 1e-6, "{lat}, {lon}: {m}");
                    if m == 0.0 {
                        assert!(m.is_sign_positive(), "{lat}, {lon}: -0.0");
                    }
                }
            }
        }
    }
}

#[test]
fn intersection_across_the_antimeridian() {
    // The equator east from 170°E meets the 175°W meridian north from 10°S.
    let x = intersection_spherical(c(0.0, 170.0), 90.0, c(-10.0, -175.0), 0.0)
        .unwrap()
        .crossing()
        .expect("intersection exists");
    assert_close(x.lat(), 0.0, 1e-9);
    assert_close(x.lon(), -175.0, 1e-9);
}

#[test]
fn cross_and_along_track_across_the_antimeridian() {
    // Path east along the equator from 175°E to 175°W; a point 1° north at 180°.
    let (start, end, point) = (c(0.0, 175.0), c(0.0, -175.0), c(1.0, 180.0));
    let one_deg = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians();
    let xt = cross_track_distance_spherical(point, start, end)
        .unwrap()
        .meters();
    assert_close(xt, -one_deg, 1e-3);
    let at = along_track_distance_spherical(point, start, end)
        .unwrap()
        .meters();
    assert_close(at, 5.0 * one_deg, 1e-3);
    // A foot behind the start is negative.
    let behind = along_track_distance_spherical(c(1.0, -5.0), c(0.0, 0.0), c(0.0, 10.0))
        .unwrap()
        .meters();
    assert_close(behind, -5.0 * one_deg, 1e-3);
}

#[cfg(feature = "serde")]
#[test]
fn intersection_serde_round_trips_every_variant() {
    let crossing =
        Intersection::Crossing(Coordinate::new(10.0, -170.0, crate::Crs::Tokyo).unwrap());
    let json = serde_json::to_string(&crossing).unwrap();
    assert!(json.starts_with(r#"{"Crossing":"#), "{json}");
    assert_eq!(
        serde_json::from_str::<Intersection>(&json).unwrap(),
        crossing
    );
    for (variant, expected) in [
        (Intersection::SameGreatCircle, r#""SameGreatCircle""#),
        (Intersection::NoForwardCrossing, r#""NoForwardCrossing""#),
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected);
        assert_eq!(
            serde_json::from_str::<Intersection>(&json).unwrap(),
            variant
        );
    }
    // The carried coordinate is validated on the way in.
    let bad = json.replace("10.0", "91.0");
    assert_ne!(bad, json);
    assert!(serde_json::from_str::<Intersection>(&bad).is_err());
}
