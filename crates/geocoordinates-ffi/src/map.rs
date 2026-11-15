//! Map-view math: Web Mercator, world pixels, XYZ tiles and quadkeys, camera
//! fitting, the clusters in view, GeoJSON layers, and the GCJ-02 display shift.
//!
//! Each valid-by-construction type crosses as a plain record with a
//! `<record>_validate`, and each method as a free function prefixed with the type.
//! The GeoJSON layers cross as JSON text.

use geocoordinates as gc;

use crate::{
    ApproxCoordinate, ClusterIndex, Coordinate, Crs, GeoBounds, GeoError, MapCluster,
    approx_coordinate,
};

/// A point on Web Mercator (EPSG:3857), in meters — mirror of
/// [`gc::map::WebMercator`]. The datum of the position it projects is carried, not
/// converted.
///
/// Plain data: [`web_mercator_validate`] checks a hand-built one.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct WebMercator {
    /// Easting, meters.
    pub x_m: f64,
    /// Northing, meters.
    pub y_m: f64,
    /// The reference system of the projected position.
    pub crs: Crs,
}

impl From<gc::map::WebMercator> for WebMercator {
    fn from(point: gc::map::WebMercator) -> Self {
        Self {
            x_m: point.x_m(),
            y_m: point.y_m(),
            crs: point.crs().into(),
        }
    }
}

impl TryFrom<WebMercator> for gc::map::WebMercator {
    type Error = gc::Error;

    fn try_from(point: WebMercator) -> Result<Self, gc::Error> {
        Self::new(point.x_m, point.y_m, point.crs.into())
    }
}

/// A position in world pixels at one zoom — mirror of [`gc::map::WorldPixel`]: `x`
/// east from the world's west edge, `y` south from its top edge.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct WorldPixel {
    /// Pixels east of the world's west edge.
    pub x: f64,
    /// Pixels south of the world's top edge.
    pub y: f64,
}

/// One XYZ tile — mirror of [`gc::map::Tile`]: column `x` east from the
/// antimeridian, row `y` south from the top edge.
///
/// Plain data: [`tile_validate`] checks a hand-built one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Tile {
    /// Zoom, 0 to 30.
    pub z: u8,
    /// Column, below `2^z`.
    pub x: u32,
    /// Row, below `2^z`.
    pub y: u32,
}

impl From<gc::map::Tile> for Tile {
    fn from(tile: gc::map::Tile) -> Self {
        Self {
            z: tile.z(),
            x: tile.x(),
            y: tile.y(),
        }
    }
}

impl TryFrom<Tile> for gc::map::Tile {
    type Error = gc::Error;

    fn try_from(tile: Tile) -> Result<Self, gc::Error> {
        Self::new(tile.z, tile.x, tile.y)
    }
}

/// The map view on screen, in pixels — mirror of [`gc::map::Viewport`]. `tile_px`
/// fixes what a zoom means: 512 for MapLibre, 256 for Google Maps, MapKit and
/// Leaflet.
///
/// Plain data: [`viewport_validate`] checks a hand-built one.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Viewport {
    /// Width, pixels.
    pub width_px: f64,
    /// Height, pixels.
    pub height_px: f64,
    /// Width of one tile, pixels.
    pub tile_px: f64,
}

impl TryFrom<Viewport> for gc::map::Viewport {
    type Error = gc::Error;

    fn try_from(view: Viewport) -> Result<Self, gc::Error> {
        Self::new(view.width_px, view.height_px, view.tile_px)
    }
}

/// Where a map looks — mirror of [`gc::map::Camera`].
///
/// Plain data: [`camera_validate`] checks a hand-built one.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Camera {
    /// The centre of the view.
    pub center: Coordinate,
    /// The fractional zoom, 0 to 30.
    pub zoom: f64,
}

impl From<gc::map::Camera> for Camera {
    fn from(camera: gc::map::Camera) -> Self {
        Self {
            center: camera.center().into(),
            zoom: camera.zoom(),
        }
    }
}

impl TryFrom<Camera> for gc::map::Camera {
    type Error = gc::Error;

    fn try_from(camera: Camera) -> Result<Self, gc::Error> {
        Self::new(camera.center.try_into()?, camera.zoom)
    }
}

/// The datum a basemap draws positions in — mirror of [`gc::map::DisplayDatum`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum DisplayDatum {
    /// WGS-84, which most basemaps draw.
    Wgs84,
    /// GCJ-02, which basemaps inside China draw.
    Gcj02,
}

impl From<DisplayDatum> for gc::map::DisplayDatum {
    fn from(datum: DisplayDatum) -> Self {
        match datum {
            DisplayDatum::Wgs84 => Self::Wgs84,
            DisplayDatum::Gcj02 => Self::Gcj02,
        }
    }
}

/// The latitude, in degrees, where Web Mercator's square world ends, about
/// 85.05112878: [`gc::map::MAX_LATITUDE`].
#[uniffi::export]
pub fn web_mercator_max_latitude_deg() -> f64 {
    gc::map::MAX_LATITUDE
}

/// The deepest tile and map zoom, 30: [`gc::map::MAX_TILE_ZOOM`].
#[uniffi::export]
pub fn tile_max_zoom() -> u8 {
    gc::map::MAX_TILE_ZOOM
}

/// Checks a hand-built [`WebMercator`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` with field `x_m` or `y_m` when it is non-finite or
/// beyond the square world's half-width.
#[uniffi::export]
pub fn web_mercator_validate(point: WebMercator) -> Result<(), GeoError> {
    gc::map::WebMercator::try_from(point)?;
    Ok(())
}

/// Projects `point`, clamping a latitude beyond ±85.05112878° to the map's edge.
///
/// # Errors
/// `GeoError::OutOfRange` for an invalid coordinate record.
#[uniffi::export]
pub fn web_mercator_from_coordinate(point: Coordinate) -> Result<WebMercator, GeoError> {
    Ok(gc::map::WebMercator::from_coordinate(point.try_into()?).into())
}

/// The geographic position of a projected point, on the CRS it carries.
///
/// # Errors
/// `GeoError::InvalidValue` for an invalid [`WebMercator`] record.
#[uniffi::export]
pub fn web_mercator_to_coordinate(point: WebMercator) -> Result<Coordinate, GeoError> {
    Ok(gc::map::WebMercator::try_from(point)?
        .to_coordinate()
        .into())
}

/// The pixel `point` is drawn at, at a fractional `zoom` on tiles `tile_px` wide.
///
/// # Errors
/// `GeoError::InvalidValue` with field `zoom` unless it is finite and in `[0, 30]`,
/// or `tile_px` unless it is finite and positive; `GeoError::OutOfRange` for an
/// invalid coordinate record.
#[uniffi::export]
pub fn world_pixel_from_coordinate(
    point: Coordinate,
    zoom: f64,
    tile_px: f64,
) -> Result<WorldPixel, GeoError> {
    let pixel = gc::map::WorldPixel::from_coordinate(point.try_into()?, zoom, tile_px)?;
    Ok(WorldPixel {
        x: pixel.x,
        y: pixel.y,
    })
}

/// The position drawn at `pixel`, on `crs`. `x` wraps around the world; `y` must lie
/// on it.
///
/// # Errors
/// `GeoError::InvalidValue` with field `zoom` or `tile_px` as
/// [`world_pixel_from_coordinate`] has them, field `x` when it is non-finite, and
/// field `y` unless it is finite and within the world's height.
#[uniffi::export]
pub fn world_pixel_to_coordinate(
    pixel: WorldPixel,
    zoom: f64,
    tile_px: f64,
    crs: Crs,
) -> Result<Coordinate, GeoError> {
    let pixel = gc::map::WorldPixel {
        x: pixel.x,
        y: pixel.y,
    };
    Ok(pixel.to_coordinate(zoom, tile_px, crs.into())?.into())
}

/// Checks a hand-built [`Tile`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` with field `z` above 30, or `x` / `y` at `2^z` or beyond.
#[uniffi::export]
pub fn tile_validate(tile: Tile) -> Result<(), GeoError> {
    gc::map::Tile::try_from(tile)?;
    Ok(())
}

/// The tile at zoom `z` that `point` is drawn in.
///
/// # Errors
/// `GeoError::InvalidValue` (field `z`) above 30; `GeoError::OutOfRange` for an
/// invalid coordinate record.
#[uniffi::export]
pub fn tile_from_coordinate(point: Coordinate, z: u8) -> Result<Tile, GeoError> {
    Ok(gc::map::Tile::from_coordinate(point.try_into()?, z)?.into())
}

/// The tile a Bing Maps quadkey names; its length is the zoom.
///
/// # Errors
/// `GeoError::InvalidGridRef` for a character other than `0`–`3`, or a key longer
/// than 30.
#[uniffi::export]
pub fn tile_from_quadkey(quadkey: String) -> Result<Tile, GeoError> {
    Ok(gc::map::Tile::from_quadkey(&quadkey)?.into())
}

/// The tile's Bing Maps quadkey.
///
/// # Errors
/// `GeoError::InvalidValue` for an invalid [`Tile`] record.
#[uniffi::export]
pub fn tile_quadkey(tile: Tile) -> Result<String, GeoError> {
    Ok(gc::map::Tile::try_from(tile)?.quadkey())
}

/// Checks a quadkey and returns its canonical form, which is the key itself: a
/// quadkey has one spelling.
///
/// # Errors
/// `GeoError::InvalidGridRef` as [`tile_from_quadkey`] has it.
#[uniffi::export]
pub fn quadkey_canonicalize(quadkey: String) -> Result<String, GeoError> {
    Ok(gc::map::Tile::from_quadkey(&quadkey)?.quadkey())
}

/// The tile's extent on `crs`, the datum of the basemap whose grid it belongs to.
///
/// # Errors
/// `GeoError::InvalidValue` for an invalid [`Tile`] record.
#[uniffi::export]
pub fn tile_bounds(tile: Tile, crs: Crs) -> Result<GeoBounds, GeoError> {
    Ok(gc::map::Tile::try_from(tile)?.bounds(crs.into()).into())
}

/// The tiles at zoom `z` that `bounds` touches, at most `max_tiles`, west to east
/// across the box and north to south within each column.
///
/// # Errors
/// `GeoError::InvalidValue` with field `z` above 30, field `max_tiles` when more
/// tiles cover the box, or for an invalid [`GeoBounds`] record.
#[uniffi::export]
pub fn tiles_covering(bounds: GeoBounds, z: u8, max_tiles: u32) -> Result<Vec<Tile>, GeoError> {
    let tiles = gc::map::tiles_covering(bounds.try_into()?, z, max_tiles as usize)?;
    Ok(tiles.into_iter().map(Into::into).collect())
}

/// Checks a hand-built [`Viewport`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` naming `width_px`, `height_px` or `tile_px` unless it is
/// finite and positive.
#[uniffi::export]
pub fn viewport_validate(viewport: Viewport) -> Result<(), GeoError> {
    gc::map::Viewport::try_from(viewport)?;
    Ok(())
}

/// Checks a hand-built [`Camera`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` (field `zoom`) unless it is finite and in `[0, 30]`;
/// `GeoError::OutOfRange` for an invalid centre.
#[uniffi::export]
pub fn camera_validate(camera: Camera) -> Result<(), GeoError> {
    gc::map::Camera::try_from(camera)?;
    Ok(())
}

/// The camera that shows all of `bounds` in `viewport`, at the deepest zoom up to
/// `max_zoom` that leaves `padding_px` free on every side; across the antimeridian
/// when the box crosses it, `max_zoom` for a single point, and zoom 0 for a box that
/// does not fit.
///
/// # Errors
/// `GeoError::InvalidValue` with field `padding_px` unless it is finite, not
/// negative and leaves part of the view, field `max_zoom` unless it is finite and in
/// `[0, 30]`, or for an invalid [`GeoBounds`] or [`Viewport`] record.
#[uniffi::export]
pub fn fit_bounds(
    bounds: GeoBounds,
    viewport: Viewport,
    padding_px: f64,
    max_zoom: f64,
) -> Result<Camera, GeoError> {
    let camera = gc::map::fit_bounds(
        bounds.try_into()?,
        viewport.try_into()?,
        padding_px,
        max_zoom,
    )?;
    Ok(camera.into())
}

/// The box `viewport` shows from `camera`, the inverse of [`fit_bounds`] with no
/// padding. A view reaching the map's top or bottom edge takes that edge to the
/// pole.
///
/// # Errors
/// `GeoError::InvalidValue` or `GeoError::OutOfRange` for an invalid [`Camera`] or
/// [`Viewport`] record.
#[uniffi::export]
pub fn viewport_bounds(camera: Camera, viewport: Viewport) -> Result<GeoBounds, GeoError> {
    Ok(gc::map::viewport_bounds(camera.try_into()?, viewport.try_into()?)?.into())
}

/// The clusters of `index` a map at the fractional `zoom` draws in `bounds`: level
/// `floor(zoom)`, capped at 22.
///
/// # Errors
/// `GeoError::InvalidValue` (field `zoom`) unless it is finite and in `[0, 30]`, or
/// for an invalid [`GeoBounds`] record; `GeoError::CrsMismatch` when `bounds` is on
/// another reference system than the points.
#[uniffi::export]
pub fn clusters_in_view(
    index: &ClusterIndex,
    bounds: GeoBounds,
    zoom: f64,
) -> Result<Vec<MapCluster>, GeoError> {
    let found = gc::map::clusters_in_view(&index.read(), bounds.try_into()?, zoom)?;
    Ok(found.iter().map(Into::into).collect())
}

/// Where a basemap drawing `display` must place the WGS-84 `point`: unchanged for
/// WGS-84, the exact forward offset for GCJ-02.
///
/// # Errors
/// `GeoError::CrsMismatch` unless `point` is on WGS-84; `GeoError::OutOfRange` for
/// an invalid coordinate record.
#[uniffi::export]
pub fn display_coordinate(
    point: Coordinate,
    display: DisplayDatum,
) -> Result<Coordinate, GeoError> {
    Ok(gc::map::display_coordinate(point.try_into()?, display.into())?.into())
}

/// The WGS-84 position under a tap a basemap reports in its own datum: exact for
/// WGS-84, approximate for GCJ-02 (`max_error_m` 0.5, the refined inverse).
///
/// # Errors
/// `GeoError::CrsMismatch` unless `tap` is on WGS-84 or GCJ-02;
/// `GeoError::OutOfRange` for an invalid coordinate record.
#[uniffi::export]
pub fn tap_to_wgs84(tap: Coordinate) -> Result<ApproxCoordinate, GeoError> {
    Ok(approx_coordinate(gc::map::tap_to_wgs84(tap.try_into()?)?))
}

/// A GeoJSON `FeatureCollection`, as JSON text, with one `Point` feature per point:
/// a top-level `id` and a `point_id` property holding its position in `points`,
/// coordinates `[lon, lat]` drawn in `display`.
///
/// # Errors
/// `GeoError::CrsMismatch` unless every point is on WGS-84; `GeoError::OutOfRange`
/// for an invalid coordinate record.
#[uniffi::export]
pub fn points_to_geojson(
    points: Vec<Coordinate>,
    display: DisplayDatum,
) -> Result<String, GeoError> {
    let points = points
        .into_iter()
        .map(gc::Coordinate::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(gc::map::points_to_geojson(&points, display.into())?)
}

/// The clusters [`clusters_in_view`] finds, as a GeoJSON `FeatureCollection` in JSON
/// text, in the shape supercluster and MapLibre's clustered sources emit: a cluster
/// has `id`, `cluster: true`, `cluster_id`, `point_count` and
/// `point_count_abbreviated`; a single point only a `point_id` property. Coordinates
/// are `[lon, lat]`, drawn in `display`.
///
/// This is the FFI form of [`gc::map::clusters_to_geojson`]: a [`MapCluster`] record
/// cannot be turned back into the core's checked cluster, so the view query and the
/// layer are one call.
///
/// # Errors
/// `GeoError::InvalidValue` (field `zoom`) unless it is finite and in `[0, 30]`, or
/// for an invalid [`GeoBounds`] record; `GeoError::CrsMismatch` when `bounds` is on
/// another reference system than the points, or the points are not on WGS-84.
#[uniffi::export]
pub fn clusters_in_view_geojson(
    index: &ClusterIndex,
    bounds: GeoBounds,
    zoom: f64,
    display: DisplayDatum,
) -> Result<String, GeoError> {
    let bounds = bounds.try_into()?;
    let core = index.read();
    let found = gc::map::clusters_in_view(&core, bounds, zoom)?;
    Ok(gc::map::clusters_to_geojson(&core, &found, display.into())?)
}
