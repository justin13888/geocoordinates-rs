//! Geodetic core: ellipsoids, ECEF, local tangent frames, and geodesic
//! computations.
//!
//! Where [`geo`](https://docs.rs/geo) already implements a routine correctly
//! (Karney geodesics, haversine, rhumb lines, bearing, area), this module
//! reuses it directly rather than re-deriving the math. It owns only the parts
//! `geo` lacks first-class types for (ECEF, ENU/NED/AER, datum/ellipsoid
//! parameters tied to our [`Crs`](crate::Crs)).

pub mod datum;
pub mod ecef;
pub mod ellipsoid;
pub mod frames;
pub mod geodesic;

pub use datum::{DatumTransform, Helmert};
pub use ecef::Ecef;
pub use ellipsoid::Ellipsoid;
pub use frames::{Aer, Enu, Ned};
pub use geodesic::{
    along_track_distance, cross_track_distance, destination, final_bearing, geodesic_distance,
    haversine_distance, initial_bearing, intermediate, intersection, midpoint, rhumb_bearing,
    rhumb_destination, rhumb_distance,
};
