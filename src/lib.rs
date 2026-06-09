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
//! See `AGENTS.md` for design constraints, and `ROADMAP.md` for the staged
//! release plan.
//!
//! ## Released surface
//!
//! This crate is shipped incrementally. The modules declared below are the
//! implemented, working surface; the remainder of the planned API
//! (full geodesy, grids, ingestion, formatting, runtime conversion, and the
//! optional `proj`/`geoid`/`dgg` integrations) is commented out and lands one
//! release at a time. `ROADMAP.md` tracks the order.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod angle;
pub mod approx;
pub mod china;
pub mod coord;
pub mod error;
pub mod fix;
pub mod geodesy;
pub mod units;

// --- Not yet released (see ROADMAP.md) ---
// Each module below is implemented behind `todo!()` stubs and is uncommented one
// release at a time. The stub source stays on disk; only its `mod` declaration,
// re-exports, prelude entries, and Cargo feature are commented out for now.
//
// pub mod convert; // 0.6 — runtime CRS dispatch
// pub mod format; // 0.11 — locale-aware formatting
// pub mod grids; // 0.7–0.9 — UTM/UPS, MGRS, Geohash/Plus Code/Maidenhead
// pub mod parse; // 0.10+ — free-text, interchange, sensor ingestion
//
// /// PROJ-backed transforms for the full EPSG/datum long tail (optional C dep).
// #[cfg(feature = "proj")]
// pub mod proj;
//
// /// Geoid models for ellipsoidal ↔ orthometric height (optional, needs data).
// #[cfg(feature = "geoid")]
// pub mod height;
//
// /// Discrete global grid indexing (H3 / S2), via external crates (optional).
// #[cfg(any(feature = "h3", feature = "s2"))]
// pub mod dgg;

/// Shared test-only helpers and reference vectors (compiled under `cfg(test)`).
#[cfg(test)]
pub(crate) mod test_support;

// Crate root re-exports: the central types you cannot do much without. The
// broader common working set is in [`prelude`]; everything else is by path.
pub use approx::Approx;
pub use china::{BaiduMercator, Bd09, Gcj02, Wgs84};
pub use coord::{Coordinate, Crs, Height, LatLon};
pub use error::{Error, Result};
pub use fix::{Accuracy, Confidence, Fix, RawSource};
pub use units::{Length, LengthUnit};

/// Common imports for typical use: `use geocoordinates::prelude::*;`.
///
/// Brings in the canonical types, the China datums, the angle encodings, the
/// `Fix` metadata, and `haversine_distance`. As later releases land (see
/// `ROADMAP.md`), this set grows to include the geodesy, grid, formatting,
/// parsing, and conversion items.
pub mod prelude {
    pub use crate::{
        Accuracy, Approx, BaiduMercator, Bd09, Confidence, Coordinate, Crs, Error, Fix, Gcj02,
        Height, LatLon, Length, LengthUnit, RawSource, Result, Wgs84,
    };

    pub use crate::angle::{Axis, Dd, Ddm, Dms, Hemisphere};
    pub use crate::fix::{AxisOrder, DatumAmbiguity};
    pub use crate::geodesy::haversine_distance;

    // Grows with each release (see ROADMAP.md):
    // pub use crate::convert::{can_convert, convert};
    // pub use crate::format::{FormatOptions, Representation, format, format_fix};
    // pub use crate::geodesy::{
    //     Aer, DatumTransform, Ecef, Ellipsoid, Enu, Helmert, Ned, along_track_distance,
    //     cross_track_distance, destination, final_bearing, geodesic_distance,
    //     initial_bearing, intermediate, intersection, midpoint, rhumb_bearing,
    //     rhumb_destination, rhumb_distance,
    // };
    // pub use crate::grids::{Geohash, Maidenhead, Mgrs, PlusCode, Ups, Utm};
    // pub use crate::parse::parse_coordinate;
}
