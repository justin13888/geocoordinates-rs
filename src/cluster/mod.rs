//! Geodesic centroids, density and map clustering, and visits and trips (feature
//! `cluster`).
//!
//! Each tool answers a different question about a set of points:
//!
//! - [`centroid`]: the mean position of a set, across ±180° and at the poles.
//! - [`dbscan`]: places, as density clusters under a geodesic distance, with noise
//!   labelled. Order-independent.
//! - [`ClusterIndex`]: pins on a map, as a hierarchy with one level per Web Mercator
//!   zoom and a fixed pixel radius. It takes `insert` and `remove` without a rebuild.
//! - [`bucket_by_geohash`] and `bucket_by_h3`: a cheap grid grouping for sets above
//!   about 10⁶ points.
//! - [`segment_visits`] and [`segment_trips`]: stay points and trips from timestamped
//!   [`Fix`](crate::fix::Fix)es.
//!
//! Every item is reached flat at `cluster::NAME`. All of them need the points to
//! share a CRS with an ellipsoid; convert GCJ-02 / BD-09 to WGS-84 first.
//!
//! ```
//! use geocoordinates::{Coordinate, Length, LengthUnit};
//! use geocoordinates::cluster::{PointLabel, centroid, dbscan};
//!
//! let photos = [
//!     Coordinate::wgs84(48.8584, 2.2945)?,  // Paris
//!     Coordinate::wgs84(48.8606, 2.3376)?,  // Paris
//!     Coordinate::wgs84(35.6586, 139.7454)?, // Tokyo, alone
//! ];
//! let places = dbscan(&photos, Length::from_unit(5.0, LengthUnit::Kilometer), 2)?;
//! assert_eq!(places.clusters().len(), 1);
//! assert_eq!(places.labels()[2], PointLabel::Noise);
//!
//! let paris = centroid(&photos[..2])?;
//! assert!((paris.value().lon() - 2.316).abs() < 0.001);
//! # Ok::<(), geocoordinates::Error>(())
//! ```

// Every submodule is private: each public item is re-exported flat below, so
// `cluster::NAME` is its one path.
mod centroid; // the normalized mean of unit vectors
mod dbscan; // order-independent density clusters
mod grid; // cheap grid buckets
mod hierarchy; // the per-zoom map clusters
mod segment; // visits and trips
mod types; // the summary every grouping shares

pub use centroid::centroid;
pub use dbscan::{Dbscan, PointLabel, dbscan};
#[cfg(feature = "h3")]
pub use grid::bucket_by_h3;
pub use grid::{GridBucket, bucket_by_geohash};
pub use hierarchy::{ClusterIndex, ClusterIndexOptions, MAX_ZOOM, MapCluster};
pub use segment::{Trip, Visit, segment_trips, segment_visits};
pub use types::Cluster;
