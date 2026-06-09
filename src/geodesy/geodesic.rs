//! Distance, bearing, and geodesic problems.
//!
//! The model is explicit in the function name, so the accuracy is obvious
//! without reading docs:
//!
//! - [`geodesic_distance`] — **exact** ellipsoidal distance (Karney's
//!   algorithm, robust everywhere including near-antipodal points). Reuses
//!   [`geo`](https://docs.rs/geo).
//! - [`haversine_distance`] — spherical approximation (cheap, ~0.5% error).
//! - [`rhumb_distance`] — loxodrome (constant-bearing) distance for marine
//!   navigation.
//!
//! **Measurement** functions (distances, bearings) take any [`LatLon`], so they
//! work on [`Coordinate`] and the per-datum newtypes alike — a scalar result has
//! no reference system to mislabel.
//!
//! **Producer** functions (those returning a position: [`destination`],
//! [`midpoint`], [`intermediate`], [`intersection`], [`rhumb_destination`])
//! take `&Coordinate` and propagate its [`Crs`](crate::Crs) to the result. The
//! ellipsoidal math assumes a WGS-84 / true-datum input; feeding an obfuscated
//! GCJ-02 / BD-09 position is a logic error.

use crate::coord::{Coordinate, LatLon};
use crate::units::Length;

/// Exact ellipsoidal (Karney geodesic) distance between two points.
///
/// Preferred over Vincenty, which fails to converge for near-antipodal points.
#[must_use]
pub fn geodesic_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    todo!("reuse geo::Geodesic distance")
}

/// Cheap spherical (haversine) distance — approximate, named for clarity.
#[must_use]
pub fn haversine_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    todo!("reuse geo::Haversine distance")
}

/// Rhumb-line (loxodrome / constant-bearing) distance.
#[must_use]
pub fn rhumb_distance(a: &impl LatLon, b: &impl LatLon) -> Length {
    todo!("reuse geo::Rhumb distance")
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees.
#[must_use]
pub fn initial_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    todo!("reuse geo bearing")
}

/// Final bearing (azimuth on arrival) of the geodesic from `a` to `b`, in
/// degrees. Differs from [`initial_bearing`] on the ellipsoid because the
/// azimuth changes along a geodesic.
#[must_use]
pub fn final_bearing(a: &impl LatLon, b: &impl LatLon) -> f64 {
    todo!("reuse geo geodesic azimuth at the destination")
}

/// Direct geodesic problem: the point reached from `start` by traveling
/// `distance` along `bearing_deg` (exact, Karney). The result carries
/// `start.crs`.
#[must_use]
pub fn destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Coordinate {
    todo!("reuse geo::Geodesic destination; result carries start.crs")
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
    todo!("reuse geo geodesic interpolation; wrap longitude across the antimeridian")
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
    todo!("reuse geo::Rhumb bearing")
}

/// The point reached from `start` by traveling `distance` along a constant
/// `bearing_deg` rhumb line (loxodrome). The result carries `start.crs`.
#[must_use]
pub fn rhumb_destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Coordinate {
    todo!("reuse geo::Rhumb destination; result carries start.crs")
}

// Polygon and line ops — ellipsoidal area/perimeter, point-in-polygon, centroid,
// convex hull, buffers, bounding boxes, line densification, and simplification
// (Douglas-Peucker / `geo::Simplify`) — are delegated to `geo` directly (it
// implements them correctly); this module owns only the point-to-point geodesic
// ops above.
