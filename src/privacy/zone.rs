//! [`ExclusionZone`], [`ZoneAction`] and [`apply`], re-exported flat from
//! [`privacy`](super).

use super::{Precision, coarsen_fix};
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::fix::Fix;
use crate::geodesy::{Ellipsoid, geodesic_distance};
use crate::index::GeoBounds;
use crate::units::Length;

/// A place whose fixes are never shared exactly: a geodesic circle, or a
/// [`GeoBounds`] box.
///
/// Membership is inclusive and measured on the zone's own datum:
///
/// - A **circle** holds every point whose ellipsoidal (Karney) geodesic
///   distance from its centre is at most its radius. The geodesic crosses the
///   antimeridian and passes over the poles wherever the shortest path does,
///   so a zone on either needs no special handling.
/// - A **box** holds what [`GeoBounds::contains`] holds, including a box that
///   crosses the antimeridian and every longitude at a pole.
///
/// Valid by construction: [`from_circle`](Self::from_circle) refuses a radius
/// that is negative, NaN or infinite, and a centre on GCJ-02 or BD-09, which
/// have no ellipsoid to measure on. Under the `serde` feature it deserializes
/// through the same checks, from `{"Circle":{"center":…,"radius":…}}` or
/// `{"Bounds":{…}}`.
///
/// ```
/// use geocoordinates::index::GeoBounds;
/// use geocoordinates::privacy::ExclusionZone;
/// use geocoordinates::{Coordinate, Crs, Length};
///
/// let circle = ExclusionZone::from_circle(Coordinate::wgs84(0.0, 179.999)?, Length::from_meters(500.0))?;
/// // 0.002° of longitude at the equator is about 223 m, across the antimeridian.
/// assert!(circle.contains(Coordinate::wgs84(0.0, -179.999)?)?);
///
/// let fiji = ExclusionZone::from_bounds(GeoBounds::new(-21.0, 176.0, -12.0, -178.0, Crs::Wgs84)?);
/// assert!(fiji.contains(Coordinate::wgs84(-17.0, 180.0)?)?);
/// assert!(!fiji.contains(Coordinate::wgs84(-17.0, 0.0)?)?);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "Shape", into = "Shape"))]
pub struct ExclusionZone {
    shape: Shape,
}

/// The shape of an [`ExclusionZone`], and its unchecked wire form.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Shape {
    Circle { center: Coordinate, radius: Length },
    Bounds(GeoBounds),
}

impl TryFrom<Shape> for ExclusionZone {
    type Error = Error;

    fn try_from(shape: Shape) -> Result<Self> {
        match shape {
            Shape::Circle { center, radius } => Self::from_circle(center, radius),
            Shape::Bounds(bounds) => Ok(Self::from_bounds(bounds)),
        }
    }
}

impl From<ExclusionZone> for Shape {
    fn from(zone: ExclusionZone) -> Self {
        zone.shape
    }
}

impl ExclusionZone {
    /// A circle of `radius` around `center`, on `center`'s datum.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] (field `radius`) when `radius` is negative, NaN
    /// or infinite; [`Error::CrsMismatch`] (expecting WGS-84) when `center` is on
    /// GCJ-02 or BD-09, which have no ellipsoid.
    ///
    /// ```
    /// use geocoordinates::privacy::ExclusionZone;
    /// use geocoordinates::{Coordinate, Error, Length};
    ///
    /// let home = Coordinate::wgs84(40.0, -105.0)?;
    /// let zone = ExclusionZone::from_circle(home, Length::from_meters(300.0))?;
    /// assert_eq!(zone.circle(), Some((home, Length::from_meters(300.0))));
    /// assert!(matches!(
    ///     ExclusionZone::from_circle(home, Length::from_meters(-1.0)),
    ///     Err(Error::InvalidValue { field: "radius", .. })
    /// ));
    /// assert!(matches!(
    ///     ExclusionZone::from_circle(Coordinate::gcj02(39.9, 116.4)?, Length::from_meters(1.0)),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn from_circle(center: Coordinate, radius: Length) -> Result<Self> {
        let meters = radius.meters();
        if !(meters.is_finite() && meters >= 0.0) {
            return Err(Error::InvalidValue {
                field: "radius",
                detail: format!("must be finite and nonnegative, got {meters} m"),
            });
        }
        if Ellipsoid::for_crs(center.crs()).is_none() {
            return Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                found: center.crs(),
            });
        }
        Ok(Self {
            shape: Shape::Circle { center, radius },
        })
    }

    /// The zone covering `bounds`, on its datum. Infallible: a [`GeoBounds`]
    /// is already valid.
    ///
    /// ```
    /// use geocoordinates::index::GeoBounds;
    /// use geocoordinates::privacy::ExclusionZone;
    /// use geocoordinates::Crs;
    ///
    /// let campus = GeoBounds::new(37.42, -122.18, 37.44, -122.16, Crs::Wgs84)?;
    /// assert_eq!(ExclusionZone::from_bounds(campus).bounds(), Some(campus));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn from_bounds(bounds: GeoBounds) -> Self {
        Self {
            shape: Shape::Bounds(bounds),
        }
    }

    /// The centre and radius of a circular zone, or `None` for a box.
    ///
    /// ```
    /// use geocoordinates::index::GeoBounds;
    /// use geocoordinates::privacy::ExclusionZone;
    /// use geocoordinates::Crs;
    ///
    /// let zone = ExclusionZone::from_bounds(GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Wgs84)?);
    /// assert_eq!(zone.circle(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn circle(&self) -> Option<(Coordinate, Length)> {
        match self.shape {
            Shape::Circle { center, radius } => Some((center, radius)),
            Shape::Bounds(_) => None,
        }
    }

    /// The box of a box zone, or `None` for a circle.
    ///
    /// ```
    /// use geocoordinates::privacy::ExclusionZone;
    /// use geocoordinates::{Coordinate, Length};
    ///
    /// let zone = ExclusionZone::from_circle(Coordinate::wgs84(0.0, 0.0)?, Length::from_meters(1.0))?;
    /// assert_eq!(zone.bounds(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn bounds(&self) -> Option<GeoBounds> {
        match self.shape {
            Shape::Bounds(bounds) => Some(bounds),
            Shape::Circle { .. } => None,
        }
    }

    /// The zone's datum: its centre's or its box's.
    ///
    /// ```
    /// use geocoordinates::privacy::ExclusionZone;
    /// use geocoordinates::{Coordinate, Crs, Length};
    ///
    /// let centre = Coordinate::new(35.0, 139.0, Crs::Tokyo)?;
    /// assert_eq!(ExclusionZone::from_circle(centre, Length::from_meters(5.0))?.crs(), Crs::Tokyo);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn crs(&self) -> Crs {
        match self.shape {
            Shape::Circle { center, .. } => center.crs(),
            Shape::Bounds(bounds) => bounds.crs(),
        }
    }

    /// Whether `point` lies in the zone, edge included.
    ///
    /// # Errors
    /// [`Error::CrsMismatch`] when `point` is not on the zone's datum: a zone
    /// never silently measures across datums.
    ///
    /// ```
    /// use geocoordinates::privacy::ExclusionZone;
    /// use geocoordinates::{Coordinate, Error, Length};
    ///
    /// // A zone around the north pole holds the pole at every longitude.
    /// let zone = ExclusionZone::from_circle(Coordinate::wgs84(90.0, 0.0)?, Length::from_meters(1_000.0))?;
    /// assert!(zone.contains(Coordinate::wgs84(90.0, 135.0)?)?);
    /// assert!(zone.contains(Coordinate::wgs84(89.995, -90.0)?)?); // about 557 m away
    /// assert!(!zone.contains(Coordinate::wgs84(89.98, 0.0)?)?); // about 2.2 km away
    /// assert!(matches!(
    ///     zone.contains(Coordinate::gcj02(39.9, 116.4)?),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn contains(&self, point: Coordinate) -> Result<bool> {
        match self.shape {
            Shape::Circle { center, radius } => Ok(geodesic_distance(center, point)? <= radius),
            Shape::Bounds(bounds) => bounds.contains(point),
        }
    }
}

/// What [`apply`] does to a fix inside an exclusion zone.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every
/// variant, so adding one here is a deliberate, compile-forcing change on both
/// sides.
///
/// ```
/// use geocoordinates::privacy::{Precision, ZoneAction};
///
/// let action = ZoneAction::Coarsen(Precision::geohash(5)?);
/// assert_ne!(action, ZoneAction::Strip);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ZoneAction {
    /// Remove the fix: no position is shared at all.
    Strip,
    /// Replace the fix by its coarsened form at this precision (see [`apply`]).
    Coarsen(Precision),
}

/// The fix to share given exclusion `zones`: `fix` unchanged when it lies in no
/// zone, otherwise what `action` makes of it, `None` for
/// [`Strip`](ZoneAction::Strip).
///
/// [`Coarsen`](ZoneAction::Coarsen) returns [`coarsen`](super::coarsen)'s fix
/// for `fix.coord`, keeping `fix.timestamp`. Its horizontal accuracy is the
/// cell's circumradius plus `fix`'s own horizontal magnitude, a
/// [`Bound`](crate::fix::AccuracyKind::Bound) when `fix` carried a bound or no
/// horizontal magnitude, and
/// [`Unspecified`](crate::fix::AccuracyKind::Unspecified) when it carried a
/// statistical one. The fix's height, vertical accuracy and source never survive:
/// the source's raw text would repeat the exact position.
///
/// Every zone is checked, so a fix on a datum some zone is not on fails rather
/// than slipping through unchecked.
///
/// # Errors
/// [`Error::CrsMismatch`] when `fix` is not on the datum of every zone, or when
/// it is coarsened off WGS-84 (every snapping grid is defined on WGS-84).
///
/// ```
/// use geocoordinates::fix::Fix;
/// use geocoordinates::privacy::{ExclusionZone, Precision, ZoneAction, apply};
/// use geocoordinates::{Coordinate, Error, Length};
///
/// let school = Coordinate::wgs84(40.7484, -73.9857)?;
/// let zones = [ExclusionZone::from_circle(school, Length::from_meters(250.0))?];
/// let action = ZoneAction::Coarsen(Precision::geohash(5)?);
///
/// let near = apply(Fix::from_coord(Coordinate::wgs84(40.749, -73.986)?), &zones, action)?;
/// assert!(near.expect("coarsened").source.expect("derived").raw.starts_with("privacy:geohash:"));
///
/// let far = Fix::from_coord(Coordinate::wgs84(41.0, -74.0)?);
/// assert_eq!(apply(far.clone(), &zones, action)?, Some(far));
///
/// let elsewhere = Fix::from_coord(Coordinate::gcj02(39.9, 116.4)?);
/// assert!(matches!(apply(elsewhere, &zones, action), Err(Error::CrsMismatch { .. })));
/// # Ok::<(), Error>(())
/// ```
pub fn apply(fix: Fix, zones: &[ExclusionZone], action: ZoneAction) -> Result<Option<Fix>> {
    let mut inside = false;
    for zone in zones {
        inside |= zone.contains(fix.coord)?;
    }
    if !inside {
        return Ok(Some(fix));
    }
    match action {
        ZoneAction::Strip => Ok(None),
        ZoneAction::Coarsen(precision) => coarsen_fix(fix, precision).map(Some),
    }
}
