//! Antimeridian-aware geographic bounds and a static geodesic spatial index
//! (feature `index`).
//!
//! [`GeoBounds`] is a latitude / longitude box whose west edge may lie east of
//! its east edge, which is how it crosses the antimeridian. [`SpatialIndex`]
//! answers radius and k-nearest queries over a fixed set of coordinates with
//! exact geodesic distances.
//!
//! Every item is reached flat at `index::NAME`.
//!
//! ```
//! use geocoordinates::{Coordinate, Length, LengthUnit};
//! use geocoordinates::index::{GeoBounds, SpatialIndex};
//!
//! let photos = [
//!     (Coordinate::wgs84(-17.7, 178.0)?, "Nadi"),
//!     (Coordinate::wgs84(-16.8, -179.9)?, "Taveuni"),
//! ];
//! // The box around Fiji is 2.1° wide, not 357.9°.
//! let bounds = GeoBounds::from_points(photos.iter().map(|(c, _)| *c))?;
//! assert!(bounds.crosses_antimeridian());
//!
//! let index = SpatialIndex::new(photos)?;
//! let center = Coordinate::wgs84(-16.9, -179.8)?;
//! let near = index.within(center, Length::from_unit(50.0, LengthUnit::Kilometer))?;
//! assert_eq!(near.len(), 1);
//! assert_eq!(*near[0].value(), "Taveuni");
//! # Ok::<(), geocoordinates::Error>(())
//! ```

// Every submodule is private: each public item is re-exported flat below, so
// `index::NAME` is its one path.
mod bounds; // the antimeridian- and pole-aware box
mod kdtree; // the k-d tree over unit vectors behind `SpatialIndex`
mod spatial; // the geodesic queries

pub use bounds::GeoBounds;
pub use spatial::{Neighbor, SpatialIndex};
