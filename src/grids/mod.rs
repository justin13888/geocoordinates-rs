//! Projected and encoded grid systems.
//!
//! Currently: **Plus Codes**, **Geohash**, **Maidenhead** (see [`encoded`]),
//! and the **UTM/UPS** projections with their **MGRS** indexing.
//!
//! Encoding a point into a cell is exact, but **decoding** yields a cell with
//! spatial extent — those methods return [`Approx`](crate::Approx) carrying the
//! cell half-extent as the error bound.
//!
//! National grid projections (OSGB36, Swiss LV95, Dutch RD, …) are out of scope
//! here — reach them through the optional `proj` feature.

pub mod encoded;
pub mod mgrs; // UTM/UPS-backed military grid
pub mod utm; // transverse Mercator / polar stereographic

pub use encoded::{Geohash, Maidenhead, PlusCode};
pub use mgrs::Mgrs;
pub use utm::{Ups, Utm};
