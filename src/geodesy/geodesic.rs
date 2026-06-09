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
//! These take any [`LatLon`](crate::coord::LatLon) so they work on
//! [`Coordinate`](crate::Coordinate) and the per-datum newtypes alike.

use crate::coord::LatLon;
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

/// Direct geodesic problem: the point reached from `start` by traveling
/// `distance` along `bearing_deg` (exact, Karney).
#[must_use]
pub fn destination(start: &impl LatLon, bearing_deg: f64, distance: Length) -> crate::Coordinate {
    todo!("reuse geo::Geodesic destination")
}

// TODO(impl): midpoint, intermediate/interpolation, cross-track & along-track
// distance, geodesic intersection, ellipsoidal polygon area/perimeter — all
// reusing `geo` where available. Handle antimeridian and poles explicitly.
