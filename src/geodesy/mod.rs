//! Geodetic core: ellipsoids, ECEF, local tangent frames, and geodesic
//! computations.
//!
//! This module owns its math, with one exception: exact ellipsoidal (Karney)
//! geodesics are delegated to
//! [`geographiclib-rs`](https://docs.rs/geographiclib-rs) — the validated Rust
//! port of Karney's GeographicLib (and the same engine the `geo` crate uses) —
//! rather than re-deriving it. Everything else (haversine, ellipsoidal
//! rhumb/loxodrome, ECEF, ENU/NED/AER, datum/ellipsoid parameters tied to our
//! [`Crs`](crate::Crs)) is implemented here directly.
//!
//! ```
//! use geocoordinates::{Coordinate, geodesy};
//!
//! let paris = Coordinate::wgs84(48.8566, 2.3522)?;
//! let london = Coordinate::wgs84(51.5074, -0.1278)?;
//! let exact = geodesy::geodesic_distance(&paris, &london)?; // ellipsoidal, Karney
//! let rough = geodesy::haversine_distance(&paris, &london)?; // mean sphere
//! assert!((exact.meters() - 343_900.0).abs() < 1_000.0);
//! assert!((exact.meters() - rough.meters()).abs() < 0.0056 * exact.meters());
//! // Position producers return a coordinate in the input's CRS.
//! let next = geodesy::destination(&paris, geodesy::initial_bearing(&paris, &london)?, exact)?;
//! assert!(geodesy::geodesic_distance(&next, &london)?.meters() < 1e-3);
//! # Ok::<(), geocoordinates::Error>(())
//! ```

pub mod geodesic; // ellipsoidal (Karney) geodesics; re-exports the two below
mod rhumb; // rhumb/loxodrome on the datum ellipsoid
pub(crate) mod series; // Krüger n-series shared by rhumb lines and UTM
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
