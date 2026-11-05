//! [`WebMercator`], [`WorldPixel`], [`MAX_LATITUDE`] and [`MAX_TILE_ZOOM`],
//! re-exported flat from [`map`](super), and the unit-square projection every map
//! routine shares.

use std::f64::consts::PI;

use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};

/// The latitude, in degrees, where Web Mercator's square world ends:
/// `atan(sinh(π))`, about 85.05112878°. Every projection here clamps a latitude
/// beyond it to the map's top or bottom edge.
///
/// ```
/// use geocoordinates::map::{MAX_LATITUDE, WebMercator};
/// use geocoordinates::Coordinate;
///
/// assert!((MAX_LATITUDE - 85.051_128_78).abs() < 1e-8);
/// // The North Pole lands on the top edge, at the same northing as the limit.
/// let pole = WebMercator::from_coordinate(Coordinate::wgs84(90.0, 0.0)?);
/// let edge = WebMercator::from_coordinate(Coordinate::wgs84(MAX_LATITUDE, 0.0)?);
/// assert_eq!(pole.y_m(), edge.y_m());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub const MAX_LATITUDE: f64 = 85.051_128_779_806_59;

/// The deepest zoom a [`Tile`](super::Tile) or a map view takes: 30, where a tile
/// is about 4 cm wide at the equator and a column index still fits a `u32`.
///
/// ```
/// use geocoordinates::{Error, map::{MAX_TILE_ZOOM, Tile}};
///
/// assert!(Tile::new(MAX_TILE_ZOOM, 0, 0).is_ok());
/// assert!(matches!(
///     Tile::new(MAX_TILE_ZOOM + 1, 0, 0),
///     Err(Error::InvalidValue { field: "z", .. })
/// ));
/// ```
pub const MAX_TILE_ZOOM: u8 = 30;

/// The WGS-84 semi-major axis, in meters: the radius of EPSG:3857's sphere.
const RADIUS_M: f64 = 6_378_137.0;

/// Half the width of the projected world, in meters: `π × 6 378 137`.
const HALF_WORLD_M: f64 = PI * RADIUS_M;

// `lon` / `lat` as fractions of the world's width and height (west and top edges 0),
// latitude clamped to the square. The cluster index's own projection, so the pins it
// places and the views and tiles drawn here share one formula.
pub(super) use crate::cluster::{world_x as unit_x, world_y as unit_y};

/// The longitude at the unit-square column `x`, wrapped into `[-180, 180)`.
pub(super) fn lon_at(x: f64) -> f64 {
    x.rem_euclid(1.0) * 360.0 - 180.0
}

/// The latitude at the unit-square row `y`, which lies in `[0, 1]`.
pub(super) fn lat_at(y: f64) -> f64 {
    (PI * (1.0 - 2.0 * y)).sinh().atan().to_degrees()
}

/// Checks a fractional map zoom: finite, in `[0, MAX_TILE_ZOOM]`.
pub(super) fn check_zoom(field: &'static str, zoom: f64) -> Result<()> {
    if zoom.is_finite() && (0.0..=f64::from(MAX_TILE_ZOOM)).contains(&zoom) {
        Ok(())
    } else {
        Err(Error::InvalidValue {
            field,
            detail: format!("{zoom} is not a finite zoom in [0, {MAX_TILE_ZOOM}]"),
        })
    }
}

/// Checks a tile width: finite and positive.
pub(super) fn check_tile_px(tile_px: f64) -> Result<()> {
    if tile_px.is_finite() && tile_px > 0.0 {
        Ok(())
    } else {
        Err(Error::InvalidValue {
            field: "tile_px",
            detail: format!("{tile_px} is not a finite, positive width"),
        })
    }
}

/// The width of the whole world in pixels at `zoom`, on tiles `tile_px` wide.
fn world_px(zoom: f64, tile_px: f64) -> Result<f64> {
    check_zoom("zoom", zoom)?;
    check_tile_px(tile_px)?;
    Ok(tile_px * zoom.exp2())
}

/// A point on Web Mercator (EPSG:3857): easting and northing in meters on a sphere
/// of the WGS-84 semi-major axis, origin at the equator on the prime meridian,
/// valid by construction.
///
/// **The datum is carried, not converted.** EPSG:3857 is defined on WGS-84, but
/// China's basemaps draw GCJ-02 positions through the same formula, so the point
/// keeps the [`Crs`] of the coordinate it came from, and
/// [`to_coordinate`](Self::to_coordinate) gives it back on that CRS.
///
/// **Exactness.** The forward and inverse are closed-form and agree to round-off
/// within [`MAX_LATITUDE`]. A latitude beyond it projects onto the edge, so the
/// inverse of a polar point is the edge latitude, not the pole.
///
/// ```
/// use geocoordinates::{Coordinate, map::WebMercator};
///
/// let denver = WebMercator::from_coordinate(Coordinate::wgs84(40.0, -105.0)?);
/// assert!((denver.x_m() - -11_688_546.533_293_726).abs() < 1e-6);
/// assert!((denver.y_m() - 4_865_942.279_503_176).abs() < 1e-6);
/// let back = denver.to_coordinate();
/// assert!((back.lat() - 40.0).abs() < 1e-12 && (back.lon() - -105.0).abs() < 1e-12);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "WebMercatorRepr"))]
pub struct WebMercator {
    x_m: f64,
    y_m: f64,
    crs: Crs,
}

/// The unchecked wire shape of a [`WebMercator`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct WebMercatorRepr {
    x_m: f64,
    y_m: f64,
    crs: Crs,
}

#[cfg(feature = "serde")]
impl TryFrom<WebMercatorRepr> for WebMercator {
    type Error = Error;

    fn try_from(repr: WebMercatorRepr) -> Result<Self> {
        Self::new(repr.x_m, repr.y_m, repr.crs)
    }
}

impl WebMercator {
    /// A projected point from its easting and northing, in meters on `crs`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] naming `x_m` or `y_m` when it is non-finite or
    /// beyond the square world's half-width, `π × 6 378 137` m.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, map::WebMercator};
    ///
    /// let origin = WebMercator::new(0.0, 0.0, Crs::Wgs84)?;
    /// assert_eq!(origin.to_coordinate().lon(), 0.0);
    /// assert!(matches!(
    ///     WebMercator::new(0.0, 3.0e7, Crs::Wgs84),
    ///     Err(Error::InvalidValue { field: "y_m", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(x_m: f64, y_m: f64, crs: Crs) -> Result<Self> {
        for (field, value) in [("x_m", x_m), ("y_m", y_m)] {
            if !value.is_finite() || value.abs() > HALF_WORLD_M {
                return Err(Error::InvalidValue {
                    field,
                    detail: format!(
                        "{value} is not a finite value in [-{HALF_WORLD_M}, {HALF_WORLD_M}]"
                    ),
                });
            }
        }
        Ok(Self { x_m, y_m, crs })
    }

    /// Projects `point`, clamping a latitude beyond [`MAX_LATITUDE`] to the edge.
    /// Its height is dropped.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, map::WebMercator};
    ///
    /// let mars = WebMercator::from_coordinate(Coordinate::gcj02(39.9, 116.4)?);
    /// assert_eq!(mars.crs(), Crs::Gcj02);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn from_coordinate(point: Coordinate) -> Self {
        // atanh(sin φ) is ln(tan(π/4 + φ/2)). Projected by magnitude, so the two
        // hemispheres mirror bit for bit.
        let lat = point.lat().abs().min(MAX_LATITUDE);
        let y_m = (RADIUS_M * lat.to_radians().sin().atanh())
            .min(HALF_WORLD_M)
            .copysign(point.lat());
        Self {
            x_m: RADIUS_M * point.lon().to_radians(),
            y_m,
            crs: point.crs(),
        }
    }

    /// The geographic position, on the CRS this point carries.
    ///
    /// ```
    /// use geocoordinates::{Crs, map::{MAX_LATITUDE, WebMercator}};
    ///
    /// let corner = WebMercator::new(-20_037_508.342_789_244, 20_037_508.342_789_244, Crs::Wgs84)?;
    /// let back = corner.to_coordinate();
    /// assert_eq!(back.lon(), -180.0);
    /// assert!((back.lat() - MAX_LATITUDE).abs() < 1e-9);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_coordinate(self) -> Coordinate {
        let lon = (self.x_m / RADIUS_M).to_degrees().clamp(-180.0, 180.0);
        let lat = (self.y_m / RADIUS_M).sinh().atan().to_degrees();
        Coordinate::new(lat, lon, self.crs)
            .expect("a point on the square world is a valid position")
    }

    /// The easting, in meters.
    ///
    /// ```
    /// use geocoordinates::{Crs, map::WebMercator};
    ///
    /// assert_eq!(WebMercator::new(1.5, 2.5, Crs::Wgs84)?.x_m(), 1.5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn x_m(&self) -> f64 {
        self.x_m
    }

    /// The northing, in meters.
    ///
    /// ```
    /// use geocoordinates::{Crs, map::WebMercator};
    ///
    /// assert_eq!(WebMercator::new(1.5, 2.5, Crs::Wgs84)?.y_m(), 2.5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn y_m(&self) -> f64 {
        self.y_m
    }

    /// The reference system of the geographic position this point projects.
    ///
    /// ```
    /// use geocoordinates::{Crs, map::WebMercator};
    ///
    /// assert_eq!(WebMercator::new(0.0, 0.0, Crs::Gcj02)?.crs(), Crs::Gcj02);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn crs(&self) -> Crs {
        self.crs
    }
}

/// A position in world pixels at one zoom: `x` east from the antimeridian's west
/// side, `y` south from the map's top edge, on a world `tile_px × 2^zoom` pixels
/// square. That is the pixel space MapLibre, Google Maps and Leaflet draw in.
///
/// Plain data: [`to_coordinate`](Self::to_coordinate) checks the values it reads.
///
/// ```
/// use geocoordinates::{Coordinate, map::WorldPixel};
///
/// // The centre of the world on one 256-pixel tile.
/// let centre = WorldPixel::from_coordinate(Coordinate::wgs84(0.0, 0.0)?, 0.0, 256.0)?;
/// assert_eq!(centre, WorldPixel { x: 128.0, y: 128.0 });
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldPixel {
    /// Pixels east of the world's west edge.
    pub x: f64,
    /// Pixels south of the world's top edge.
    pub y: f64,
}

impl WorldPixel {
    /// The pixel `point` is drawn at, at a fractional `zoom` on tiles `tile_px`
    /// wide. A latitude beyond [`MAX_LATITUDE`] lands on the top or bottom edge.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] with `field: "zoom"` unless `zoom` is finite and in
    /// `[0, MAX_TILE_ZOOM]`, and `field: "tile_px"` unless `tile_px` is finite and
    /// positive.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, map::WorldPixel};
    ///
    /// let pole = Coordinate::wgs84(90.0, 180.0)?;
    /// assert_eq!(WorldPixel::from_coordinate(pole, 1.0, 512.0)?, WorldPixel { x: 1024.0, y: 0.0 });
    /// assert!(matches!(
    ///     WorldPixel::from_coordinate(pole, -1.0, 512.0),
    ///     Err(Error::InvalidValue { field: "zoom", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_coordinate(point: Coordinate, zoom: f64, tile_px: f64) -> Result<Self> {
        let size = world_px(zoom, tile_px)?;
        Ok(Self {
            x: unit_x(point.lon()) * size,
            y: unit_y(point.lat()) * size,
        })
    }

    /// The position drawn at this pixel, on `crs`. The map repeats east and west,
    /// so `x` wraps; `y` must lie on the map.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] with `field: "zoom"` or `"tile_px"` as
    /// [`from_coordinate`](Self::from_coordinate) has them, `field: "x"` when `x`
    /// is non-finite, and `field: "y"` unless `y` is finite and within the world's
    /// height.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, map::WorldPixel};
    ///
    /// // One world east of the centre is the centre again.
    /// let wrapped = WorldPixel { x: 384.0, y: 128.0 }.to_coordinate(0.0, 256.0, Crs::Wgs84)?;
    /// assert_eq!((wrapped.lat(), wrapped.lon()), (0.0, 0.0));
    /// assert!(matches!(
    ///     WorldPixel { x: 0.0, y: -1.0 }.to_coordinate(0.0, 256.0, Crs::Wgs84),
    ///     Err(Error::InvalidValue { field: "y", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn to_coordinate(self, zoom: f64, tile_px: f64, crs: Crs) -> Result<Coordinate> {
        let size = world_px(zoom, tile_px)?;
        if !self.x.is_finite() {
            return Err(Error::InvalidValue {
                field: "x",
                detail: format!("{} is not finite", self.x),
            });
        }
        if !self.y.is_finite() || !(0.0..=size).contains(&self.y) {
            return Err(Error::InvalidValue {
                field: "y",
                detail: format!("{} is not on a world {size} pixels high", self.y),
            });
        }
        Coordinate::new(lat_at(self.y / size), lon_at(self.x / size), crs)
    }
}
