//! [`Viewport`], [`Camera`], [`fit_bounds`], [`viewport_bounds`] and
//! [`clusters_in_view`], re-exported flat from [`map`](super).

use super::mercator::{check_tile_px, check_zoom, lat_at, lon_at, unit_x, unit_y};
use crate::cluster::{ClusterIndex, MAX_ZOOM, MapCluster};
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::index::GeoBounds;

/// The map view on screen: its width and height, and the width of one tile, all in
/// pixels, valid by construction.
///
/// `tile_px` fixes what a zoom means: MapLibre counts zoom on 512-pixel tiles, and
/// Google Maps, Apple MapKit and Leaflet on 256-pixel ones, so the same view is one
/// zoom apart between them. Use the SDK's own.
///
/// ```
/// use geocoordinates::{Error, map::Viewport};
///
/// let phone = Viewport::new(390.0, 844.0, 256.0)?;
/// assert_eq!((phone.width_px(), phone.height_px(), phone.tile_px()), (390.0, 844.0, 256.0));
/// assert!(matches!(
///     Viewport::new(0.0, 844.0, 256.0),
///     Err(Error::InvalidValue { field: "width_px", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "ViewportRepr"))]
pub struct Viewport {
    width_px: f64,
    height_px: f64,
    tile_px: f64,
}

/// The unchecked wire shape of a [`Viewport`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct ViewportRepr {
    width_px: f64,
    height_px: f64,
    tile_px: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<ViewportRepr> for Viewport {
    type Error = Error;

    fn try_from(repr: ViewportRepr) -> Result<Self> {
        Self::new(repr.width_px, repr.height_px, repr.tile_px)
    }
}

impl Viewport {
    /// A view `width_px` by `height_px` on tiles `tile_px` wide.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] naming `width_px`, `height_px` or `tile_px` unless it
    /// is finite and positive.
    ///
    /// ```
    /// use geocoordinates::{Error, map::Viewport};
    ///
    /// assert!(Viewport::new(800.0, 600.0, 512.0).is_ok());
    /// assert!(matches!(
    ///     Viewport::new(800.0, 600.0, f64::NAN),
    ///     Err(Error::InvalidValue { field: "tile_px", .. })
    /// ));
    /// ```
    pub fn new(width_px: f64, height_px: f64, tile_px: f64) -> Result<Self> {
        for (field, value) in [("width_px", width_px), ("height_px", height_px)] {
            if !value.is_finite() || value <= 0.0 {
                return Err(Error::InvalidValue {
                    field,
                    detail: format!("{value} is not a finite, positive size"),
                });
            }
        }
        check_tile_px(tile_px)?;
        Ok(Self {
            width_px,
            height_px,
            tile_px,
        })
    }

    /// The width, in pixels.
    ///
    /// ```
    /// use geocoordinates::map::Viewport;
    ///
    /// assert_eq!(Viewport::new(800.0, 600.0, 512.0)?.width_px(), 800.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn width_px(&self) -> f64 {
        self.width_px
    }

    /// The height, in pixels.
    ///
    /// ```
    /// use geocoordinates::map::Viewport;
    ///
    /// assert_eq!(Viewport::new(800.0, 600.0, 512.0)?.height_px(), 600.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn height_px(&self) -> f64 {
        self.height_px
    }

    /// The width of one tile, in pixels.
    ///
    /// ```
    /// use geocoordinates::map::Viewport;
    ///
    /// assert_eq!(Viewport::new(800.0, 600.0, 512.0)?.tile_px(), 512.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn tile_px(&self) -> f64 {
        self.tile_px
    }

    /// The size of the world, in pixels, at `zoom`.
    fn world_px(&self, zoom: f64) -> f64 {
        self.tile_px * zoom.exp2()
    }
}

/// Where a map looks: the centre of the view and a fractional zoom, valid by
/// construction. It is what [`fit_bounds`] returns and [`viewport_bounds`] reads.
///
/// ```
/// use geocoordinates::{Coordinate, Error, map::Camera};
///
/// let camera = Camera::new(Coordinate::wgs84(48.86, 2.35)?, 11.5)?;
/// assert_eq!(camera.zoom(), 11.5);
/// assert!(matches!(
///     Camera::new(camera.center(), 31.0),
///     Err(Error::InvalidValue { field: "zoom", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "CameraRepr"))]
pub struct Camera {
    center: Coordinate,
    zoom: f64,
}

/// The unchecked wire shape of a [`Camera`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct CameraRepr {
    center: Coordinate,
    zoom: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<CameraRepr> for Camera {
    type Error = Error;

    fn try_from(repr: CameraRepr) -> Result<Self> {
        Self::new(repr.center, repr.zoom)
    }
}

impl Camera {
    /// A camera looking at `center` from a fractional `zoom`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "zoom"`) unless `zoom` is finite and in
    /// `[0, MAX_TILE_ZOOM]`.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, map::Camera};
    ///
    /// assert!(Camera::new(Coordinate::wgs84(0.0, 0.0)?, 0.0).is_ok());
    /// assert!(matches!(
    ///     Camera::new(Coordinate::wgs84(0.0, 0.0)?, f64::NAN),
    ///     Err(Error::InvalidValue { field: "zoom", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(center: Coordinate, zoom: f64) -> Result<Self> {
        check_zoom("zoom", zoom)?;
        Ok(Self { center, zoom })
    }

    /// The centre of the view.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, map::Camera};
    ///
    /// let paris = Coordinate::wgs84(48.86, 2.35)?;
    /// assert_eq!(Camera::new(paris, 3.0)?.center(), paris);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn center(&self) -> Coordinate {
        self.center
    }

    /// The fractional zoom.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, map::Camera};
    ///
    /// assert_eq!(Camera::new(Coordinate::wgs84(0.0, 0.0)?, 3.25)?.zoom(), 3.25);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn zoom(&self) -> f64 {
        self.zoom
    }
}

/// The span of `bounds` as a fraction of the world's width, and its west edge on
/// the unit square.
fn columns_of(bounds: &GeoBounds) -> (f64, f64) {
    let span = if bounds.crosses_antimeridian() {
        bounds.east() - bounds.west() + 360.0
    } else {
        bounds.east() - bounds.west()
    };
    (unit_x(bounds.west()), span / 360.0)
}

/// The camera that shows all of `bounds` in `viewport`, at the deepest zoom up to
/// `max_zoom` that leaves `padding_px` free on every side, centred on the box as the
/// map draws it (the middle of its Web Mercator extent, not its geodesic centre).
///
/// A box across the antimeridian is fitted across it, not around the world. A
/// single point, or a box too small to limit the zoom, gets `max_zoom`. A box that
/// does not fit even at zoom 0 gets zoom 0. A latitude beyond
/// [`MAX_LATITUDE`](super::MAX_LATITUDE) is fitted at the map's edge, where it is
/// drawn. The centre is on the CRS of `bounds`.
///
/// # Errors
///
/// [`Error::InvalidValue`] with `field: "padding_px"` unless it is finite, not
/// negative, and leaves some of the view on both axes, and `field: "max_zoom"`
/// unless it is finite and in `[0, MAX_TILE_ZOOM]`.
///
/// ```
/// use geocoordinates::{Crs, Error, index::GeoBounds, map::{Viewport, fit_bounds}};
///
/// let view = Viewport::new(512.0, 512.0, 512.0)?;
/// // Fiji, across the antimeridian: centred on it, 16 times narrower than the world.
/// let fiji = GeoBounds::new(-21.0, 176.0, -12.0, -161.5, Crs::Wgs84)?;
/// let camera = fit_bounds(fiji, view, 0.0, 18.0)?;
/// assert!((camera.center().lon() - -172.75).abs() < 1e-9);
/// assert!((camera.zoom() - 4.0).abs() < 1e-9);
///
/// assert!(matches!(
///     fit_bounds(fiji, view, 256.0, 18.0),
///     Err(Error::InvalidValue { field: "padding_px", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn fit_bounds(
    bounds: GeoBounds,
    viewport: Viewport,
    padding_px: f64,
    max_zoom: f64,
) -> Result<Camera> {
    check_zoom("max_zoom", max_zoom)?;
    let free_w = viewport.width_px - 2.0 * padding_px;
    let free_h = viewport.height_px - 2.0 * padding_px;
    if !padding_px.is_finite() || padding_px < 0.0 || free_w <= 0.0 || free_h <= 0.0 {
        return Err(Error::InvalidValue {
            field: "padding_px",
            detail: format!(
                "{padding_px} is not a finite, non-negative padding that leaves part of a \
                 {} × {} view",
                viewport.width_px, viewport.height_px
            ),
        });
    }
    let (west, span) = columns_of(&bounds);
    let (top, bottom) = (unit_y(bounds.north()), unit_y(bounds.south()));
    // A zero extent leaves its axis unlimited, so the other axis or `max_zoom` decides.
    let zoom_for = |free: f64, extent: f64| (free / (extent * viewport.tile_px)).log2();
    let zoom = zoom_for(free_w, span)
        .min(zoom_for(free_h, bottom - top))
        .min(max_zoom)
        .max(0.0);
    let center = Coordinate::new(
        lat_at((top + bottom) / 2.0),
        lon_at(west + span / 2.0),
        bounds.crs(),
    )?;
    Camera::new(center, zoom)
}

/// The box `viewport` shows from `camera`: the inverse of [`fit_bounds`] with no
/// padding, on the CRS of the camera's centre.
///
/// A view at least as wide as the world spans every longitude, `-180` to `180`. A
/// view reaching the map's top or bottom edge takes the north or south edge to the
/// pole, since points beyond [`MAX_LATITUDE`](super::MAX_LATITUDE) are drawn on that
/// edge and are in view. Otherwise the box crosses the antimeridian exactly when
/// the view does.
///
/// # Errors
///
/// None in practice: the result is built through [`GeoBounds::new`], whose checks
/// every edge computed here passes.
///
/// ```
/// use geocoordinates::{Coordinate, map::{Camera, Viewport, viewport_bounds}};
///
/// let view = Viewport::new(512.0, 256.0, 512.0)?;
/// let pacific = Camera::new(Coordinate::wgs84(0.0, 180.0)?, 3.0)?;
/// let bounds = viewport_bounds(pacific, view)?;
/// assert!(bounds.crosses_antimeridian());
/// assert_eq!((bounds.west(), bounds.east()), (157.5, -157.5));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn viewport_bounds(camera: Camera, viewport: Viewport) -> Result<GeoBounds> {
    let world = viewport.world_px(camera.zoom);
    let half_w = viewport.width_px / world / 2.0;
    let half_h = viewport.height_px / world / 2.0;
    let x = unit_x(camera.center.lon());
    let (west, east) = if half_w >= 0.5 {
        (-180.0, 180.0)
    } else {
        // East is read in (-180, 180], so a view ending on the antimeridian ends at 180.
        (lon_at(x - half_w), -lon_at(-(x + half_w)))
    };
    let y = unit_y(camera.center.lat());
    let north = if y - half_h <= 0.0 {
        90.0
    } else {
        lat_at(y - half_h)
    };
    let south = if y + half_h >= 1.0 {
        -90.0
    } else {
        lat_at(y + half_h)
    };
    GeoBounds::new(south, west, north, east, camera.center.crs())
}

/// The clusters of `index` a map at the fractional `zoom` draws in `bounds`: the
/// level `floor(zoom)`, capped at the index's deepest,
/// [`MAX_ZOOM`](crate::cluster::MAX_ZOOM), as supercluster rounds it. Pass the
/// [`viewport_bounds`] of the camera, and build the index with the view's
/// `tile_px`, so its pixel radius is the map's.
///
/// # Errors
///
/// [`Error::InvalidValue`] (`field: "zoom"`) unless `zoom` is finite and in
/// `[0, MAX_TILE_ZOOM]`, and [`Error::CrsMismatch`] when `bounds` is on another CRS
/// than the index's points.
///
/// ```
/// use geocoordinates::{Coordinate, Error};
/// use geocoordinates::cluster::ClusterIndex;
/// use geocoordinates::map::{Camera, Viewport, clusters_in_view, viewport_bounds};
///
/// let index = ClusterIndex::new(
///     [Coordinate::wgs84(48.85, 2.29)?, Coordinate::wgs84(48.86, 2.34)?, Coordinate::wgs84(35.7, 139.7)?],
///     Default::default(),
/// )?;
/// let paris = Camera::new(Coordinate::wgs84(48.86, 2.32)?, 9.6)?;
/// let bounds = viewport_bounds(paris, Viewport::new(800.0, 600.0, 512.0)?)?;
/// let pins = clusters_in_view(&index, bounds, paris.zoom())?;
/// assert_eq!(pins.len(), 1); // Tokyo is out of view, Paris is one pin at zoom 9
/// assert_eq!(pins[0].count(), 2);
/// assert!(matches!(
///     clusters_in_view(&index, bounds, -0.5),
///     Err(Error::InvalidValue { field: "zoom", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn clusters_in_view(
    index: &ClusterIndex,
    bounds: GeoBounds,
    zoom: f64,
) -> Result<Vec<MapCluster>> {
    check_zoom("zoom", zoom)?;
    // In range, so the floor fits a `u8`.
    let level = (zoom.floor() as u8).min(MAX_ZOOM);
    index.clusters(bounds, level)
}
