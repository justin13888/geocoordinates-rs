//! Projected and encoded grid systems.
//!
//! Currently: the encoded **Plus Codes** ([`PlusCode`]), **Geohash**
//! ([`Geohash`]), and **Maidenhead** ([`Maidenhead`]) locators, and the
//! **UTM/UPS** projections ([`Utm`], [`Ups`]) with their **MGRS** indexing
//! ([`Mgrs`]). Every item is reached flat at `grids::NAME`.
//!
//! Encoding a point into a cell is exact, but **decoding** yields a cell with
//! spatial extent — those methods return [`Approx`](crate::Approx) carrying a
//! bound on the distance from the decoded center to any point of the cell.
//! For the encoded locators that bound is the largest WGS-84 geodesic (Karney)
//! distance from the cell center to its four corners, a true upper bound on
//! the distance to any point of the cell.
//!
//! Each encoded code type is valid by construction: the only ways to obtain one
//! are its `encode` (which clamps the pole and wraps the antimeridian) and its
//! validating `TryFrom<&str>` / `TryFrom<String>` / `FromStr` (which serde
//! routes through too). `Display` and `AsRef<str>` yield the canonical string,
//! and parsing that string back returns an equal value.
//!
//! ```
//! use geocoordinates::Coordinate;
//! use geocoordinates::grids::{Geohash, utm_zone_for};
//!
//! let code = Geohash::encode(Coordinate::wgs84(42.6, -5.6)?, 5)?;
//! assert_eq!(code.to_string(), "ezs42");
//! assert_eq!("EZS42".parse::<Geohash>()?, code);
//! assert_eq!(utm_zone_for(&Coordinate::wgs84(42.6, -5.6)?)?, 30);
//! assert!("ezs4a".parse::<Geohash>().is_err()); // `a` is not a geohash digit
//! # Ok::<(), geocoordinates::Error>(())
//! ```
//!
//! National grid projections (OSGB36, Swiss LV95, Dutch RD, …) are out of scope
//! here. PROJ-backed support for them is deferred and not available in any
//! released feature (see
//! [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)).

// Every submodule is private: each public item is re-exported flat below, so
// `grids::NAME` is its one path.
mod encoded; // Plus Code, Geohash, Maidenhead
mod mgrs; // UTM/UPS-backed military grid
mod utm; // transverse Mercator / polar stereographic

pub use encoded::{Geohash, Maidenhead, PlusCode};
pub use mgrs::{Mgrs, MgrsPrecision};
pub use utm::{Ups, Utm, UtmHemisphere, utm_central_meridian_deg, utm_zone_for};
