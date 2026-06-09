//! # geocoordinates
//!
//! A feature-complete geospatial coordinate library for Rust — China datums
//! (GCJ-02/BD-09), geodetic transforms, geodesics, ingestion, and presentation.
//!
//! ## Conversion conventions
//!
//! The guarantee of every conversion is visible at the call site, without
//! reading docs:
//!
//! - **Exact & total** (deterministic, lossless within the model) implement
//!   [`From`]/[`Into`] and offer an inherent `to_x()` returning the bare type.
//!   If `let g: Gcj02 = wgs.into();` compiles, the conversion is exact.
//! - **Exact but fallible** (bad range / unparseable input) use
//!   [`TryFrom`]/[`TryInto`] and `try_to_x()`, returning [`Result`].
//! - **Approximate** (lossy/iterative inverse, secret-algorithm inverse, or
//!   cell decode) never implement [`From`]. They return [`Approx<T>`], which
//!   carries the error bound, and their names carry a `_fast`/`_refined`
//!   suffix where multiple precisions exist.
//!
//! ```ignore
//! let gcj: Gcj02 = wgs.into();              // exact forward offset
//! let bd:  Bd09  = wgs.to_bd09();           // exact composition
//! let w:   Approx<Wgs84> = gcj.to_wgs84_refined(); // approximate inverse
//! println!("{} ± {} m", w.lat(), w.max_error_m());
//! ```
//!
//! See `AGENTS.md` for design constraints. The implementation is in progress;
//! most bodies are `todo!()` placeholders pending review.

// TODO(impl): remove once the stub bodies below are implemented. While the API
// is `todo!()`-only, parameters and private fields are intentionally unused.
#![allow(unused_variables, dead_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod angle;
pub mod approx;
pub mod china;
pub mod convert;
pub mod coord;
pub mod error;
pub mod fix;
pub mod format;
pub mod geodesy;
pub mod grids;
pub mod parse;
pub mod units;

/// PROJ-backed transforms for the full EPSG/datum long tail (optional C dep).
#[cfg(feature = "proj")]
pub mod proj;

/// Geoid models for ellipsoidal ↔ orthometric height (optional, needs data).
#[cfg(feature = "geoid")]
pub mod height;

/// Discrete global grid indexing (H3 / S2), via external crates (optional).
#[cfg(any(feature = "h3", feature = "s2"))]
pub mod dgg;

// Crate root re-exports: the central types you cannot do much without. The
// broader common working set is in [`prelude`]; everything else is by path.
pub use approx::Approx;
pub use china::{Bd09, Gcj02, Wgs84};
pub use coord::{Coordinate, Crs, Height, LatLon};
pub use error::{Error, Result};
pub use fix::{Accuracy, Confidence, Fix, RawSource};
pub use units::{Length, LengthUnit};

/// Common imports for typical use: `use geocoordinates::prelude::*;`.
///
/// Brings in the canonical types plus the most-used angle, geodesy, grid,
/// formatting, parsing, and conversion items. Less-common items (PROJ, geoid,
/// DGG, raw Helmert/datum internals) remain reachable by their module path.
pub mod prelude {
    pub use crate::{
        Accuracy, Approx, Bd09, Confidence, Coordinate, Crs, Error, Fix, Gcj02, Height, LatLon,
        Length, LengthUnit, RawSource, Result, Wgs84,
    };

    pub use crate::angle::{Axis, Dd, Ddm, Dms, Hemisphere};
    pub use crate::convert::{can_convert, convert};
    pub use crate::fix::{AxisOrder, DatumAmbiguity};
    pub use crate::format::{FormatOptions, Representation, format, format_fix};
    pub use crate::geodesy::{
        Aer, DatumTransform, Ecef, Ellipsoid, Enu, Helmert, Ned, along_track_distance,
        cross_track_distance, destination, final_bearing, geodesic_distance, haversine_distance,
        initial_bearing, intermediate, intersection, midpoint, rhumb_bearing, rhumb_destination,
        rhumb_distance,
    };
    pub use crate::grids::{Geohash, Maidenhead, Mgrs, PlusCode, Ups, Utm};
    pub use crate::parse::parse_coordinate;
}
