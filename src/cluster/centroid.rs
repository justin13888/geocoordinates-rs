//! [`centroid`], re-exported flat from [`cluster`](super), and the unit-vector
//! arithmetic every cluster's centroid shares.

use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::geodesy::Ellipsoid;
use crate::{Approx, Length};

/// Added to every merged bound, in meters, to cover round-off in the vectors.
pub(super) const ROUND_OFF_M: f64 = 1e-6;
/// A mean vector shorter than this has no meaningful direction.
const DEGENERATE_MEAN: f64 = 1e-9;

/// The unit normal of the ellipsoid at `point` (its n-vector): the direction its
/// geodetic latitude and longitude name, which is one vector at a pole and across
/// the antimeridian whatever the longitude says.
pub(super) fn normal(point: Coordinate) -> [f64; 3] {
    let (sin_lat, cos_lat) = point.lat().to_radians().sin_cos();
    let (sin_lon, cos_lon) = point.lon().to_radians().sin_cos();
    [cos_lat * cos_lon, cos_lat * sin_lon, sin_lat]
}

/// The largest radius of curvature of `crs`'s ellipsoid, `a² / b`.
///
/// Along any curve, arc length is at most this radius times the angle the surface
/// normal turns through, since both principal radii are at most `a² / b`. So
/// this radius times the angle between two normals bounds the geodesic distance
/// between their points from above, tight to about 1%, without solving a geodesic.
///
/// # Errors
///
/// [`Error::CrsMismatch`] on GCJ-02 / BD-09, which have no ellipsoid.
pub(super) fn normal_radius_m(crs: Crs) -> Result<f64> {
    let ellipsoid = Ellipsoid::for_crs(crs).ok_or(Error::CrsMismatch {
        expected: Crs::Wgs84,
        found: crs,
    })?;
    let (a, b) = (
        ellipsoid.semi_major().meters(),
        ellipsoid.semi_minor().meters(),
    );
    Ok(a * a / b)
}

/// The angle between two unit vectors, in radians, accurate at every separation.
pub(super) fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    (cross[0].hypot(cross[1]).hypot(cross[2])).atan2(dot)
}

/// The point on `crs` whose normal is the direction of `sum`, a sum of `count` unit
/// vectors, or `None` when those vectors cancel and `sum` has no direction.
pub(super) fn from_sum(sum: [f64; 3], count: u64, crs: Crs) -> Option<Coordinate> {
    let length = sum[0].hypot(sum[1]).hypot(sum[2]);
    if length < DEGENERATE_MEAN * count as f64 {
        return None;
    }
    let lat = sum[2].atan2(sum[0].hypot(sum[1])).to_degrees();
    let lon = sum[1].atan2(sum[0]).to_degrees();
    Some(Coordinate::new(lat, lon, crs).expect("atan2 gives an in-range position"))
}

/// The mean position of a set of points: the normalized mean of their unit
/// vectors, converted back to the ellipsoid.
///
/// Well defined across ±180° and at the poles, because the mean is taken over
/// surface normals and never over latitude and longitude. The result carries no
/// height, and input heights are ignored.
///
/// **This is approximate.** It is the n-vector mean, not the point minimizing the
/// sum of squared geodesic distances, so the result is an [`Approx`]. Its
/// [`max_error`](Approx::max_error) is an upper bound on the geodesic distance from
/// the result to the farthest input, tight to about 1%: for a set small enough to
/// lie within a hemisphere, any other definition of the set's middle lies within
/// that distance too. The two means differ by far less than the bound for a tight
/// set, and the bound is the spread of the set, not an estimate of that difference.
///
/// # Errors
///
/// [`Error::InvalidValue`] (`field: "points"`) for no points, or for points whose
/// unit vectors cancel, such as an antipodal pair, which have no mean direction;
/// [`Error::CrsMismatch`] when the points' CRSs differ or are GCJ-02 / BD-09
/// (obfuscated renderings with no ellipsoid; convert them to WGS-84 first).
///
/// ```
/// use geocoordinates::{Coordinate, Error, cluster::centroid};
///
/// // Across the antimeridian the mean stays by the antimeridian.
/// let middle = centroid(&[Coordinate::wgs84(0.0, 179.0)?, Coordinate::wgs84(0.0, -179.0)?])?;
/// assert!((middle.value().lon().abs() - 180.0).abs() < 1e-9);
/// assert!((middle.max_error().meters() - 111_694.0).abs() < 1_000.0);
///
/// // Antipodal points have no mean.
/// let antipodal = [Coordinate::wgs84(10.0, 20.0)?, Coordinate::wgs84(-10.0, -160.0)?];
/// assert!(matches!(centroid(&antipodal), Err(Error::InvalidValue { field: "points", .. })));
/// assert!(matches!(centroid(&[]), Err(Error::InvalidValue { field: "points", .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn centroid(points: &[Coordinate]) -> Result<Approx<Coordinate>> {
    let first = points.first().ok_or_else(|| Error::InvalidValue {
        field: "points",
        detail: "no points to average".into(),
    })?;
    let crs = first.crs();
    let radius_m = normal_radius_m(crs)?;
    let mut sum = [0.0; 3];
    for point in points {
        if point.crs() != crs {
            return Err(Error::CrsMismatch {
                expected: crs,
                found: point.crs(),
            });
        }
        let unit = normal(*point);
        sum = [sum[0] + unit[0], sum[1] + unit[1], sum[2] + unit[2]];
    }
    let middle = from_sum(sum, points.len() as u64, crs).ok_or_else(|| Error::InvalidValue {
        field: "points",
        detail: "the points' unit vectors cancel, so they have no mean direction".into(),
    })?;
    let center = normal(middle);
    let spread = points
        .iter()
        .map(|point| angle_between(center, normal(*point)))
        .fold(0.0, f64::max);
    Approx::new(middle, Length::from_meters(spread * radius_m + ROUND_OFF_M))
}
