//! Location privacy: deterministic coarsening and exclusion zones (feature
//! `privacy`).
//!
//! Sharing a photo's exact position can reveal a home or a school. Rounding
//! decimal degrees by hand states no precision and shrinks in longitude toward
//! the poles, and random jitter leaks on repeated release: averaging many
//! jittered copies of one point recovers it. This module does neither.
//!
//! - [`coarsen`] snaps a position to the centre of the grid cell holding it, at
//!   a stated [`Precision`]. The result is a function of the cell alone, so every
//!   release of every point in the cell is the same point and repeated sharing
//!   narrows nothing down. Coarsening a coarsened point changes nothing.
//! - [`ExclusionZone`] marks a place (a geodesic circle, or a [`GeoBounds`]
//!   box) whose fixes are never shared exactly, and [`apply`] strips or
//!   coarsens a fix inside any zone, as its [`ZoneAction`] says.
//!
//! **No randomness.** Nothing here draws a random number: the same input always
//! gives the same output, on every platform.
//!
//! **What a coarsened fix claims.** Its accuracy is a
//! [`Bound`](crate::fix::AccuracyKind::Bound) equal to the cell's circumradius:
//! the largest WGS-84 geodesic distance from the cell centre to the cell's
//! boundary vertices, so the input lies no farther from the result. Its
//! [`RawSource`] starts with `privacy:` and names the cell, never the input. It
//! carries no height.
//!
//! Every grid here is defined on WGS-84 latitude and longitude, so coarsening a
//! coordinate on another datum is refused with [`Error::CrsMismatch`]; convert
//! it with [`convert`](crate::convert::convert) first.
//!
//! Every item is reached flat at `privacy::NAME`.
//!
//! ```
//! use geocoordinates::fix::{AccuracyKind, Fix};
//! use geocoordinates::geodesy::geodesic_distance;
//! use geocoordinates::privacy::{ExclusionZone, Precision, ZoneAction, apply, coarsen};
//! use geocoordinates::{Coordinate, Length};
//!
//! let home = Coordinate::wgs84(51.5007, -0.1246)?;
//!
//! // About a kilometre: the finest geohash cell at least 1 km tall.
//! let precision = Precision::at_least(Length::from_meters(1_000.0))?;
//! let shared = coarsen(home, precision)?;
//! let bound = shared.accuracy.expect("a coarsened fix states its bound");
//! assert_eq!(bound.kind(), AccuracyKind::Bound);
//! assert!(geodesic_distance(home, shared.coord)? <= bound.horizontal().expect("horizontal"));
//! // Deterministic and idempotent: the cell centre coarsens to itself.
//! assert_eq!(coarsen(shared.coord, precision)?, shared);
//!
//! // A 200 m zone around home strips every fix inside it.
//! let zones = [ExclusionZone::from_circle(home, Length::from_meters(200.0))?];
//! assert_eq!(apply(Fix::from_coord(home), &zones, ZoneAction::Strip)?, None);
//! let away = Fix::from_coord(Coordinate::wgs84(48.8584, 2.2945)?);
//! assert_eq!(apply(away.clone(), &zones, ZoneAction::Strip)?, Some(away));
//! # Ok::<(), geocoordinates::Error>(())
//! ```
//!
//! [`GeoBounds`]: crate::index::GeoBounds

mod zone;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod zone_tests;

pub use self::zone::{ExclusionZone, ZoneAction, apply};

use crate::angle::wrap_longitude;
use crate::approx::Approx;
use crate::coord::Coordinate;
#[cfg(feature = "h3")]
use crate::dgg::H3Cell;
#[cfg(feature = "s2")]
use crate::dgg::S2CellId;
use crate::error::{Error, Result};
use crate::fix::{Accuracy, AccuracyKind, Confidence, Fix, RawSource};
// The longest geohash `coarsen` snaps to is the longest `Geohash::encode`
// accepts, so the two share one constant and cannot drift apart.
use crate::grids::{GEOHASH_MAX_LENGTH, Geohash};
use crate::units::Length;

/// The finest H3 resolution.
#[cfg(feature = "h3")]
const H3_MAX_RESOLUTION: u8 = 15;

/// The finest S2 level.
#[cfg(feature = "s2")]
const S2_MAX_LEVEL: u8 = 30;

/// Meters per degree of latitude along a WGS-84 meridian at the equator,
/// rounded down: `a (1 - e²) π / 180` is 110 574.27 m. The meridian's radius of
/// curvature grows toward the poles, so no degree of latitude anywhere is
/// shorter, and a cell height computed with it is a lower bound everywhere.
const MIN_METERS_PER_DEGREE_LATITUDE: f64 = 110_574.0;

/// The discrete grid a [`Precision`] snaps to.
///
/// [`Geohash`](Self::Geohash) is always available; `H3` needs the
/// `h3` feature and `S2` the `s2` feature, so a grid whose backing
/// crate is not compiled in cannot be named.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every
/// variant, so adding one here is a deliberate, compile-forcing change on both
/// sides.
///
/// ```
/// use geocoordinates::privacy::{Precision, SnapGrid};
///
/// assert_eq!(Precision::geohash(6)?.grid(), SnapGrid::Geohash);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SnapGrid {
    /// Geohash cells, levels (code lengths) 1 to 12: latitude / longitude
    /// rectangles whose width shrinks toward the poles.
    Geohash,
    /// Uber H3 hexagons (and twelve pentagons per resolution), resolutions 0
    /// to 15, of near-uniform area worldwide.
    #[cfg(feature = "h3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "h3")))]
    H3,
    /// Google S2 quadrilateral cells, levels 0 to 30, whose area varies by
    /// about a factor of two worldwide.
    #[cfg(feature = "s2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "s2")))]
    S2,
}

/// How coarse [`coarsen`] makes a position: a grid and a level on it.
///
/// Valid by construction: every constructor rejects a level the grid does not
/// define, and under the `serde` feature deserialization validates the same
/// way. Build one from a size with [`at_least`](Self::at_least), or name the
/// grid and level with [`geohash`](Self::geohash), `h3`, `s2`, or
/// [`new`](Self::new).
///
/// ```
/// use geocoordinates::privacy::{Precision, SnapGrid};
/// use geocoordinates::Length;
///
/// let precision = Precision::at_least(Length::from_meters(5_000.0))?;
/// assert_eq!((precision.grid(), precision.level()), (SnapGrid::Geohash, 4));
/// assert_eq!(Precision::new(SnapGrid::Geohash, 4)?, precision);
/// assert!(Precision::geohash(13).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "PrecisionRepr"))]
pub struct Precision {
    grid: SnapGrid,
    level: u8,
}

/// The unchecked wire shape of a [`Precision`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct PrecisionRepr {
    grid: SnapGrid,
    level: u8,
}

#[cfg(feature = "serde")]
impl TryFrom<PrecisionRepr> for Precision {
    type Error = Error;

    fn try_from(repr: PrecisionRepr) -> Result<Self> {
        Self::new(repr.grid, repr.level)
    }
}

impl Precision {
    /// A precision of `level` on `grid`.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] when the grid does not define `level`: field
    /// `geohash length` outside `1..=12`, `H3 resolution` above 15, or
    /// `S2 level` above 30.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::privacy::{Precision, SnapGrid};
    ///
    /// assert_eq!(Precision::new(SnapGrid::Geohash, 5)?.level(), 5);
    /// assert!(matches!(
    ///     Precision::new(SnapGrid::Geohash, 0),
    ///     Err(Error::InvalidValue { field: "geohash length", .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn new(grid: SnapGrid, level: u8) -> Result<Self> {
        let (field, range) = match grid {
            SnapGrid::Geohash => ("geohash length", 1..=GEOHASH_MAX_LENGTH),
            #[cfg(feature = "h3")]
            SnapGrid::H3 => ("H3 resolution", 0..=H3_MAX_RESOLUTION),
            #[cfg(feature = "s2")]
            SnapGrid::S2 => ("S2 level", 0..=S2_MAX_LEVEL),
        };
        if !range.contains(&level) {
            return Err(Error::InvalidValue {
                field,
                detail: format!(
                    "must be in {}..={}, got {level}",
                    range.start(),
                    range.end()
                ),
            });
        }
        Ok(Self { grid, level })
    }

    /// A geohash precision of `length` characters, 1 (coarsest) to 12.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] (field `geohash length`) outside `1..=12`.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::privacy::Precision;
    ///
    /// assert_eq!(Precision::geohash(7)?.level(), 7);
    /// assert!(matches!(
    ///     Precision::geohash(13),
    ///     Err(Error::InvalidValue { field: "geohash length", .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn geohash(length: u8) -> Result<Self> {
        Self::new(SnapGrid::Geohash, length)
    }

    /// An H3 precision at `resolution`, 0 (coarsest) to 15.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] (field `H3 resolution`) above 15.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::privacy::{Precision, SnapGrid};
    ///
    /// assert_eq!(Precision::h3(7)?.grid(), SnapGrid::H3);
    /// assert!(matches!(
    ///     Precision::h3(16),
    ///     Err(Error::InvalidValue { field: "H3 resolution", .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    #[cfg(feature = "h3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "h3")))]
    pub fn h3(resolution: u8) -> Result<Self> {
        Self::new(SnapGrid::H3, resolution)
    }

    /// An S2 precision at `level`, 0 (coarsest) to 30.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] (field `S2 level`) above 30.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::privacy::{Precision, SnapGrid};
    ///
    /// assert_eq!(Precision::s2(13)?.grid(), SnapGrid::S2);
    /// assert!(matches!(
    ///     Precision::s2(31),
    ///     Err(Error::InvalidValue { field: "S2 level", .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    #[cfg(feature = "s2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "s2")))]
    pub fn s2(level: u8) -> Result<Self> {
        Self::new(SnapGrid::S2, level)
    }

    /// The finest geohash precision whose cells are at least `size` tall.
    ///
    /// A geohash cell's north–south height depends only on its length, so this
    /// mapping holds everywhere: every cell at the returned length is at least
    /// `size` tall, measured along the meridian. Its east–west width is at least
    /// its height at the equator and shrinks with the cosine of the latitude
    /// toward the poles. The guaranteed heights, from
    /// [`min_cell_height`](Self::min_cell_height):
    ///
    /// | Length | Height at least | Length | Height at least |
    /// |---|---|---|---|
    /// | 1 | 4 975 830 m | 7 | 151.8 m |
    /// | 2 | 621 978.7 m | 8 | 18.98 m |
    /// | 3 | 155 494.6 m | 9 | 4.745 m |
    /// | 4 | 19 436.8 m | 10 | 0.593 m |
    /// | 5 | 4 859.2 m | 11 | 0.148 m |
    /// | 6 | 607.4 m | 12 | 0.0185 m |
    ///
    /// Only geohash is mapped from a size: H3 and S2 cells vary in size and
    /// shape by position, so name their level with `h3` or `s2` instead.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] (field `size`) when `size` is negative, NaN or
    /// infinite, or taller than the coarsest geohash cell (length 1).
    ///
    /// ```
    /// use geocoordinates::privacy::Precision;
    /// use geocoordinates::{Error, Length};
    ///
    /// let precision = Precision::at_least(Length::from_meters(150.0))?;
    /// assert_eq!(precision.level(), 7);
    /// assert!(precision.min_cell_height().expect("geohash") >= Length::from_meters(150.0));
    /// assert!(matches!(
    ///     Precision::at_least(Length::from_meters(1.0e7)),
    ///     Err(Error::InvalidValue { field: "size", .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn at_least(size: Length) -> Result<Self> {
        let meters = size.meters();
        if !(meters.is_finite() && meters >= 0.0) {
            return Err(Error::InvalidValue {
                field: "size",
                detail: format!("must be finite and nonnegative, got {meters} m"),
            });
        }
        (1..=GEOHASH_MAX_LENGTH)
            .rev()
            .find(|&length| geohash_min_height_m(length) >= meters)
            .map(|length| Self {
                grid: SnapGrid::Geohash,
                level: length,
            })
            .ok_or_else(|| Error::InvalidValue {
                field: "size",
                detail: format!(
                    "{meters} m is taller than the coarsest geohash cell ({} m)",
                    geohash_min_height_m(1)
                ),
            })
    }

    /// The grid this precision snaps to.
    ///
    /// ```
    /// use geocoordinates::privacy::{Precision, SnapGrid};
    ///
    /// assert_eq!(Precision::geohash(3)?.grid(), SnapGrid::Geohash);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn grid(self) -> SnapGrid {
        self.grid
    }

    /// The level on [`grid`](Self::grid): the geohash length, H3 resolution, or
    /// S2 level.
    ///
    /// ```
    /// use geocoordinates::privacy::Precision;
    ///
    /// assert_eq!(Precision::geohash(3)?.level(), 3);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn level(self) -> u8 {
        self.level
    }

    /// For a geohash precision, the north–south height every one of its cells
    /// has at least, anywhere on the globe; `None` for H3 and S2, whose cells
    /// have no position-independent size.
    ///
    /// ```
    /// use geocoordinates::privacy::Precision;
    ///
    /// let height = Precision::geohash(6)?.min_cell_height().expect("geohash");
    /// assert!((height.meters() - 607.4).abs() < 0.1);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn min_cell_height(self) -> Option<Length> {
        match self.grid {
            SnapGrid::Geohash => Some(Length::from_meters(geohash_min_height_m(self.level))),
            #[cfg(feature = "h3")]
            SnapGrid::H3 => None,
            #[cfg(feature = "s2")]
            SnapGrid::S2 => None,
        }
    }
}

/// The least north–south height, in meters, of any geohash cell of `length`
/// characters: its latitude span (180° halved once per latitude bit, of which a
/// code of `length` characters has `5 * length / 2`) at the shortest meridian
/// degree.
fn geohash_min_height_m(length: u8) -> f64 {
    let latitude_bits = 5 * i32::from(length) / 2;
    180.0 / 2_f64.powi(latitude_bits) * MIN_METERS_PER_DEGREE_LATITUDE
}

/// Snaps `coord` to the centre of the cell of `precision` that holds it.
///
/// The result depends only on that cell, so every point of the cell coarsens to
/// the same fix, and coarsening the result again returns it unchanged. One
/// place is one input: longitude 180 is read as -180, and a pole as longitude
/// 0, so a point on the antimeridian or at a pole lands in one cell however its
/// longitude is written. The result's fields:
///
/// - `coord`: the cell centre on WGS-84, with no height.
/// - `accuracy`: a [`Bound`](AccuracyKind::Bound) horizontal magnitude equal to
///   the cell's circumradius, the largest WGS-84 geodesic distance from the
///   centre to a cell vertex, which `coord`'s input never lies farther than.
/// - `source`: `raw` is `privacy:` and the cell (`privacy:geohash:gcpuvp`,
///   `privacy:h3:` and the index in hex, `privacy:s2:` and the id in hex), with
///   full confidence.
/// - `timestamp`: `None`.
///
/// # Errors
/// [`Error::CrsMismatch`] (expecting WGS-84) when `coord` is on another datum.
///
/// ```
/// use geocoordinates::privacy::{Precision, coarsen};
/// use geocoordinates::{Coordinate, Error};
///
/// let fix = coarsen(Coordinate::wgs84(57.64911, 10.40744)?, Precision::geohash(5)?)?;
/// assert_eq!(fix.source.expect("named").raw, "privacy:geohash:u4pru");
/// assert!(fix.accuracy.and_then(|a| a.horizontal()).expect("bound").meters() < 3_000.0);
/// assert!(matches!(
///     coarsen(Coordinate::gcj02(39.9, 116.4)?, Precision::geohash(5)?),
///     Err(Error::CrsMismatch { .. })
/// ));
/// # Ok::<(), Error>(())
/// ```
pub fn coarsen(coord: Coordinate, precision: Precision) -> Result<Fix> {
    let (cell, name) = snap(coord, precision)?;
    let max_error = cell.max_error();
    let mut fix = Fix::from_coord(cell.into_inner());
    fix.accuracy = Some(Accuracy::new(AccuracyKind::Bound, Some(max_error), None)?);
    fix.source = Some(RawSource::new(
        format!("privacy:{name}"),
        Confidence::CERTAIN,
    ));
    Ok(fix)
}

/// [`coarsen`] for a whole fix: the cell's fix, keeping `fix`'s timestamp.
///
/// The horizontal magnitude grows by the fix's own, since the true position may
/// lie that much farther from the cell centre. It stays a
/// [`Bound`](AccuracyKind::Bound) when the fix carried a bound or no horizontal
/// magnitude, and is [`Unspecified`](AccuracyKind::Unspecified) when the fix's
/// magnitude was a statistical one, which a hard offset does not preserve.
pub(crate) fn coarsen_fix(fix: Fix, precision: Precision) -> Result<Fix> {
    let mut coarse = coarsen(fix.coord, precision)?;
    coarse.timestamp = fix.timestamp;
    if let Some((kind, own)) = fix
        .accuracy
        .and_then(|accuracy| Some((accuracy.kind(), accuracy.horizontal()?)))
    {
        let cell = coarse
            .accuracy
            .and_then(|accuracy| accuracy.horizontal())
            .unwrap_or(Length::ZERO);
        let kind = if kind == AccuracyKind::Bound {
            AccuracyKind::Bound
        } else {
            AccuracyKind::Unspecified
        };
        coarse.accuracy = Some(Accuracy::new(kind, Some(cell + own), None)?);
    }
    Ok(coarse)
}

/// The cell of `precision` holding `coord`, decoded to its centre and
/// circumradius, and the cell's name for the result's [`RawSource`].
///
/// One place always lands in one cell: longitude 180 is read as -180 and a
/// pole's longitude as 0, so a point on the antimeridian or at a pole does not
/// fall in a different cell depending on how its longitude was written.
fn snap(coord: Coordinate, precision: Precision) -> Result<(Approx<Coordinate>, String)> {
    let lon = if coord.lat().abs() == 90.0 {
        0.0
    } else {
        wrap_longitude(coord.lon())
    };
    let coord = Coordinate::new(coord.lat(), lon, coord.crs())?;
    match precision.grid {
        SnapGrid::Geohash => {
            let code = Geohash::encode(coord, precision.level)?;
            Ok((code.decode(), format!("geohash:{code}")))
        }
        #[cfg(feature = "h3")]
        SnapGrid::H3 => {
            let cell = H3Cell::encode(coord, precision.level)?;
            Ok((cell.decode(), format!("h3:{:x}", cell.as_u64())))
        }
        #[cfg(feature = "s2")]
        SnapGrid::S2 => {
            let cell = S2CellId::encode(coord, precision.level)?;
            Ok((cell.decode(), format!("s2:{:016x}", cell.as_u64())))
        }
    }
}
