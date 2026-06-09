//! Projected and encoded grid systems.
//!
//! - **Projected:** UTM, UPS (poles) — see [`utm`].
//! - **Military grid:** MGRS strings — see [`mgrs`].
//! - **Encoded/discrete:** Geohash, Plus Codes (Open Location Code), and
//!   Maidenhead locators — see [`encoded`].
//!
//! Encoding a point into a cell is exact, but **decoding** yields a cell with
//! spatial extent — those methods return [`Approx`](crate::Approx) carrying the
//! cell half-width as the error bound.
//!
//! National grid projections (OSGB36, Swiss LV95, Dutch RD, …) are out of scope
//! here — reach them through the optional `proj` feature.

pub mod encoded;
pub mod mgrs;
pub mod utm;

pub use encoded::{Geohash, Maidenhead, PlusCode};
pub use mgrs::Mgrs;
pub use utm::{Ups, Utm};
