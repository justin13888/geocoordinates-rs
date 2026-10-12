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
//!   series, Baidu's published Mercator fits).
//! - **Approximate** (lossy/iterative inverse, secret-algorithm inverse, or
//!   cell decode) never implement [`From`]. They return [`Approx<T>`], which
//!   carries the error bound, and their names carry a `_fast`/`_refined`
//!   suffix where multiple precisions exist.
//!
//! - **Exact and infallible** conversions between valid-by-construction types
//!   are plain `to_x()` methods, with [`From`] as sugar.
//!
//! ```
//! use geocoordinates::{Approx, Bd09, Gcj02, Wgs84};
//!
//! let wgs = Wgs84::new(39.915, 116.404)?; // validated once, here
//! let gcj: Gcj02 = wgs.to_gcj02(); // exact forward offset, infallible
//! let bd: Bd09 = wgs.try_to_bd09()?; // exact composition, fallible near a pole
//! let w: Approx<Wgs84> = gcj.to_wgs84_refined(); // approximate inverse
//! println!("{} ± {} m", w.lat(), w.max_error_m());
//! # let _ = bd;
//! # Ok::<(), geocoordinates::Error>(())
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
pub mod format; // DD/DMS/DDM and grid-string presentation
pub mod grids; // Plus Code, Geohash, Maidenhead, UTM/UPS, MGRS
pub mod parse; // free text, geo: URI, interchange formats, NMEA sensors

// --- Deferred: blocked on an external dependency (see STABILIZATION.md) ---
// Neither ships in 0.x, and neither has any source in-tree. A PROJ-backed EPSG
// long tail needs the system libproj C library; geoid (EGM) height conversion
// needs multi-megabyte grid data files. Their Cargo features stay commented out
// in Cargo.toml until a concrete consumer appears.

/// Discrete global grid indexing (H3 / S2), via external crates (optional).
///
/// ```
/// # #[cfg(feature = "h3")]
/// # {
/// use geocoordinates::Coordinate;
/// use geocoordinates::dgg::H3Cell;
///
/// let cell = H3Cell::encode(Coordinate::wgs84(40.7128, -74.006)?, 9)?;
/// let center = cell.decode(); // a cell covers an area: the center is approximate
/// assert!((center.lat() - 40.7128).abs() < 0.01 && center.max_error_m() > 0.0);
/// // Encoding needs a WGS-84 position.
/// assert!(H3Cell::encode(Coordinate::gcj02(40.7128, -74.006)?, 9).is_err());
/// # }
/// # Ok::<(), geocoordinates::Error>(())
/// ```
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
///
/// ```
/// use geocoordinates::prelude::*;
///
/// let a = Coordinate::wgs84(51.5074, -0.1278)?;
/// let b = Coordinate::wgs84(48.8566, 2.3522)?;
/// let km = geodesic_distance(&a, &b)?.to_unit(LengthUnit::Kilometer);
/// assert!((km - 343.9).abs() < 1.0);
/// let lat = Dms::from_degrees(a.lat(), Axis::Latitude)?;
/// assert_eq!((lat.degrees(), lat.hemisphere()), (51, Hemisphere::North));
/// // The glob import brings the crate's `Result<T>` alias, which shadows std's.
/// let bad: Result<Coordinate> = Coordinate::wgs84(0.0, 200.0);
/// assert!(bad.is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub mod prelude {
    pub use crate::{
        Accuracy, Approx, BaiduMercator, Bd09, Confidence, Coordinate, Crs, Error, Fix, Gcj02,
        Height, LatLon, Length, LengthUnit, RawSource, Result, Wgs84,
    };

    pub use crate::angle::{Axis, Ddm, Dms, Hemisphere};
    pub use crate::fix::{AxisOrder, DatumAmbiguity};
    pub use crate::geodesy::haversine_distance;

    pub use crate::convert::convert;
    pub use crate::format::{
        DecimalSeparator, FormatOptions, HemisphereStyle, Representation, SymbolStyle, format,
        format_fix,
    };
    pub use crate::geodesy::{
        Aer, Ecef, Ellipsoid, Enu, Intersection, Ned, along_track_distance_spherical,
        cross_track_distance_spherical, destination, final_bearing, geodesic_distance,
        initial_bearing, intermediate, intersection_spherical, midpoint, rhumb_bearing,
        rhumb_destination, rhumb_distance,
    };
    pub use crate::geodesy::{DatumTransform, Helmert};
    pub use crate::grids::{
        Geohash, Maidenhead, Mgrs, MgrsPrecision, PlusCode, Ups, Utm, UtmHemisphere,
    };
    pub use crate::parse::parse_coordinate;
}
