//! Geodetic core: ellipsoids, ECEF, local tangent frames, and geodesic
//! computations.
//!
//! This module owns its math, with one exception: exact ellipsoidal (Karney)
//! geodesics are delegated to
//! [`geographiclib-rs`](https://docs.rs/geographiclib-rs) — the validated Rust
//! port of Karney's GeographicLib (and the same engine the `geo` crate uses) —
//! rather than re-deriving it. Everything else (haversine, rhumb/loxodrome,
//! ECEF, ENU/NED/AER, datum/ellipsoid parameters tied to our
//! [`Crs`](crate::Crs)) is implemented here directly.

pub mod geodesic; // ellipsoidal (Karney) geodesics; re-exports the two below
mod rhumb; // rhumb/loxodrome on the mean sphere
mod spherical; // haversine, cross/along-track, intersection on the mean sphere

pub use geodesic::{Intersection, haversine_distance};

pub mod ecef; // ECEF geocentric coordinates
pub mod ellipsoid; // reference ellipsoid parameters
pub mod frames; // local tangent frames (ENU/NED/AER)

pub use ecef::Ecef;
pub use ellipsoid::Ellipsoid;
pub use frames::{Aer, Enu, Ned};

pub use geodesic::{
    along_track_distance_spherical, cross_track_distance_spherical, destination, final_bearing,
    geodesic_distance, initial_bearing, intermediate, intersection_spherical, midpoint,
    rhumb_bearing, rhumb_destination, rhumb_distance,
};

pub mod datum; // Helmert / classic-datum transforms

pub use datum::{DatumTransform, Helmert};
