//! [`Tile`] and [`tiles_covering`], re-exported flat from [`map`](super).

use super::mercator::{MAX_TILE_ZOOM, lat_at, unit_x, unit_y};
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::index::GeoBounds;

/// One XYZ ("slippy map") tile: zoom `z`, column `x` counted east from the
/// antimeridian, row `y` counted south from the map's top edge, valid by
/// construction. It is the scheme OpenStreetMap, MapLibre, Google Maps and Bing
/// (through [`quadkey`](Self::quadkey)) share; TMS counts rows from the bottom.
///
/// ```
/// use geocoordinates::{Crs, map::Tile};
///
/// let tile = Tile::new(10, 486, 332)?;
/// assert_eq!(tile.quadkey(), "0313102310");
/// let bounds = tile.bounds(Crs::Wgs84);
/// assert_eq!((bounds.west(), bounds.east()), (-9.140625, -8.7890625));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "TileRepr"))]
pub struct Tile {
    z: u8,
    x: u32,
    y: u32,
}

/// The unchecked wire shape of a [`Tile`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct TileRepr {
    z: u8,
    x: u32,
    y: u32,
}

#[cfg(feature = "serde")]
impl TryFrom<TileRepr> for Tile {
    type Error = Error;

    fn try_from(repr: TileRepr) -> Result<Self> {
        Self::new(repr.z, repr.x, repr.y)
    }
}

/// The number of tiles across the world at zoom `z`.
const fn tiles_across(z: u8) -> u64 {
    1 << z
}

/// The column holding the unit-square position `x`, the east edge in the last one.
fn column(x: f64, n: u64) -> u64 {
    ((x * n as f64).floor() as u64).min(n - 1)
}

impl Tile {
    /// A tile from its zoom, column and row.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] with `field: "z"` above [`MAX_TILE_ZOOM`], and
    /// `field: "x"` or `"y"` at `2^z` or beyond.
    ///
    /// ```
    /// use geocoordinates::{Error, map::Tile};
    ///
    /// assert_eq!(Tile::new(1, 1, 0)?.x(), 1);
    /// assert!(matches!(Tile::new(1, 2, 0), Err(Error::InvalidValue { field: "x", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(z: u8, x: u32, y: u32) -> Result<Self> {
        if z > MAX_TILE_ZOOM {
            return Err(Error::InvalidValue {
                field: "z",
                detail: format!("{z} is above the deepest tile zoom {MAX_TILE_ZOOM}"),
            });
        }
        let n = tiles_across(z);
        for (field, value) in [("x", x), ("y", y)] {
            if u64::from(value) >= n {
                return Err(Error::InvalidValue {
                    field,
                    detail: format!("{value} is not below the {n} tiles across zoom {z}"),
                });
            }
        }
        Ok(Self { z, x, y })
    }

    /// The tile at zoom `z` that `point` is drawn in. A point on a shared edge
    /// belongs to the tile east or south of it, except on the world's east and
    /// bottom edges; a latitude beyond [`MAX_LATITUDE`](super::MAX_LATITUDE) is in
    /// the top or bottom row.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "z"`) above [`MAX_TILE_ZOOM`].
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, map::Tile};
    ///
    /// let denver = Coordinate::wgs84(40.0, -105.0)?;
    /// assert_eq!(Tile::from_coordinate(denver, 14)?, Tile::new(14, 3413, 6202)?);
    /// assert!(matches!(
    ///     Tile::from_coordinate(denver, 31),
    ///     Err(Error::InvalidValue { field: "z", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_coordinate(point: Coordinate, z: u8) -> Result<Self> {
        let probe = Self::new(z, 0, 0)?;
        let n = tiles_across(z);
        Ok(Self {
            x: column(unit_x(point.lon()), n) as u32,
            y: column(unit_y(point.lat()), n) as u32,
            ..probe
        })
    }

    /// Reads a Bing Maps quadkey: one digit `0`–`3` per zoom level, so its length
    /// is the zoom. The empty key is the zoom-0 tile.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidGridRef`] for a character other than `0`–`3`, or a key
    /// longer than [`MAX_TILE_ZOOM`].
    ///
    /// ```
    /// use geocoordinates::{Error, map::Tile};
    ///
    /// assert_eq!(Tile::from_quadkey("0313102310")?, Tile::new(10, 486, 332)?);
    /// assert!(matches!(Tile::from_quadkey("0314"), Err(Error::InvalidGridRef(_))));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_quadkey(quadkey: &str) -> Result<Self> {
        if quadkey.len() > usize::from(MAX_TILE_ZOOM) {
            return Err(Error::InvalidGridRef(format!(
                "quadkey {quadkey:?} is longer than the deepest tile zoom {MAX_TILE_ZOOM}"
            )));
        }
        let (mut x, mut y) = (0_u32, 0_u32);
        for digit in quadkey.chars() {
            let quadrant = digit.to_digit(4).ok_or_else(|| {
                Error::InvalidGridRef(format!("quadkey {quadkey:?} holds {digit:?}, not 0-3"))
            })?;
            x = (x << 1) | (quadrant & 1);
            y = (y << 1) | (quadrant >> 1);
        }
        Self::new(quadkey.len() as u8, x, y)
    }

    /// The tile's Bing Maps quadkey, one digit per zoom level.
    ///
    /// ```
    /// use geocoordinates::map::Tile;
    ///
    /// assert_eq!(Tile::new(3, 3, 5)?.quadkey(), "213");
    /// assert_eq!(Tile::new(0, 0, 0)?.quadkey(), "");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn quadkey(&self) -> String {
        (1..=self.z)
            .rev()
            .map(|level| {
                let bit = level - 1;
                let digit = ((self.x >> bit) & 1) | (((self.y >> bit) & 1) << 1);
                char::from(b'0' + digit as u8)
            })
            .collect()
    }

    /// The tile's extent on `crs`, the datum of the basemap whose grid it belongs
    /// to. The top row's north edge and the bottom row's south edge are
    /// ±[`MAX_LATITUDE`](super::MAX_LATITUDE), where the map ends, though a point
    /// beyond them is drawn on that edge.
    ///
    /// ```
    /// use geocoordinates::{Crs, map::{MAX_LATITUDE, Tile}};
    ///
    /// let world = Tile::new(0, 0, 0)?.bounds(Crs::Wgs84);
    /// assert_eq!((world.west(), world.east()), (-180.0, 180.0));
    /// assert!((world.north() - MAX_LATITUDE).abs() < 1e-12);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn bounds(&self, crs: Crs) -> GeoBounds {
        let n = tiles_across(self.z) as f64;
        let (x, y) = (f64::from(self.x), f64::from(self.y));
        GeoBounds::new(
            lat_at((y + 1.0) / n),
            x / n * 360.0 - 180.0,
            lat_at(y / n),
            (x + 1.0) / n * 360.0 - 180.0,
            crs,
        )
        .expect("a tile's edges are valid bounds")
    }

    /// The zoom.
    ///
    /// ```
    /// use geocoordinates::map::Tile;
    ///
    /// assert_eq!(Tile::new(5, 1, 2)?.z(), 5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn z(&self) -> u8 {
        self.z
    }

    /// The column, counted east from the antimeridian.
    ///
    /// ```
    /// use geocoordinates::map::Tile;
    ///
    /// assert_eq!(Tile::new(5, 1, 2)?.x(), 1);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn x(&self) -> u32 {
        self.x
    }

    /// The row, counted south from the map's top edge.
    ///
    /// ```
    /// use geocoordinates::map::Tile;
    ///
    /// assert_eq!(Tile::new(5, 1, 2)?.y(), 2);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn y(&self) -> u32 {
        self.y
    }
}

/// The tiles at zoom `z` that `bounds` touches, at most `max_tiles` of them, west to
/// east across the box (so across the antimeridian when it crosses) and north to
/// south within each column.
///
/// An east or south edge lying exactly on a tile edge does not take in the tile
/// beyond it. A box reaching past [`MAX_LATITUDE`](super::MAX_LATITUDE) takes the
/// top or bottom row, where such points are drawn. The CRS of `bounds` is taken as
/// the basemap's.
///
/// # Errors
///
/// [`Error::InvalidValue`] with `field: "z"` above [`MAX_TILE_ZOOM`], and
/// `field: "max_tiles"` when more tiles than `max_tiles` cover the box, before any
/// is built, so a large box at a deep zoom cannot ask for unbounded output.
///
/// ```
/// use geocoordinates::{Crs, Error, index::GeoBounds, map::{Tile, tiles_covering}};
///
/// // Across the antimeridian: the last column, then the first.
/// let fiji = GeoBounds::new(-20.0, 170.0, -10.0, -170.0, Crs::Wgs84)?;
/// assert_eq!(
///     tiles_covering(fiji, 2, 16)?,
///     [Tile::new(2, 3, 2)?, Tile::new(2, 0, 2)?],
/// );
/// assert!(matches!(
///     tiles_covering(fiji, 20, 1_000),
///     Err(Error::InvalidValue { field: "max_tiles", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn tiles_covering(bounds: GeoBounds, z: u8, max_tiles: usize) -> Result<Vec<Tile>> {
    Tile::new(z, 0, 0)?;
    let n = tiles_across(z);
    let first_column = (unit_x(bounds.west()) * n as f64).floor() as u64;
    let span = if bounds.crosses_antimeridian() {
        bounds.east() - bounds.west() + 360.0
    } else {
        bounds.east() - bounds.west()
    };
    let east = (unit_x(bounds.west()) + span / 360.0) * n as f64;
    // A box at least a world wide takes every column once.
    let columns = ((east.ceil() as u64).max(first_column + 1) - first_column).min(n);
    let top = column(unit_y(bounds.north()), n);
    let bottom_edge = unit_y(bounds.south()) * n as f64;
    let bottom = ((bottom_edge.ceil() as u64).saturating_sub(1)).clamp(top, n - 1);
    let rows = bottom - top + 1;

    let count = u128::from(columns) * u128::from(rows);
    if count > max_tiles as u128 {
        return Err(Error::InvalidValue {
            field: "max_tiles",
            detail: format!("{count} tiles cover the box at zoom {z}, above {max_tiles}"),
        });
    }
    let mut tiles = Vec::with_capacity(count as usize);
    for step in 0..columns {
        let x = ((first_column + step) % n) as u32;
        for y in top..=bottom {
            tiles.push(Tile { z, x, y: y as u32 });
        }
    }
    Ok(tiles)
}
