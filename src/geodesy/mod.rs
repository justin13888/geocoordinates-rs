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
//! Every item is reached flat at `geodesy::NAME`.
//!
//! ## Distance, bearing, and geodesic problems
//!
//! Ellipsoidal (Karney) [`geodesic_distance`], [`initial_bearing`] /
//! [`final_bearing`], and the position producers ([`destination`],
//! [`midpoint`], [`intermediate`]) run on the ellipsoid of the inputs' own
//! [`Crs`](crate::Crs) ([`Ellipsoid::for_crs`]): a Tokyo-datum pair is measured
//! on Bessel 1841, a NAD27 pair on Clarke 1866. GCJ-02 and BD-09 are obfuscated
//! renderings with no ellipsoid, so each of these rejects them with
//! [`Error::CrsMismatch`](crate::Error::CrsMismatch); convert them to WGS-84
//! first.
//!
//! The path routines run on that ellipsoid too: [`cross_track_distance`] /
//! [`along_track_distance`] find the foot of the perpendicular from a point to
//! a geodesic path, and [`intersection`] where two geodesic paths cross
//! ([`GeodesicIntersection`]). Each iterates (after Karney) to well under a
//! micrometre, and states its stopping rule.
//!
//! The great-circle routines on the IUGG mean sphere — [`haversine_distance`],
//! [`cross_track_distance_spherical`] / [`along_track_distance_spherical`], and
//! [`intersection_spherical`] — sit beside the ellipsoidal rhumb-line routines
//! ([`rhumb_distance`] / [`rhumb_bearing`] / [`rhumb_destination`]). Every
//! multi-point operation rejects mixed CRS values.
//!
//! Measurement functions take any [`LatLon`](crate::LatLon), so they work on
//! `Coordinate` and the per-datum newtypes alike. The producers take
//! `&Coordinate` and return a coordinate in the input's CRS, with its longitude
//! wrapped into `[-180, 180)` and no height (a surface path has no height to
//! carry).
//!
//! **Bearings at a pole.** Every direction leaves a pole southward, so a
//! bearing there is taken against the meridian of the coordinate's own
//! longitude (GeographicLib's convention): from `(90, λ)`, bearing 180° runs
//! down meridian `λ`, bearing 0° down meridian `λ + 180°`, and bearing `β` down
//! meridian `λ + 180° − β`; from `(−90, λ)`, bearing `β` runs up meridian
//! `λ + β`. The geodesic and rhumb-line functions share it, so each bearing
//! function's answer from a pole is the one its destination function inverts.
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

// Every submodule is private: each public item is re-exported flat below, so
// `geodesy::NAME` is its one path.
mod datum; // Helmert / classic-datum transforms
mod ecef; // ECEF geocentric coordinates
mod ellipsoid; // reference ellipsoid parameters
mod frames; // local tangent frames (ENU/NED/AER)
mod geodesic; // ellipsoidal (Karney) geodesics
mod rhumb; // rhumb/loxodrome on the datum ellipsoid
pub(crate) mod series; // Krüger n-series shared by rhumb lines and UTM
mod spherical; // haversine, cross/along-track, intersection on the mean sphere
mod track; // cross/along-track and intersection on the datum ellipsoid

pub(crate) use datum::HubRoute;
pub use datum::{DatumTransform, Helmert};
pub use ecef::Ecef;
pub use ellipsoid::Ellipsoid;
pub use frames::{Aer, Enu, Ned};
pub(crate) use geodesic::farthest_geodesic_m;
pub use geodesic::{
    destination, final_bearing, geodesic_distance, initial_bearing, intermediate, midpoint,
};
pub use rhumb::{rhumb_bearing, rhumb_destination, rhumb_distance};
pub use spherical::{
    Intersection, along_track_distance_spherical, cross_track_distance_spherical,
    haversine_distance, intersection_spherical,
};
pub use track::{GeodesicIntersection, along_track_distance, cross_track_distance, intersection};
