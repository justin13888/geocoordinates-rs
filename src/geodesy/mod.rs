//! Geodetic core: ellipsoids, ECEF, local tangent frames, and geodesic
//! computations.
//!
//! This module owns its math, with one planned exception: exact ellipsoidal
//! (Karney) geodesics will be delegated to
//! [`geographiclib-rs`](https://docs.rs/geographiclib-rs) — the validated Rust
//! port of Karney's GeographicLib (and the same engine the `geo` crate uses) —
//! rather than re-deriving it. Everything else (haversine, rhumb/loxodrome,
//! ECEF, ENU/NED/AER, datum/ellipsoid parameters tied to our
//! [`Crs`](crate::Crs)) is implemented here directly.

pub mod geodesic;

pub use geodesic::haversine_distance;

pub mod ecef; // ECEF geocentric coordinates
pub mod ellipsoid; // reference ellipsoid parameters
pub mod frames; // local tangent frames (ENU/NED/AER)

pub use ecef::Ecef;
pub use ellipsoid::Ellipsoid;
pub use frames::{Aer, Enu, Ned};

// --- Deferred: released with the later geodesy milestones (see ROADMAP.md) ---
// pub mod datum;     // Helmert / classic-datum transforms
// pub use datum::{DatumTransform, Helmert};
// pub use geodesic::{
//     along_track_distance, cross_track_distance, destination, final_bearing, geodesic_distance,
//     initial_bearing, intermediate, intersection, midpoint, rhumb_bearing,
//     rhumb_destination, rhumb_distance,
// };
