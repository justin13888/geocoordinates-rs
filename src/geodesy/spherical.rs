//! Great-circle routines on the IUGG mean sphere.
//!
//! [`haversine_distance`], the cross/along-track pair
//! ([`cross_track_distance_spherical`] / [`along_track_distance_spherical`]),
//! and the great-circle [`intersection_spherical`] model the Earth as a sphere
//! of radius `MEAN_EARTH_RADIUS_M`, so they are approximate by construction:
//! a great-circle distance differs from the ellipsoidal geodesic by up to
//! about 0.56% (see [`haversine_distance`]). They are implemented here
//! directly and re-exported from [`super::geodesic`].
//!
//! The sphere is not any datum's surface, so the measurements accept any
//! shared [`Crs`](crate::Crs) and measure its latitudes and longitudes as
//! given. [`intersection_spherical`] produces a position, and so rejects
//! GCJ-02 and BD-09 like the ellipsoidal producers do. At a pole, a bearing is
//! taken against the meridian of the coordinate's own longitude, as in
//! [`super::geodesic`].

use core::f64::consts::PI;

use super::ecef::ellipsoid_of;
use super::geodesic::{finish, validate_pair};
use crate::coord::{Coordinate, LatLon};
use crate::error::{Error, Result};
use crate::units::Length;

/// IUGG mean Earth radius R1 = (2a + b) / 3, in meters, used by every
/// spherical routine: haversine, cross/along-track, and intersection.
pub(super) const MEAN_EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Cheap spherical (haversine) distance — approximate, named for clarity.
///
/// Uses the IUGG mean Earth radius. The `atan2` form is numerically robust for
/// the full range of separations, including near-antipodal points.
///
/// **Error bound.** Against the WGS-84 ellipsoidal geodesic
/// ([`geodesic_distance`](super::geodesic_distance)), the relative error
/// measured over a global 5° grid of pairs peaks at about 0.56% (≈ 5.6 m per
/// km). Use the geodesic where that matters.
///
/// The sphere is no datum's surface, so any shared CRS is accepted and its
/// latitudes and longitudes are measured as given; for GCJ-02 / BD-09 that is
/// the distance between the obfuscated renderings, not between the true
/// positions.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when the two CRSs differ.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, geodesy::haversine_distance};
///
/// // One degree of a great circle on the mean sphere: R·π/180.
/// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(1.0, 0.0)?);
/// assert!((haversine_distance(&a, &b)?.meters() - 111_195.08).abs() < 0.01);
/// let tokyo = Coordinate::new(1.0, 0.0, Crs::Tokyo)?;
/// assert!(matches!(haversine_distance(&a, &tokyo), Err(Error::CrsMismatch { .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn haversine_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    validate_pair(a, b)?;
    Ok(Length::from_meters(
        MEAN_EARTH_RADIUS_M * angular_distance_rad(a, b),
    ))
}

/// Signed perpendicular distance from `point` to the great-circle path
/// `start` → `end` on the mean sphere (positive to the right, negative to the
/// left). Approximate: the sphere is not the ellipsoid.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when the three CRSs differ, and
/// [`Error::InvalidValue`] (`field: "path"`)
/// when `start` and `end` are the same place or antipodal, which fixes no
/// single great circle.
///
/// ```
/// use geocoordinates::{Coordinate, Error, geodesy::cross_track_distance_spherical};
///
/// // A path east along the equator; a point 1° north lies to its left.
/// let (start, end) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 10.0)?);
/// let xt = cross_track_distance_spherical(&Coordinate::wgs84(1.0, 5.0)?, &start, &end)?;
/// assert!((xt.meters() + 111_195.08).abs() < 0.01);
/// assert!(matches!(
///     cross_track_distance_spherical(&start, &start, &start),
///     Err(Error::InvalidValue { field: "path", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn cross_track_distance_spherical(
    point: &impl LatLon,
    start: &impl LatLon,
    end: &impl LatLon,
) -> Result<Length> {
    validate_path(point, start, end)?;
    let d13 = angular_distance_rad(start, point);
    let bearing_diff = spherical_bearing_rad(start, point) - spherical_bearing_rad(start, end);
    // `+ 0.0` turns a -0.0 (a point at `start`) into +0.0.
    let xt = (d13.sin() * bearing_diff.sin()).asin() + 0.0;
    Ok(Length::from_meters(xt * MEAN_EARTH_RADIUS_M))
}

/// Distance from `start` to the foot of the perpendicular from `point` onto the
/// great-circle path `start` → `end` on the mean sphere (the along-track
/// component; negative when the foot lies behind `start`). Approximate: the
/// sphere is not the ellipsoid.
///
/// The result lies in `[-πR, πR]`: the foot is measured the shorter way round
/// the great circle from `start`.
///
/// # Errors
///
/// As for [`cross_track_distance_spherical`], and [`Error::InvalidValue`]
/// (`field: "point"`) when `point` is a pole of the path's great circle (within
/// about 6 µm), where every foot is equally near and no along-track distance
/// is defined.
///
/// ```
/// use geocoordinates::{Coordinate, Error, geodesy::along_track_distance_spherical};
///
/// let (start, end) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 10.0)?);
/// // A point beside the path, 5° along it.
/// let at = along_track_distance_spherical(&Coordinate::wgs84(1.0, 5.0)?, &start, &end)?;
/// assert!((at.meters() - 5.0 * 111_195.08).abs() < 0.1);
/// // Behind the start, the along-track distance is negative.
/// let behind = along_track_distance_spherical(&Coordinate::wgs84(0.0, -2.0)?, &start, &end)?;
/// assert!(behind.meters() < 0.0);
/// // Antipodal ends fix no single great circle.
/// let antipode = Coordinate::wgs84(0.0, 180.0)?;
/// assert!(matches!(
///     along_track_distance_spherical(&end, &start, &antipode),
///     Err(Error::InvalidValue { field: "path", .. })
/// ));
/// // The north pole is a pole of the equator: it has no along-track distance.
/// assert!(matches!(
///     along_track_distance_spherical(&Coordinate::wgs84(90.0, 0.0)?, &start, &end),
///     Err(Error::InvalidValue { field: "point", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn along_track_distance_spherical(
    point: &impl LatLon,
    start: &impl LatLon,
    end: &impl LatLon,
) -> Result<Length> {
    validate_path(point, start, end)?;
    let a = n_vector(start);
    let normal = cross(a, n_vector(end));
    let normal = normal.map(|k| k / dot(normal, normal).sqrt());
    // The unit direction of travel at `start`, toward `end`.
    let heading = cross(normal, a);
    let x = n_vector(point);
    // `x` projected onto the path's plane has length cos(cross-track angle):
    // zero at a pole of the great circle, which every foot is equally near.
    if is_parallel(dot(heading, x).hypot(dot(a, x))) {
        return Err(Error::InvalidValue {
            field: "point",
            detail: "a pole of the path's great circle has no along-track distance".into(),
        });
    }
    let at = along_angle(a, heading, x);
    Ok(Length::from_meters(at * MEAN_EARTH_RADIUS_M))
}

/// The outcome of [`intersection_spherical`]: where two great-circle paths
/// cross ahead of both starts, or why there is no single such point.
///
/// ```
/// use geocoordinates::{Coordinate, geodesy::{Intersection, intersection_spherical}};
///
/// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(-10.0, 10.0)?);
/// match intersection_spherical(&a, 90.0, &b, 0.0)? {
///     Intersection::Crossing(p) => assert!((p.lon() - 10.0).abs() < 1e-9),
///     other => panic!("expected a crossing, got {other:?}"),
/// }
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Intersection {
    /// The paths cross at this point, less than half a great circle ahead of
    /// each start (or at a start itself). It carries the inputs' CRS and no
    /// height.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, geodesy::{Intersection, intersection_spherical}};
    ///
    /// // North from (0, 0) and west from (0, 30) meet at the start of the first.
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 30.0)?);
    /// let Intersection::Crossing(p) = intersection_spherical(&a, 0.0, &b, 270.0)? else {
    ///     panic!("expected a crossing");
    /// };
    /// assert!(p.lat().abs() < 1e-9 && p.lon().abs() < 1e-9);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    Crossing(Coordinate),
    /// Both paths run along the same great circle (in either direction), so
    /// every point of it is shared and no single crossing exists.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, geodesy::{Intersection, intersection_spherical}};
    ///
    /// // Both paths run along the equator, in opposite directions.
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 30.0)?);
    /// assert_eq!(intersection_spherical(&a, 90.0, &b, 270.0)?, Intersection::SameGreatCircle);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    SameGreatCircle,
    /// The two great circles cross at a pair of antipodal points, but neither
    /// lies ahead of both starts: one path must travel backwards (or exactly
    /// half a great circle) to reach either.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, geodesy::{Intersection, intersection_spherical}};
    ///
    /// // Two meridians meet only at the poles; one path heads north, the other south.
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 30.0)?);
    /// assert_eq!(intersection_spherical(&a, 0.0, &b, 180.0)?, Intersection::NoForwardCrossing);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    NoForwardCrossing,
}

/// Where two great-circle paths, each a start point and an initial bearing in
/// degrees, cross ahead of both starts, on the mean sphere. Approximate: the
/// sphere is not the ellipsoid (see [`haversine_distance`] for the size of
/// that difference).
///
/// At a pole the bearing is taken against the meridian of the start's own
/// longitude: from `(90, λ)`, 180° runs down meridian `λ`. The result is a
/// [`Intersection::Crossing`] in `a.crs()` with no height, or names why there
/// is no single crossing.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when the CRSs differ or are GCJ-02 / BD-09
/// (obfuscated renderings; convert them to WGS-84 first), and
/// [`Error::InvalidValue`] (`field: "bearing"`) for a non-finite bearing.
///
/// ```
/// use geocoordinates::{Coordinate, Error, geodesy::{Intersection, intersection_spherical}};
///
/// // North from (0, 0) and north-west from (0, 10): they meet north of the equator.
/// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 10.0)?);
/// let Intersection::Crossing(p) = intersection_spherical(&a, 0.0, &b, 315.0)? else {
///     panic!("expected a crossing");
/// };
/// assert!(p.lat() > 9.0 && p.lon().abs() < 1e-9);
/// assert!(matches!(
///     intersection_spherical(&a, f64::NAN, &b, 315.0),
///     Err(Error::InvalidValue { field: "bearing", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn intersection_spherical(
    a: &Coordinate,
    bearing_a_deg: f64,
    b: &Coordinate,
    bearing_b_deg: f64,
) -> Result<Intersection> {
    validate_pair(a, b)?;
    ellipsoid_of(a.crs())?;
    if !bearing_a_deg.is_finite() || !bearing_b_deg.is_finite() {
        return Err(Error::InvalidValue {
            field: "bearing",
            detail: "must be finite".into(),
        });
    }
    let (p1, d1) = (n_vector(a), heading(a, bearing_a_deg));
    let (p2, d2) = (n_vector(b), heading(b, bearing_b_deg));
    // Each path's great circle is the plane normal to p × d; the circles meet
    // along the line both normals are perpendicular to.
    // Its length is the sine of the angle between the circles. It is left
    // unnormalized: the angle and position below are `atan2`s, which only its
    // direction affects.
    let i = cross(cross(p1, d1), cross(p2, d2));
    if is_parallel(dot(i, i).sqrt()) {
        return Ok(Intersection::SameGreatCircle);
    }
    for x in [i, i.map(|v| -v)] {
        if is_forward(along_angle(p1, d1, x)) && is_forward(along_angle(p2, d2, x)) {
            let lat = x[2].atan2(x[0].hypot(x[1])).to_degrees();
            let lon = x[1].atan2(x[0]).to_degrees();
            return finish(lat, lon, a.crs()).map(Intersection::Crossing);
        }
    }
    Ok(Intersection::NoForwardCrossing)
}

/// A 3-vector in Earth-centred axes (x to (0, 0), z to the north pole).
type Vec3 = [f64; 3];

fn dot(u: Vec3, v: Vec3) -> f64 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

fn cross(u: Vec3, v: Vec3) -> Vec3 {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// The n-vector (unit position vector) of a position on the sphere.
fn n_vector(p: &impl LatLon) -> Vec3 {
    let (phi, lam) = (p.lat().to_radians(), p.lon().to_radians());
    [phi.cos() * lam.cos(), phi.cos() * lam.sin(), phi.sin()]
}

/// The unit direction of travel leaving `p` on `bearing_deg`.
///
/// East is `(-sin λ, cos λ, 0)` and north `(-sin φ cos λ, -sin φ sin λ, cos φ)`;
/// both stay defined at a pole, where they take the meridian of `p`'s own
/// longitude, so no pole needs a special case.
fn heading(p: &impl LatLon, bearing_deg: f64) -> Vec3 {
    let (phi, lam) = (p.lat().to_radians(), p.lon().to_radians());
    let theta = bearing_deg.to_radians();
    let east = [-lam.sin(), lam.cos(), 0.0];
    let north = [-phi.sin() * lam.cos(), -phi.sin() * lam.sin(), phi.cos()];
    [0, 1, 2].map(|k| north[k] * theta.cos() + east[k] * theta.sin())
}

/// The signed angle, in radians `[-π, π]`, travelled from `p` heading `d` to
/// reach `x` along their great circle (negative when `x` lies behind).
fn along_angle(p: Vec3, d: Vec3, x: Vec3) -> f64 {
    dot(d, x).atan2(dot(p, x))
}

/// Whether two unit vectors whose cross product has length `sin_angle` are
/// treated as parallel or antiparallel: strictly below 1e-12, about 6 µm on
/// the Earth's surface. Two great-circle normals that are count as one circle;
/// two positions that are count as coincident or antipodal.
fn is_parallel(sin_angle: f64) -> bool {
    sin_angle < 1e-12
}

/// Whether a crossing `along` radians from a start lies ahead of it: in
/// `(-1e-12, π - 1e-12)`. The slack admits a crossing at the start itself
/// despite rounding, and excludes the antipode of the start, which every
/// direction reaches after half a great circle.
fn is_forward(along: f64) -> bool {
    along > -1e-12 && along < PI - 1e-12
}

/// Validates a cross/along-track path: one shared CRS, and `start` / `end` neither coincident nor antipodal (either leaves
/// the great circle through them undefined).
fn validate_path(point: &impl LatLon, start: &impl LatLon, end: &impl LatLon) -> Result<()> {
    validate_pair(point, start)?;
    validate_pair(start, end)?;
    let normal = cross(n_vector(start), n_vector(end));
    if is_parallel(dot(normal, normal).sqrt()) {
        return Err(Error::InvalidValue {
            field: "path",
            detail: "start and end must be distinct and not antipodal".into(),
        });
    }
    Ok(())
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

#[cfg(test)]
mod tests;
