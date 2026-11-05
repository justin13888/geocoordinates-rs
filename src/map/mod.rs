//! Map-view math (feature `map`): Web Mercator, XYZ tiles and quadkeys, fitting a
//! camera to bounds, the clusters in view, GeoJSON layers, and the GCJ-02 shift
//! China's basemaps need.
//!
//! Every client of one library sees the same map: the Apple, Android and web apps
//! fit the same camera and draw the same cluster pins, because the math is here
//! and only the drawing is theirs. Nothing here renders, fetches tiles, or styles.
//!
//! - [`WebMercator`] and [`WorldPixel`]: EPSG:3857 meters and pixels at a zoom,
//!   clamped at ±[`MAX_LATITUDE`].
//! - [`Tile`] and [`tiles_covering`]: XYZ tiles, their bounds and quadkeys, and the
//!   tiles a box needs, bounded in number.
//! - [`fit_bounds`] and [`viewport_bounds`]: a [`Camera`] that shows a box in a
//!   [`Viewport`], and the box a camera shows, across the antimeridian too.
//! - [`clusters_in_view`]: a [`ClusterIndex`](crate::cluster::ClusterIndex) read at
//!   a fractional map zoom.
//! - `points_to_geojson` and `clusters_to_geojson` (with the `geojson` feature):
//!   layers in the shape supercluster and MapLibre's clustered sources emit.
//! - [`display_coordinate`] and [`tap_to_wgs84`]: a WGS-84 layer shifted for a
//!   GCJ-02 basemap, and a tap on one brought back, approximately.
//!
//! **Axis order.** Rust signatures take latitude first, as the rest of the crate
//! does; [`WorldPixel`] is `x` (east) then `y` (south), and GeoJSON output is
//! `[lon, lat]`, as RFC 7946 orders it.
//!
//! Every item is reached flat at `map::NAME`.
//!
//! ```
//! use geocoordinates::{Coordinate, index::GeoBounds};
//! use geocoordinates::map::{Tile, Viewport, fit_bounds, viewport_bounds};
//!
//! let photos = [Coordinate::wgs84(-17.7, 178.0)?, Coordinate::wgs84(-16.8, -179.9)?];
//! let bounds = GeoBounds::from_points(photos)?; // across the antimeridian
//! let view = Viewport::new(400.0, 300.0, 512.0)?;
//! let camera = fit_bounds(bounds, view, 20.0, 16.0)?;
//! assert!(camera.center().lon() > 178.0 || camera.center().lon() < -179.0);
//!
//! let shown = viewport_bounds(camera, view)?;
//! for photo in photos {
//!     assert!(shown.contains(photo)?);
//! }
//! assert_eq!(Tile::from_coordinate(photos[0], 4)?.quadkey(), "3111");
//! # Ok::<(), geocoordinates::Error>(())
//! ```

// Every submodule is private: each public item is re-exported flat below, so
// `map::NAME` is its one path.
mod display; // the GCJ-02 shift for China's basemaps
#[cfg(feature = "geojson")]
mod layer; // GeoJSON layers
mod mercator; // EPSG:3857 and world pixels
mod tile; // XYZ tiles and quadkeys
mod viewport; // cameras and views

#[cfg(test)]
mod tests;

pub use display::{DisplayDatum, display_coordinate, tap_to_wgs84};
#[cfg(feature = "geojson")]
pub use layer::{clusters_to_geojson, points_to_geojson};
pub use mercator::{MAX_LATITUDE, MAX_TILE_ZOOM, WebMercator, WorldPixel};
pub use tile::{Tile, tiles_covering};
pub use viewport::{Camera, Viewport, clusters_in_view, fit_bounds, viewport_bounds};
