//! [`DisplayDatum`], [`display_coordinate`] and [`tap_to_wgs84`], re-exported flat
//! from [`map`](super).

use crate::approx::Approx;
use crate::convert::convert;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::units::Length;

/// The datum a basemap draws positions in.
///
/// Most basemaps draw WGS-84. Inside China, AMap, Tencent Maps and Apple Maps draw
/// GCJ-02, so a WGS-84 layer is shifted by up to about 700 m unless it is converted
/// first. Baidu's own projection is [`BaiduMercator`](crate::china::BaiduMercator).
///
/// ```
/// use geocoordinates::{Coordinate, Crs, map::{DisplayDatum, display_coordinate}};
///
/// let shown = display_coordinate(Coordinate::wgs84(39.9, 116.4)?, DisplayDatum::Gcj02)?;
/// assert_eq!(shown.crs(), Crs::Gcj02);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DisplayDatum {
    /// WGS-84, which most basemaps draw.
    Wgs84,
    /// GCJ-02, which basemaps inside China draw.
    Gcj02,
}

/// Checks that `point` is the WGS-84 position a layer holds.
fn require_wgs84(point: Coordinate) -> Result<()> {
    if point.crs() == Crs::Wgs84 {
        Ok(())
    } else {
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: point.crs(),
        })
    }
}

/// Where a basemap drawing `display` must place the WGS-84 `point`: unchanged for
/// WGS-84, and the exact forward GCJ-02 offset for GCJ-02, which leaves a point
/// outside China where it is. The height is kept.
///
/// # Errors
///
/// [`Error::CrsMismatch`] unless `point` is on WGS-84.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, map::{DisplayDatum, display_coordinate}};
///
/// let beijing = Coordinate::wgs84(39.9087, 116.3975)?;
/// let shown = display_coordinate(beijing, DisplayDatum::Gcj02)?;
/// assert!((shown.lon() - beijing.lon()).abs() > 0.005); // hundreds of meters east
/// assert_eq!(display_coordinate(beijing, DisplayDatum::Wgs84)?, beijing);
/// assert!(matches!(
///     display_coordinate(shown, DisplayDatum::Gcj02),
///     Err(Error::CrsMismatch { expected: Crs::Wgs84, found: Crs::Gcj02 })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn display_coordinate(point: Coordinate, display: DisplayDatum) -> Result<Coordinate> {
    require_wgs84(point)?;
    match display {
        DisplayDatum::Wgs84 => Ok(point),
        // The forward offset is exact, so the bound `convert` reports is zero.
        DisplayDatum::Gcj02 => Ok(convert(point, Crs::Gcj02)?.into_inner()),
    }
}

/// The WGS-84 position under a tap on a basemap, from the position the map reports
/// in its own datum. A WGS-84 tap is exact. A GCJ-02 tap is inverted iteratively
/// ([`Gcj02::to_wgs84_refined`](crate::china::Gcj02::to_wgs84_refined)), which is
/// approximate: the result carries a bound of 0.5 m. The height is kept.
///
/// # Errors
///
/// [`Error::CrsMismatch`] unless `tap` is on WGS-84 or GCJ-02, the two
/// [`DisplayDatum`]s.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, map::tap_to_wgs84};
///
/// let tap = Coordinate::gcj02(39.9101, 116.4037)?;
/// let under = tap_to_wgs84(tap)?;
/// assert_eq!(under.value().crs(), Crs::Wgs84);
/// assert_eq!(under.max_error().meters(), 0.5);
/// assert!(matches!(
///     tap_to_wgs84(Coordinate::bd09(39.9, 116.4)?),
///     Err(Error::CrsMismatch { expected: Crs::Gcj02, found: Crs::Bd09 })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn tap_to_wgs84(tap: Coordinate) -> Result<Approx<Coordinate>> {
    match tap.crs() {
        Crs::Wgs84 => Approx::new(tap, Length::from_meters(0.0)),
        Crs::Gcj02 => convert(tap, Crs::Wgs84),
        found => Err(Error::CrsMismatch {
            expected: Crs::Gcj02,
            found,
        }),
    }
}
