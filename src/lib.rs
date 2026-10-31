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
//! - **Fallible construction from primitives** (bad range / unparseable input)
//!   is `new`, a `from_*` named constructor, or `parse`, returning [`Result`]
//!   (`Coordinate::new`, `Dms::from_degrees`, `"…".parse::<Geohash>()`).
//! - **Deterministic but fallible conversions between crate types** are
//!   `try_to_x()` / `try_from_x()`, with [`TryFrom`] as sugar, returning
//!   [`Result`]. These are exact, or carry a fixed error their docs state (a
//!   truncated projection series, Baidu's published Mercator fits).
//! - **Approximate** (lossy/iterative inverse, secret-algorithm inverse, or
//!   cell decode) never implement [`From`]. They return [`Approx<T>`], which
//!   carries the error bound, and their names carry a `_fast`/`_refined`
//!   suffix where multiple precisions exist.
//! - **Exact and infallible** conversions between valid-by-construction types
//!   are plain `to_x()` methods, with [`From`] as sugar.
//!
//! ```
//! use geocoordinates::Approx;
//! use geocoordinates::china::{Bd09, Gcj02, Wgs84};
//!
//! let wgs = Wgs84::new(39.915, 116.404)?; // validated once, here
//! let gcj: Gcj02 = wgs.to_gcj02(); // exact forward offset, infallible
//! let bd: Bd09 = wgs.try_to_bd09()?; // exact composition, fallible near a pole
//! let w: Approx<Wgs84> = gcj.to_wgs84_refined(); // approximate inverse
//! println!("{} ± {} m", w.value().lat(), w.max_error().meters());
//! # let _ = bd;
//! # Ok::<(), geocoordinates::Error>(())
//! ```
//!
//! ## One path per item
//!
//! Every public item has exactly one path, plus its [`prelude`] entry. The
//! crate root holds the core types ([`Coordinate`], [`Crs`], [`Height`],
//! [`LatLon`], [`Approx`], [`Length`], [`LengthUnit`], [`Error`],
//! [`ParseErrorKind`], [`Result`]);
//! every other item lives flat in its subsystem module: [`angle`], [`china`],
//! [`convert`], [`fix`], [`format`](mod@format), [`geodesy`], [`grids`],
//! [`parse`], `dgg` (behind the `h3` / `s2` features), and `index` (behind the
//! `index` feature). Implementation
//! submodules are private, so a type is never reachable at both
//! `geodesy::Ecef` and `geodesy::ecef::Ecef`.
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

mod approx;
mod coord;
mod error;
mod units;

pub mod angle;
pub mod china;
pub mod convert; // runtime CRS dispatch (China typed + classic-datum Helmert)
pub mod fix;
pub mod format; // DD/DMS/DDM and grid-string presentation
pub mod geodesy;
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
/// assert!((center.value().lat() - 40.7128).abs() < 0.01 && center.max_error().meters() > 0.0);
/// // Encoding needs a WGS-84 position.
/// assert!(H3Cell::encode(Coordinate::gcj02(40.7128, -74.006)?, 9).is_err());
/// # }
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(any(feature = "h3", feature = "s2"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "h3", feature = "s2"))))]
pub mod dgg;

#[cfg(feature = "index")]
#[cfg_attr(docsrs, doc(cfg(feature = "index")))]
pub mod index;

/// Shared test-only helpers and reference vectors (compiled under `cfg(test)`).
#[cfg(test)]
pub(crate) mod test_support;

// The crate root holds the core types, each re-exported from its private
// implementation module; it is their only path. The broader common working
// set is in [`prelude`]; every other item lives in its subsystem module.
pub use approx::Approx;
pub use coord::{Coordinate, Crs, LatLon};
pub use error::{Error, ParseErrorKind, Result};
pub use units::{Height, Length, LengthUnit};

/// Common imports for typical use: `use geocoordinates::prelude::*;`.
///
/// Brings in the core types, the China datums, the angle encodings, the
/// `Fix` metadata, and the geodesy, grid, formatting, parsing, and conversion
/// items. Anything outside this working set is reached by its module path.
///
/// The prelude deliberately leaves out the crate's [`Result`] alias, so a
/// glob import never shadows the standard library's `Result`:
///
/// ```
/// use geocoordinates::prelude::*;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let a = Coordinate::wgs84(51.5074, -0.1278)?;
///     let b = Coordinate::wgs84(48.8566, 2.3522)?;
///     let km = geodesic_distance(a, b)?.to_unit(LengthUnit::Kilometer);
///     assert!((km - 343.9).abs() < 1.0);
///     let lat = Dms::from_degrees(a.lat(), Axis::Latitude)?;
///     assert_eq!((lat.degrees(), lat.hemisphere()), (51, Hemisphere::North));
///     // Errors stay typed: an out-of-range longitude is `Error::OutOfRange`.
///     let bad: Result<Coordinate, Error> = Coordinate::wgs84(0.0, 200.0);
///     assert!(matches!(bad, Err(Error::OutOfRange { .. })));
///     Ok(())
/// }
/// ```
pub mod prelude {
    pub use crate::{Approx, Coordinate, Crs, Error, Height, LatLon, Length, LengthUnit};

    pub use crate::angle::{Axis, Ddm, Dms, Hemisphere};
    pub use crate::china::{BaiduMercator, Bd09, Gcj02, Wgs84};
    pub use crate::fix::{
        Accuracy, AccuracyKind, AxisOrder, Confidence, DatumAmbiguity, Fix, RawSource,
    };
    pub use crate::geodesy::haversine_distance;

    pub use crate::convert::convert;
    pub use crate::format::{
        DecimalSeparator, FormatOptions, HemisphereStyle, Representation, SymbolStyle, format,
        format_fix,
    };
    pub use crate::geodesy::{
        Aer, Ecef, Ellipsoid, Enu, GeodesicIntersection, Intersection, Ned, along_track_distance,
        along_track_distance_spherical, cross_track_distance, cross_track_distance_spherical,
        destination, final_bearing, geodesic_distance, initial_bearing, intermediate, intersection,
        intersection_spherical, midpoint, rhumb_bearing, rhumb_destination, rhumb_distance,
    };
    pub use crate::geodesy::{DatumTransform, Helmert};
    pub use crate::grids::{
        Geohash, Maidenhead, Mgrs, MgrsPrecision, PlusCode, Ups, Utm, UtmHemisphere,
    };
    #[cfg(feature = "index")]
    pub use crate::index::{GeoBounds, Neighbor, SpatialIndex};
    pub use crate::parse::{GridKind, ParseOptions, parse_coordinate, parse_coordinate_with};
}
