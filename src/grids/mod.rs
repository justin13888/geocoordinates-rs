//! Projected and encoded grid systems.
//!
//! Currently: **Plus Codes**, **Geohash**, **Maidenhead** (see [`encoded`]),
//! and the **UTM/UPS** projections with their **MGRS** indexing.
//!
//! Encoding a point into a cell is exact, but **decoding** yields a cell with
//! spatial extent — those methods return [`Approx`](crate::Approx) carrying a
//! bound on the distance from the decoded center to any point of the cell.
//!
//! National grid projections (OSGB36, Swiss LV95, Dutch RD, …) are out of scope
//! here. PROJ-backed support for them is deferred and not available in any
//! released feature (see
//! [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)).

pub mod encoded;
pub mod mgrs; // UTM/UPS-backed military grid
pub mod utm; // transverse Mercator / polar stereographic

pub use encoded::{Geohash, Maidenhead, PlusCode};
pub use mgrs::Mgrs;
pub use utm::{Ups, Utm, UtmHemisphere};
