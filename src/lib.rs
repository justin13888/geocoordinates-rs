//! # geocoordinates
//!
//! Low-level geospatial coordinate primitives for Rust — China datums
//! (GCJ-02/BD-09), angle encodings, coordinate parsing/formatting, and geodesy
//! utilities. Following the UNIX philosophy, this crate abstracts *only*
//! geo-related complexity; higher-level concerns (e.g. EXIF extraction) live in
//! separate libraries that consume these primitives.
//!
//! ## Conversion conventions
//!
//! The guarantee of every conversion is visible at the call site, without
//! reading docs:
//!
//! - **Deterministic but fallible** (bad range / unparseable input) use
//!   [`TryFrom`]/[`TryInto`] and `try_to_x()`, returning [`Result`]. These are
//!   exact, or carry a fixed error their docs state (a truncated projection
//!   series, the ECEF inverse, Baidu's published Mercator fits).
//! - **Approximate** (lossy/iterative inverse, secret-algorithm inverse, or
//!   cell decode) never implement [`From`]. They return [`Approx<T>`], which
//!   carries the error bound, and their names carry a `_fast`/`_refined`
//!   suffix where multiple precisions exist.
//!
//! ```ignore
//! let gcj: Gcj02 = wgs.try_to_gcj02()?; // exact forward offset
//! let bd:  Bd09  = wgs.try_to_bd09()?; // exact composition
//! let w:   Approx<Wgs84> = gcj.try_to_wgs84_refined()?; // approximate inverse
//! println!("{} ± {} m", w.lat(), w.max_error_m());
//! ```
//!
//! See [AGENTS.md](https://github.com/justin13888/geocoordinates-rs/blob/master/AGENTS.md)
//! for design constraints, and
//! [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)
//! for the stabilization ledger and what remains before the 1.0 API freeze.
//!
//! ## Released surface
//!
//! Every module declared below is implemented, tested, and released. The
//! surface spans the core primitives, the China datums, formatting and
//! parsing, interchange and sensor ingestion, geodesy, projected and encoded
//! grids, and runtime CRS dispatch. `README.md` tabulates it by subsystem,
//! together with the FFI parity status of each entry.
//!
//! Two capabilities remain deliberately unreleased because they need a system
//! library or multi-megabyte external data: PROJ-backed EPSG transforms and
//! geoid height models.
//! [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)
//! records that decision.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod angle;
pub mod approx;
pub mod china;
pub mod coord;
pub mod error;
pub mod fix;
pub mod geodesy;
pub mod units;

pub mod convert; // runtime CRS dispatch (China typed + classic-datum Helmert)
pub mod format; // DD/DMS/DDM/Plus Code presentation
pub mod grids; // Plus Code, Geohash, Maidenhead, UTM/UPS, MGRS
pub mod parse; // free text, geo: URI, interchange formats, NMEA sensors

// --- Deferred: blocked on an external dependency (see STABILIZATION.md) ---
// Neither ships in 0.x, and neither has any source in-tree. A PROJ-backed EPSG
// long tail needs the system libproj C library; geoid (EGM) height conversion
// needs multi-megabyte grid data files. Their Cargo features stay commented out
// in Cargo.toml until a concrete consumer appears.

/// Discrete global grid indexing (H3 / S2), via external crates (optional).
#[cfg(any(feature = "h3", feature = "s2"))]
pub mod dgg;

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
/// `Fix` metadata, and the geodesy, grid, formatting, parsing, and conversion
/// items. Anything outside this working set is reached by its module path.
pub mod prelude {
    pub use crate::{
        Accuracy, Approx, BaiduMercator, Bd09, Confidence, Coordinate, Crs, Error, Fix, Gcj02,
        Height, LatLon, Length, LengthUnit, RawSource, Result, Wgs84,
    };

    pub use crate::angle::{Axis, Ddm, Dms, Hemisphere};
    pub use crate::fix::{AxisOrder, DatumAmbiguity};
    pub use crate::geodesy::haversine_distance;

    pub use crate::convert::{can_convert, convert};
    pub use crate::format::{FormatOptions, Representation, format, format_fix};
    pub use crate::geodesy::{
        Aer, Ecef, Ellipsoid, Enu, Ned, along_track_distance, cross_track_distance, destination,
        final_bearing, geodesic_distance, initial_bearing, intermediate, intersection, midpoint,
        rhumb_bearing, rhumb_destination, rhumb_distance,
    };
    pub use crate::geodesy::{DatumTransform, Helmert};
    pub use crate::grids::{
        Geohash, Maidenhead, Mgrs, MgrsPrecision, PlusCode, Ups, Utm, UtmHemisphere,
    };
    pub use crate::parse::parse_coordinate;
}
