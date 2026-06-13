//! Distance, bearing, and geodesic problems.
//!
//! Exact ellipsoidal (Karney) [`geodesic_distance`], [`initial_bearing`] /
//! [`final_bearing`], and the position producers ([`destination`], [`midpoint`],
//! [`intermediate`], [`intersection`]) delegate to
//! [`geographiclib-rs`](https://docs.rs/geographiclib-rs). The spherical
//! [`haversine_distance`], rhumb/loxodrome ([`rhumb_distance`] /
//! [`rhumb_bearing`] / [`rhumb_destination`]), and cross/along-track
//! ([`cross_track_distance`] / [`along_track_distance`]) routines are
//! implemented here directly.
//!
//! The model is explicit in the function name, so the accuracy is obvious
//! without reading docs. Measurement functions (distances, bearings) take any
//! [`LatLon`], so they work on `Coordinate` and the per-datum newtypes alike — a
//! scalar result has no reference system to mislabel. Producer functions will
//! take `&Coordinate` and propagate its CRS to the result; that ellipsoidal math
//! assumes a WGS-84 / true-datum input, so feeding an obfuscated GCJ-02 / BD-09
//! position is a logic error.

use core::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

use crate::angle::{normalize_degrees, wrap_longitude};
use crate::coord::{Coordinate, LatLon};
use crate::units::Length;

/// IUGG mean Earth radius R1 = (2a + b) / 3, in meters, used by the spherical
/// (rhumb, cross/along-track, intersection) routines.
const MEAN_EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Exact ellipsoidal (Karney geodesic) distance between two points.
///
/// Preferred over Vincenty, which fails to converge for near-antipodal points.
#[must_use]
pub fn geodesic_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    let s12: f64 = Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Length::from_meters(s12)
}

/// Cheap spherical (haversine) distance — approximate, named for clarity.
///
/// Uses the IUGG mean Earth radius. The `atan2` form is numerically robust for
/// the full range of separations, including near-antipodal points.
#[must_use]
pub fn haversine_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    /// IUGG mean Earth radius R1 = (2a + b) / 3, in meters.
    const MEAN_EARTH_RADIUS_M: f64 = 6_371_008.8;

    let lat1 = a.lat().to_radians();
    let lat2 = b.lat().to_radians();
    let d_lat = (b.lat() - a.lat()).to_radians();
    let d_lon = (b.lon() - a.lon()).to_radians();

    let h = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * h.sqrt().atan2((1.0 - h).max(0.0).sqrt());
    Length::from_meters(MEAN_EARTH_RADIUS_M * c)
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees `[0, 360)`.
#[must_use]
pub fn initial_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    // geographiclib-rs returns the 3-tuple as (azi1, azi2, a12).
    let (azi1, _, _): (f64, f64, f64) =
        Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    normalize_degrees(azi1)
}

/// Final bearing (azimuth on arrival) of the geodesic from `a` to `b`, in
/// degrees `[0, 360)`. Differs from [`initial_bearing`] on the ellipsoid.
#[must_use]
pub fn final_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    // The 3-tuple is (azi1, azi2, a12); the final azimuth is the second element.
    let (_, azi2, _): (f64, f64, f64) =
        Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    normalize_degrees(azi2)
}

/// Direct geodesic problem: the point reached from `start` by traveling
/// `distance` along `bearing_deg` (exact, Karney). Carries `start.crs`.
#[must_use]
pub fn destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Coordinate {
    let (lat2, lon2): (f64, f64) =
        Geodesic::wgs84().direct(start.lat, start.lon, bearing_deg, distance.meters());
    Coordinate::new(lat2, lon2, start.crs)
}

/// The geodesic midpoint between `a` and `b`. Carries `a.crs`.
#[must_use]
pub fn midpoint(a: &Coordinate, b: &Coordinate) -> Coordinate {
    intermediate(a, b, 0.5)
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the geodesic
/// from `a` to `b`. Carries `a.crs`.
#[must_use]
pub fn intermediate(a: &Coordinate, b: &Coordinate, fraction: f64) -> Coordinate {
    let g = Geodesic::wgs84();
    // The 4-tuple is (s12, azi1, azi2, a12); travel `fraction · s12` along azi1.
    let (s12, azi1, _, _): (f64, f64, f64, f64) = g.inverse(a.lat, a.lon, b.lat, b.lon);
    let (lat, lon): (f64, f64) = g.direct(a.lat, a.lon, azi1, s12 * fraction);
    Coordinate::new(lat, lon, a.crs)
}

/// Rhumb-line (loxodrome / constant-bearing) distance, on a sphere.
#[must_use]
pub fn rhumb_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    let phi1 = a.lat().to_radians();
    let phi2 = b.lat().to_radians();
    let d_phi = phi2 - phi1;
    let d_lon = shortest_d_lon(a, b);
    let d_psi = stretched_lat_diff(phi1, phi2);
    let q = if d_psi.abs() > 1e-12 {
        d_phi / d_psi
    } else {
        phi1.cos()
    };
    let dist = (d_phi * d_phi + q * q * d_lon * d_lon).sqrt() * MEAN_EARTH_RADIUS_M;
    Length::from_meters(dist)
}

/// Rhumb-line (loxodrome / constant) bearing from `a` to `b`, in degrees
/// `[0, 360)`.
#[must_use]
pub fn rhumb_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let d_psi = stretched_lat_diff(a.lat().to_radians(), b.lat().to_radians());
    normalize_degrees(shortest_d_lon(a, b).atan2(d_psi).to_degrees())
}

/// The point reached from `start` by traveling `distance` along a constant
/// `bearing_deg` rhumb line. Carries `start.crs`.
#[must_use]
pub fn rhumb_destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Coordinate {
    let theta = bearing_deg.to_radians();
    let delta = distance.meters() / MEAN_EARTH_RADIUS_M;
    let phi1 = start.lat.to_radians();
    let d_phi = delta * theta.cos();
    let mut phi2 = phi1 + d_phi;
    // Crossing a pole flips to the far side.
    if phi2.abs() > FRAC_PI_2 {
        phi2 = if phi2 > 0.0 { PI - phi2 } else { -PI - phi2 };
    }
    let d_psi = stretched_lat_diff(phi1, phi2);
    let q = if d_psi.abs() > 1e-12 {
        d_phi / d_psi
    } else {
        phi1.cos()
    };
    let d_lon = (delta * theta.sin() / q).to_degrees();
    Coordinate::new(
        phi2.to_degrees(),
        wrap_longitude(start.lon + d_lon),
        start.crs,
    )
}

/// Signed perpendicular distance from `point` to the great-circle path
/// `start` → `end` (positive to the right, negative to the left).
#[must_use]
pub fn cross_track_distance(point: &impl LatLon, start: &impl LatLon, end: &impl LatLon) -> Length {
    let d13 = angular_distance_rad(start, point);
    let bearing_diff = spherical_bearing_rad(start, point) - spherical_bearing_rad(start, end);
    let xt = (d13.sin() * bearing_diff.sin()).asin();
    Length::from_meters(xt * MEAN_EARTH_RADIUS_M)
}

/// Distance from `start` to the foot of the perpendicular from `point` onto the
/// great-circle path `start` → `end` (the along-track component; negative when
/// the foot lies behind `start`).
#[must_use]
pub fn along_track_distance(point: &impl LatLon, start: &impl LatLon, end: &impl LatLon) -> Length {
    let d13 = angular_distance_rad(start, point);
    let bearing_diff = spherical_bearing_rad(start, point) - spherical_bearing_rad(start, end);
    let xt = (d13.sin() * bearing_diff.sin()).asin();
    let at = (d13.cos() / xt.cos()).clamp(-1.0, 1.0).acos();
    let signed = at * bearing_diff.cos().signum();
    Length::from_meters(signed * MEAN_EARTH_RADIUS_M)
}

/// Intersection of two great circles, each given by a point and an initial
/// bearing. `None` when the paths are parallel/coincident or ambiguous. Carries
/// `a.crs`.
#[must_use]
pub fn intersection(
    a: &Coordinate,
    bearing_a_deg: f64,
    b: &Coordinate,
    bearing_b_deg: f64,
) -> Option<Coordinate> {
    let (phi1, lam1) = (a.lat.to_radians(), a.lon.to_radians());
    let (phi2, lam2) = (b.lat.to_radians(), b.lon.to_radians());
    let theta13 = bearing_a_deg.to_radians();
    let theta23 = bearing_b_deg.to_radians();
    let d_phi = phi2 - phi1;
    let d_lam = lam2 - lam1;

    let delta12 = 2.0
        * ((d_phi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (d_lam / 2.0).sin().powi(2))
            .sqrt()
            .asin();
    if delta12 < 1e-12 {
        return Some(*a); // coincident endpoints
    }

    let theta_a = ((phi2.sin() - phi1.sin() * delta12.cos()) / (delta12.sin() * phi1.cos()))
        .clamp(-1.0, 1.0)
        .acos();
    let theta_b = ((phi1.sin() - phi2.sin() * delta12.cos()) / (delta12.sin() * phi2.cos()))
        .clamp(-1.0, 1.0)
        .acos();
    let (theta12, theta21) = if d_lam.sin() > 0.0 {
        (theta_a, 2.0 * PI - theta_b)
    } else {
        (2.0 * PI - theta_a, theta_b)
    };

    let alpha1 = theta13 - theta12;
    let alpha2 = theta21 - theta23;
    if alpha1.sin().abs() < 1e-12 && alpha2.sin().abs() < 1e-12 {
        return None; // same great circle — infinitely many intersections
    }
    if alpha1.sin() * alpha2.sin() < 0.0 {
        return None; // ambiguous (the antipodal intersection)
    }

    let cos_alpha3 = -alpha1.cos() * alpha2.cos() + alpha1.sin() * alpha2.sin() * delta12.cos();
    let delta13 = (delta12.sin() * alpha1.sin() * alpha2.sin())
        .atan2(alpha2.cos() + alpha1.cos() * cos_alpha3);
    let phi3 = (phi1.sin() * delta13.cos() + phi1.cos() * delta13.sin() * theta13.cos())
        .clamp(-1.0, 1.0)
        .asin();
    let d_lam13 =
        (theta13.sin() * delta13.sin() * phi1.cos()).atan2(delta13.cos() - phi1.sin() * phi3.sin());
    let lon3 = wrap_longitude((lam1 + d_lam13).to_degrees());
    Some(Coordinate::new(phi3.to_degrees(), lon3, a.crs))
}

/// The shorter signed longitude difference `b − a`, in radians (antimeridian-safe).
fn shortest_d_lon(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let mut d_lon = (b.lon() - a.lon()).to_radians();
    if d_lon.abs() > PI {
        d_lon -= d_lon.signum() * 2.0 * PI;
    }
    d_lon
}

/// The stretched-latitude (Mercator `ψ`) difference between two latitudes.
fn stretched_lat_diff(phi1: f64, phi2: f64) -> f64 {
    ((phi2 / 2.0 + FRAC_PI_4).tan() / (phi1 / 2.0 + FRAC_PI_4).tan()).ln()
}

/// Initial bearing on a sphere, in radians.
fn spherical_bearing_rad(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let phi1 = a.lat().to_radians();
    let phi2 = b.lat().to_radians();
    let d_lon = (b.lon() - a.lon()).to_radians();
    let y = d_lon.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * d_lon.cos();
    y.atan2(x)
}

/// Angular distance on a sphere, in radians (haversine).
fn angular_distance_rad(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let phi1 = a.lat().to_radians();
    let phi2 = b.lat().to_radians();
    let d_phi = (b.lat() - a.lat()).to_radians();
    let d_lon = (b.lon() - a.lon()).to_radians();
    let h = (d_phi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (d_lon / 2.0).sin().powi(2);
    2.0 * h.sqrt().atan2((1.0 - h).max(0.0).sqrt())
}

// Polygon and line ops — ellipsoidal area/perimeter, point-in-polygon, centroid,
// convex hull, buffers, bounding boxes, line densification, and simplification —
// are out of scope for this crate: use the `geo` crate directly for them. This
// module owns only the point-to-point geodesic ops above.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    fn c(lat: f64, lon: f64) -> Coordinate {
        Coordinate::wgs84(lat, lon)
    }

    #[test]
    fn geodesic_distance_equator_reference() {
        // 1° along the equator on WGS-84 (a · π/180).
        assert_close(
            geodesic_distance(&c(0.0, 0.0), &c(0.0, 1.0)).meters(),
            111_319.49,
            0.5,
        );
    }

    #[test]
    fn bearings_are_cardinal() {
        assert_close(initial_bearing(&c(0.0, 0.0), &c(0.0, 1.0)), 90.0, 1e-6); // east
        assert_close(initial_bearing(&c(0.0, 0.0), &c(1.0, 0.0)), 0.0, 1e-6); // north
        assert_close(final_bearing(&c(0.0, 0.0), &c(1.0, 0.0)), 0.0, 1e-6);
    }

    #[test]
    fn destination_inverts_distance_and_bearing() {
        let (start, end) = (c(40.0, -75.0), c(41.0, -73.0));
        let d = destination(
            &start,
            initial_bearing(&start, &end),
            geodesic_distance(&start, &end),
        );
        assert_close(d.lat, 41.0, 1e-6);
        assert_close(d.lon, -73.0, 1e-6);
    }

    #[test]
    fn destination_carries_crs() {
        let d = destination(
            &Coordinate::gcj02(0.0, 0.0),
            90.0,
            Length::from_meters(100_000.0),
        );
        assert_eq!(d.crs, crate::Crs::Gcj02);
    }

    #[test]
    fn midpoint_and_intermediate() {
        let mid = midpoint(&c(0.0, 0.0), &c(0.0, 2.0));
        assert_close(mid.lat, 0.0, 1e-6);
        assert_close(mid.lon, 1.0, 1e-6);
        assert_close(
            intermediate(&c(0.0, 0.0), &c(0.0, 10.0), 0.25).lon,
            2.5,
            1e-6,
        );
    }

    #[test]
    fn rhumb_references() {
        // Along the equator the rhumb line is the equator (q = cos 0 = 1).
        assert_close(
            rhumb_distance(&c(0.0, 0.0), &c(0.0, 1.0)).meters(),
            111_195.0,
            1.0,
        );
        assert_close(rhumb_bearing(&c(0.0, 0.0), &c(0.0, 10.0)), 90.0, 1e-9); // east
        assert_close(rhumb_bearing(&c(0.0, 0.0), &c(10.0, 0.0)), 0.0, 1e-9); // north
    }

    #[test]
    fn rhumb_destination_round_trip() {
        let (start, end) = (c(40.0, -75.0), c(45.0, -70.0));
        let d = rhumb_destination(
            &start,
            rhumb_bearing(&start, &end),
            rhumb_distance(&start, &end),
        );
        assert_close(d.lat, 45.0, 1e-6);
        assert_close(d.lon, -70.0, 1e-6);
    }

    #[test]
    fn cross_and_along_track() {
        // Path east along the equator; a point 1° north at lon 5.
        let (start, end, point) = (c(0.0, 0.0), c(0.0, 10.0), c(1.0, 5.0));
        let xt = cross_track_distance(&point, &start, &end).meters();
        assert!(xt < 0.0); // north of an eastward path is to the left
        assert_close(xt.abs(), 111_195.0, 2_000.0);
        let at = along_track_distance(&point, &start, &end).meters();
        assert_close(at, 5.0 * 111_195.0, 2_000.0); // foot at lon 5
    }

    #[test]
    fn intersection_of_equator_and_meridian() {
        // Equator (east from (0,-10)) meets the prime meridian (north from
        // (-10,0)) at (0, 0); neither endpoint lies on the other path.
        let x =
            intersection(&c(0.0, -10.0), 90.0, &c(-10.0, 0.0), 0.0).expect("intersection exists");
        assert_close(x.lat, 0.0, 1e-7);
        assert_close(x.lon, 0.0, 1e-7);
    }

    #[test]
    fn coincident_great_circles_have_no_intersection() {
        // Both paths run east along the equator — the same great circle.
        assert!(intersection(&c(0.0, 0.0), 90.0, &c(0.0, 5.0), 90.0).is_none());
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
        let fb = final_bearing(&c(0.0, 0.0), &c(10.0, 10.0));
        assert!(
            (40.0..50.0).contains(&fb),
            "final bearing {fb} out of expected band"
        );
    }

    #[test]
    fn rhumb_destination_due_east_holds_latitude() {
        // A due-east rhumb keeps latitude fixed (d_psi → 0, so q = cos φ₁).
        let d = rhumb_destination(&c(40.0, 0.0), 90.0, Length::from_meters(100_000.0));
        assert_close(d.lat, 40.0, 1e-9);
        assert_close(d.lon, 1.173_979_358_250_968, 1e-9);
    }

    #[test]
    fn rhumb_destination_reflects_over_the_poles() {
        // Pushing north past the pole reflects the latitude back down the far side.
        let n = rhumb_destination(&c(80.0, 0.0), 0.0, Length::from_meters(1_668_000.0));
        assert_close(n.lat, 84.999_336_333_074_71, 1e-9);
        assert_close(n.lon, 0.0, 1e-9);
        // …and symmetrically over the south pole (the negative reflection arm).
        let s = rhumb_destination(&c(-80.0, 0.0), 180.0, Length::from_meters(1_668_000.0));
        assert_close(s.lat, -84.999_336_333_074_71, 1e-9);
        assert_close(s.lon, 0.0, 1e-9);
    }

    #[test]
    fn intersection_second_geometry_and_west_branch() {
        // A real great circle (east from (10,0)) meets the λ=10 meridian at a
        // positive latitude — pins the φ₃/λ₃ formulas away from the trivial origin.
        let x = intersection(&c(10.0, 0.0), 90.0, &c(0.0, 10.0), 0.0).expect("intersection exists");
        assert_close(x.lat, 9.851_076_116_583_906, 1e-6);
        assert_close(x.lon, 10.0, 1e-6);
        // b west of a (Δλ < 0) takes the other azimuth-assignment branch.
        let w =
            intersection(&c(0.0, 10.0), 270.0, &c(-10.0, 0.0), 0.0).expect("intersection exists");
        assert_close(w.lat, 0.0, 1e-6);
        assert_close(w.lon, 0.0, 1e-6);
    }

    #[test]
    fn intersection_coincident_endpoints_short_circuit() {
        // Identical start points (Δ₁₂ ≈ 0) return that point regardless of bearings.
        let x = intersection(&c(5.0, 5.0), 10.0, &c(5.0, 5.0), 80.0).expect("coincident point");
        assert_close(x.lat, 5.0, 1e-12);
        assert_close(x.lon, 5.0, 1e-12);
    }

    #[test]
    fn intersection_behind_a_bearing_is_none() {
        // The crossing of these great circles lies *behind* the westward path, so
        // the forward intersection is the ambiguous antipode → None.
        assert!(intersection(&c(10.0, 0.0), 270.0, &c(0.0, 10.0), 0.0).is_none());
    }
}
