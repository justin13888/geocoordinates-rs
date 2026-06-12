//! Distance, bearing, and geodesic problems.
//!
//! Only the spherical [`haversine_distance`] (cheap, ~0.5% error) is available
//! in this release. The full set — exact ellipsoidal Karney `geodesic_distance`,
//! rhumb-line distance and bearing, forward/final bearings, and the position
//! producers (`destination`, `midpoint`, `intermediate`, `intersection`,
//! `rhumb_destination`) — lands with the deferred geodesics milestone (see
//! `ROADMAP.md`). Karney math will be delegated to
//! [`geographiclib-rs`](https://docs.rs/geographiclib-rs); the spherical
//! rhumb/loxodrome and cross/along-track routines are implemented here.
//!
//! The model is explicit in the function name, so the accuracy is obvious
//! without reading docs. Measurement functions (distances, bearings) take any
//! [`LatLon`], so they work on `Coordinate` and the per-datum newtypes alike — a
//! scalar result has no reference system to mislabel. Producer functions will
//! take `&Coordinate` and propagate its CRS to the result; that ellipsoidal math
//! assumes a WGS-84 / true-datum input, so feeding an obfuscated GCJ-02 / BD-09
//! position is a logic error.

use crate::coord::LatLon;
use crate::units::Length;

// --- geodesic_distance: released with the geodesics milestone (see ROADMAP.md) ---
/*
/// Exact ellipsoidal (Karney geodesic) distance between two points.
///
/// Preferred over Vincenty, which fails to converge for near-antipodal points.
#[must_use]
pub fn geodesic_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    todo!("geographiclib_rs::Geodesic::wgs84().inverse() distance")
}
*/

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

// --- The remaining geodesic ops: released with the geodesics milestone (see ROADMAP.md) ---
/*
/// Rhumb-line (loxodrome / constant-bearing) distance.
#[must_use]
pub fn rhumb_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    todo!("hand-rolled spherical loxodrome distance")
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees.
#[must_use]
pub fn initial_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    todo!("geographiclib_rs::Geodesic::wgs84().inverse() azimuth at `a`")
}

/// Final bearing (azimuth on arrival) of the geodesic from `a` to `b`, in
/// degrees. Differs from [`initial_bearing`] on the ellipsoid because the
/// azimuth changes along a geodesic.
#[must_use]
pub fn final_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    todo!("geographiclib_rs::Geodesic::wgs84().inverse() azimuth at `b`")
}

/// Direct geodesic problem: the point reached from `start` by traveling
/// `distance` along `bearing_deg` (exact, Karney). The result carries
/// `start.crs`.
#[must_use]
pub fn destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Coordinate {
    todo!("geographiclib_rs::Geodesic::wgs84().direct(); result carries start.crs")
}

/// The geodesic midpoint between `a` and `b`. The result carries `a.crs`.
#[must_use]
pub fn midpoint(a: &Coordinate, b: &Coordinate) -> Coordinate {
    todo!("intermediate at fraction 0.5; result carries a.crs")
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the geodesic
/// from `a` to `b`. Interpolates along the geodesic, handling the antimeridian.
/// The result carries `a.crs`.
#[must_use]
pub fn intermediate(a: &Coordinate, b: &Coordinate, fraction: f64) -> Coordinate {
    todo!(
        "inverse() for distance/azimuth, then direct() at fraction * s12; \
         wrap longitude across the antimeridian"
    )
}

/// Signed perpendicular distance from `point` to the geodesic path
/// `start` → `end` (positive to the right of the path, negative to the left).
#[must_use]
pub fn cross_track_distance(point: &impl LatLon, start: &impl LatLon, end: &impl LatLon) -> Length {
    todo!("cross-track distance relative to the start→end geodesic")
}

/// Distance from `start` to the foot of the perpendicular from `point` onto the
/// geodesic path `start` → `end` (the along-track component).
#[must_use]
pub fn along_track_distance(point: &impl LatLon, start: &impl LatLon, end: &impl LatLon) -> Length {
    todo!("along-track distance relative to the start→end geodesic")
}

/// Intersection of two geodesics, each given by a point and an initial bearing.
///
/// Returns `None` when the paths are parallel or coincident. Geodesics on the
/// sphere generally intersect twice; this returns the nearer intersection. The
/// result carries `a.crs`.
#[must_use]
pub fn intersection(
    a: &Coordinate,
    bearing_a_deg: f64,
    b: &Coordinate,
    bearing_b_deg: f64,
) -> Option<Coordinate> {
    todo!("great-circle path intersection; handle antimeridian and poles")
}

/// Rhumb-line (loxodrome / constant) bearing from `a` to `b`, in degrees.
#[must_use]
pub fn rhumb_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    todo!("hand-rolled spherical loxodrome bearing")
}

/// The point reached from `start` by traveling `distance` along a constant
/// `bearing_deg` rhumb line (loxodrome). The result carries `start.crs`.
#[must_use]
pub fn rhumb_destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Coordinate {
    todo!("hand-rolled spherical loxodrome destination; result carries start.crs")
}
*/

// Polygon and line ops — ellipsoidal area/perimeter, point-in-polygon, centroid,
// convex hull, buffers, bounding boxes, line densification, and simplification —
// are out of scope for this crate: use the `geo` crate directly for them. This
// module owns only the point-to-point geodesic ops above.
