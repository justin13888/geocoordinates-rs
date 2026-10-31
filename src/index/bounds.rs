//! [`GeoBounds`], re-exported flat from [`index`](super).

use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

mod arc;

use self::arc::{LonArc, canonical_edges, same_meridian, smallest_arc};
use crate::angle::wrap_longitude;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::geodesy::Ellipsoid;
use crate::units::Length;

/// Outward margin, in degrees (about 0.1 mm), that [`GeoBounds::expand_by`]
/// adds to each moved edge, so round-off in the geodesic solution cannot leave
/// a point at exactly the requested distance outside the box.
const EXPAND_MARGIN_DEG: f64 = 1e-9;

/// Ternary-search steps that locate the bearing of greatest longitude reach:
/// they narrow the 180° bracket to under 1e-7°, and the reach is flat at its
/// peak, so its value is then accurate to round-off.
const REACH_SEARCH_STEPS: usize = 60;

/// A geographic box: south and north latitude edges and west and east longitude
/// edges, in decimal degrees on one [`Crs`], with every edge inclusive.
///
/// A west edge greater than the east edge means the box crosses the
/// antimeridian: `west = 170, east = -170` is 20° wide. `west = -180, east = 180`
/// spans every longitude. Longitudes `180` and `-180` name one meridian, and a
/// pole is one point whatever its longitude, so [`contains`](Self::contains) and
/// [`intersects`](Self::intersects) treat them that way.
///
/// **Axis order.** [`new`](Self::new) takes `south, west, north, east`, the order
/// of the crate's lat-first convention, which is not GeoJSON's
/// `west, south, east, north`.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, index::GeoBounds};
///
/// let fiji = GeoBounds::new(-21.0, 176.0, -12.0, -178.0, Crs::Wgs84)?;
/// assert!(fiji.crosses_antimeridian());
/// assert!(fiji.contains(Coordinate::wgs84(-17.0, 180.0)?)?);
/// assert!(fiji.contains(Coordinate::wgs84(-17.0, -180.0)?)?);
/// assert!(!fiji.contains(Coordinate::wgs84(-17.0, 0.0)?)?);
/// // South must not lie north of north.
/// assert!(matches!(
///     GeoBounds::new(10.0, 0.0, -10.0, 1.0, Crs::Wgs84),
///     Err(Error::InvalidValue { field: "south", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "GeoBoundsRepr"))]
pub struct GeoBounds {
    south: f64,
    west: f64,
    north: f64,
    east: f64,
    crs: Crs,
}

/// The unchecked wire shape of a [`GeoBounds`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct GeoBoundsRepr {
    south: f64,
    west: f64,
    north: f64,
    east: f64,
    crs: Crs,
}

#[cfg(feature = "serde")]
impl TryFrom<GeoBoundsRepr> for GeoBounds {
    type Error = Error;

    fn try_from(repr: GeoBoundsRepr) -> Result<Self> {
        Self::new(repr.south, repr.west, repr.north, repr.east, repr.crs)
    }
}

impl GeoBounds {
    /// A box from its four edges, in decimal degrees on `crs`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] naming the edge when it is non-finite or outside
    /// `[-90, 90]` (`south`, `north`) or `[-180, 180]` (`west`, `east`), and
    /// `field: "south"` when south lies north of north.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, index::GeoBounds};
    ///
    /// let alps = GeoBounds::new(44.0, 5.0, 48.0, 16.0, Crs::Wgs84)?;
    /// assert_eq!(alps.north(), 48.0);
    /// assert!(matches!(
    ///     GeoBounds::new(0.0, 181.0, 1.0, 0.0, Crs::Wgs84),
    ///     Err(Error::InvalidValue { field: "west", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(south: f64, west: f64, north: f64, east: f64, crs: Crs) -> Result<Self> {
        for (field, value, limit) in [
            ("south", south, 90.0),
            ("west", west, 180.0),
            ("north", north, 90.0),
            ("east", east, 180.0),
        ] {
            if !value.is_finite() || value.abs() > limit {
                return Err(Error::InvalidValue {
                    field,
                    detail: format!("{value} is not a finite value in [-{limit}, {limit}]"),
                });
            }
        }
        if south > north {
            return Err(Error::InvalidValue {
                field: "south",
                detail: format!("{south} lies north of the north edge {north}"),
            });
        }
        Ok(Self {
            south,
            west,
            north,
            east,
            crs,
        })
    }

    /// The smallest box holding every point, choosing the smallest longitude
    /// span, so a cluster around ±180° does not span the globe.
    ///
    /// The result does not depend on the order of the points. The widest empty
    /// longitude gap falls outside the box; where several gaps tie, the box is
    /// the one with the lowest west edge. A pole has no longitude of its own,
    /// so it extends the latitude range only, and a set of nothing but poles has
    /// `west = east = 0`. Longitude `180` is read as `-180` when it would be a
    /// west edge or a lone meridian, and `-180` as `180` when it would be an east
    /// edge. Heights are ignored.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "points"`) for no points, and
    /// [`Error::CrsMismatch`] when the points' CRSs differ.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, index::GeoBounds};
    ///
    /// let bounds = GeoBounds::from_points([
    ///     Coordinate::wgs84(10.0, 179.0)?,
    ///     Coordinate::wgs84(12.0, -179.0)?,
    /// ])?;
    /// assert_eq!((bounds.west(), bounds.east()), (179.0, -179.0));
    /// assert!(matches!(
    ///     GeoBounds::from_points([]),
    ///     Err(Error::InvalidValue { field: "points", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_points(points: impl IntoIterator<Item = Coordinate>) -> Result<Self> {
        let mut points = points.into_iter();
        let first = points.next().ok_or_else(|| Error::InvalidValue {
            field: "points",
            detail: "no points to bound".into(),
        })?;
        let crs = first.crs();
        let (mut south, mut north) = (first.lat(), first.lat());
        let mut lons = Vec::new();
        for point in std::iter::once(first).chain(points) {
            if point.crs() != crs {
                return Err(Error::CrsMismatch {
                    expected: crs,
                    found: point.crs(),
                });
            }
            south = south.min(point.lat());
            north = north.max(point.lat());
            if point.lat().abs() < 90.0 {
                lons.push(same_meridian(point.lon()));
            }
        }
        let (west, east) = smallest_arc(lons);
        Ok(Self {
            south,
            west,
            north,
            east,
            crs,
        })
    }

    /// The south edge, in decimal degrees.
    ///
    /// ```
    /// use geocoordinates::{Crs, index::GeoBounds};
    ///
    /// assert_eq!(GeoBounds::new(1.0, 2.0, 3.0, 4.0, Crs::Wgs84)?.south(), 1.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn south(&self) -> f64 {
        self.south
    }

    /// The west edge, in decimal degrees.
    ///
    /// ```
    /// use geocoordinates::{Crs, index::GeoBounds};
    ///
    /// assert_eq!(GeoBounds::new(1.0, 2.0, 3.0, 4.0, Crs::Wgs84)?.west(), 2.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn west(&self) -> f64 {
        self.west
    }

    /// The north edge, in decimal degrees.
    ///
    /// ```
    /// use geocoordinates::{Crs, index::GeoBounds};
    ///
    /// assert_eq!(GeoBounds::new(1.0, 2.0, 3.0, 4.0, Crs::Wgs84)?.north(), 3.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn north(&self) -> f64 {
        self.north
    }

    /// The east edge, in decimal degrees.
    ///
    /// ```
    /// use geocoordinates::{Crs, index::GeoBounds};
    ///
    /// assert_eq!(GeoBounds::new(1.0, 2.0, 3.0, 4.0, Crs::Wgs84)?.east(), 4.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn east(&self) -> f64 {
        self.east
    }

    /// The reference system the edges are expressed in.
    ///
    /// ```
    /// use geocoordinates::{Crs, index::GeoBounds};
    ///
    /// assert_eq!(GeoBounds::new(1.0, 2.0, 3.0, 4.0, Crs::Gcj02)?.crs(), Crs::Gcj02);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn crs(&self) -> Crs {
        self.crs
    }

    /// Whether the box crosses the antimeridian: its west edge lies east of its
    /// east edge.
    ///
    /// ```
    /// use geocoordinates::{Crs, index::GeoBounds};
    ///
    /// assert!(GeoBounds::new(0.0, 170.0, 1.0, -170.0, Crs::Wgs84)?.crosses_antimeridian());
    /// assert!(!GeoBounds::new(0.0, -170.0, 1.0, 170.0, Crs::Wgs84)?.crosses_antimeridian());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn crosses_antimeridian(&self) -> bool {
        self.west > self.east
    }

    /// Whether `point` lies in the box, edges included. Its height is ignored.
    ///
    /// A pole is inside a box that reaches it, whatever the box's longitudes.
    ///
    /// # Errors
    ///
    /// [`Error::CrsMismatch`] when `point` is not on the box's CRS.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, index::GeoBounds};
    ///
    /// let cap = GeoBounds::new(80.0, 10.0, 90.0, 20.0, Crs::Wgs84)?;
    /// assert!(cap.contains(Coordinate::wgs84(85.0, 15.0)?)?);
    /// assert!(!cap.contains(Coordinate::wgs84(85.0, 25.0)?)?);
    /// assert!(cap.contains(Coordinate::wgs84(90.0, 25.0)?)?); // the pole
    /// assert!(matches!(
    ///     cap.contains(Coordinate::gcj02(85.0, 15.0)?),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn contains(&self, point: Coordinate) -> Result<bool> {
        self.require_crs(point.crs())?;
        let lat = point.lat();
        if lat < self.south || lat > self.north {
            return Ok(false);
        }
        Ok(lat.abs() == 90.0 || self.arc().contains(point.lon()))
    }

    /// Whether the two boxes share any point, edges included.
    ///
    /// # Errors
    ///
    /// [`Error::CrsMismatch`] when the boxes are on different CRSs.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, index::GeoBounds};
    ///
    /// let crossing = GeoBounds::new(-10.0, 170.0, 10.0, -170.0, Crs::Wgs84)?;
    /// let east_side = GeoBounds::new(0.0, -175.0, 5.0, -160.0, Crs::Wgs84)?;
    /// let far = GeoBounds::new(0.0, 0.0, 5.0, 10.0, Crs::Wgs84)?;
    /// assert!(crossing.intersects(&east_side)?);
    /// assert!(!crossing.intersects(&far)?);
    /// let other = GeoBounds::new(0.0, 0.0, 5.0, 10.0, Crs::Gcj02)?;
    /// assert!(matches!(far.intersects(&other), Err(Error::CrsMismatch { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn intersects(&self, other: &Self) -> Result<bool> {
        self.require_crs(other.crs)?;
        if self.south > other.north || other.south > self.north {
            return Ok(false);
        }
        if self.north == 90.0 && other.north == 90.0 || self.south == -90.0 && other.south == -90.0
        {
            return Ok(true); // both reach one pole, which has every longitude
        }
        let (a, b) = (self.arc(), other.arc());
        Ok(a.contains(b.west) || b.contains(a.west))
    }

    /// The smallest box holding both boxes, taking the shorter way around the
    /// globe between them. Where both ways are equally long, the result has the
    /// lower west edge, so the operation is commutative.
    ///
    /// # Errors
    ///
    /// [`Error::CrsMismatch`] when the boxes are on different CRSs.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, index::GeoBounds};
    ///
    /// let west = GeoBounds::new(0.0, 170.0, 1.0, 175.0, Crs::Wgs84)?;
    /// let east = GeoBounds::new(2.0, -175.0, 3.0, -170.0, Crs::Wgs84)?;
    /// let both = west.union(&east)?;
    /// assert_eq!(
    ///     (both.south(), both.west(), both.north(), both.east()),
    ///     (0.0, 170.0, 3.0, -170.0)
    /// );
    /// let other = GeoBounds::new(0.0, 0.0, 5.0, 10.0, Crs::Gcj02)?;
    /// assert!(matches!(west.union(&other), Err(Error::CrsMismatch { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn union(&self, other: &Self) -> Result<Self> {
        self.require_crs(other.crs)?;
        // A box that is one pole has no longitudes to cover.
        let arc = match (self.is_pole(), other.is_pole()) {
            (true, true) => LonArc::meridian(0.0),
            (true, false) => other.arc(),
            (false, true) => self.arc(),
            (false, false) => self.arc().union(other.arc()),
        };
        Ok(Self {
            south: self.south.min(other.south),
            west: arc.west,
            north: self.north.max(other.north),
            east: arc.east,
            crs: self.crs,
        })
    }

    /// The middle of the box in latitude and longitude, on the box's CRS, with
    /// its longitude wrapped into `[-180, 180)` and no height. This is the
    /// midpoint of the edges, not a geodesic centroid.
    ///
    /// ```
    /// use geocoordinates::{Crs, index::GeoBounds};
    ///
    /// let crossing = GeoBounds::new(-10.0, 170.0, 20.0, -160.0, Crs::Wgs84)?;
    /// let center = crossing.center();
    /// assert_eq!((center.lat(), center.lon()), (5.0, -175.0));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn center(&self) -> Coordinate {
        let arc = self.arc();
        let lat = self.south + (self.north - self.south) / 2.0;
        let lon = wrap_longitude(arc.west + arc.span() / 2.0);
        Coordinate::new(lat, lon, self.crs).expect("the middle of valid edges is a valid position")
    }

    /// The box grown so it holds every point within geodesic `distance` of the
    /// original box, on the ellipsoid of the box's CRS.
    ///
    /// The latitude edges move along their meridians and clamp at the poles.
    /// When either reaches a pole, every longitude is within reach, so the box
    /// spans `-180` to `180`. Otherwise the longitude edges move by the greatest
    /// longitude reach of a geodesic of that length, measured from the edge
    /// nearest a pole. Each moved edge has about 0.1 mm of margin beyond the exact
    /// solution, so the box is at most that much larger than the tightest one.
    /// A zero `distance` returns the box unchanged.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "distance"`) when `distance` is negative
    /// or non-finite, and [`Error::CrsMismatch`] on GCJ-02 / BD-09, which have no
    /// ellipsoid.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, Length, LengthUnit, index::GeoBounds};
    ///
    /// let edge = GeoBounds::new(0.0, 179.5, 1.0, 180.0, Crs::Wgs84)?;
    /// let grown = edge.expand_by(Length::from_unit(100.0, LengthUnit::Kilometer))?;
    /// assert!(grown.crosses_antimeridian() && grown.east() > -179.2);
    ///
    /// // Reaching a pole clamps latitude and takes in every longitude.
    /// let polar = GeoBounds::new(89.9, 0.0, 89.95, 1.0, Crs::Wgs84)?
    ///     .expand_by(Length::from_unit(50.0, LengthUnit::Kilometer))?;
    /// assert_eq!((polar.north(), polar.west(), polar.east()), (90.0, -180.0, 180.0));
    ///
    /// assert!(matches!(
    ///     edge.expand_by(Length::from_meters(-1.0)),
    ///     Err(Error::InvalidValue { field: "distance", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn expand_by(&self, distance: Length) -> Result<Self> {
        let meters = distance.meters();
        if !meters.is_finite() || meters < 0.0 {
            return Err(Error::InvalidValue {
                field: "distance",
                detail: "must be finite and nonnegative".into(),
            });
        }
        let ellipsoid = Ellipsoid::for_crs(self.crs).ok_or(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: self.crs,
        })?;
        if meters == 0.0 {
            return Ok(*self);
        }
        let geodesic = Geodesic::new(ellipsoid.semi_major().meters(), ellipsoid.flattening());

        let north_gap: f64 = geodesic.inverse(self.north, 0.0, 90.0, 0.0);
        let south_gap: f64 = geodesic.inverse(self.south, 0.0, -90.0, 0.0);
        if meters >= north_gap || meters >= south_gap {
            let north = if meters >= north_gap {
                90.0
            } else {
                meridian_lat(&geodesic, self.north, 0.0, meters)
            };
            let south = if meters >= south_gap {
                -90.0
            } else {
                meridian_lat(&geodesic, self.south, 180.0, meters)
            };
            return Self::new(south, -180.0, north, 180.0, self.crs);
        }
        let north = meridian_lat(&geodesic, self.north, 0.0, meters);
        let south = meridian_lat(&geodesic, self.south, 180.0, meters);

        // The parallels shrink towards a pole, so the same distance reaches the
        // most longitude from the edge nearest one.
        let reach = longitude_reach(&geodesic, self.north.abs().max(self.south.abs()), meters)
            + EXPAND_MARGIN_DEG;
        let arc = self.arc();
        if arc.span() + 2.0 * reach >= 360.0 {
            return Self::new(south, -180.0, north, 180.0, self.crs);
        }
        let (west, east) = canonical_edges(
            wrap_longitude(arc.west - reach),
            wrap_longitude(arc.east + reach),
        );
        Self::new(south, west, north, east, self.crs)
    }

    fn require_crs(&self, found: Crs) -> Result<()> {
        if found == self.crs {
            Ok(())
        } else {
            Err(Error::CrsMismatch {
                expected: self.crs,
                found,
            })
        }
    }

    fn arc(&self) -> LonArc {
        LonArc {
            west: self.west,
            east: self.east,
        }
    }

    /// Whether the box is a single pole, whose longitude edges mean nothing.
    fn is_pole(&self) -> bool {
        self.south == self.north && self.south.abs() == 90.0
    }
}

/// `lat` moved `meters` along its meridian at `bearing` (0 north, 180 south) and
/// past that by the margin. The caller has checked the pole is out of reach.
fn meridian_lat(geodesic: &Geodesic, lat: f64, bearing: f64, meters: f64) -> f64 {
    let (moved, _): (f64, f64) = geodesic.direct(lat, 0.0, bearing, meters);
    let margin = if bearing == 0.0 {
        EXPAND_MARGIN_DEG
    } else {
        -EXPAND_MARGIN_DEG
    };
    (moved + margin).clamp(-90.0, 90.0)
}

/// The greatest longitude difference, in degrees, between a point at `lat` and
/// any point `meters` away, when that circle does not enclose a pole.
///
/// A ternary search over the initial bearing: longitude reach rises to one peak
/// between due north and due south.
fn longitude_reach(geodesic: &Geodesic, lat: f64, meters: f64) -> f64 {
    let reach = |bearing: f64| -> f64 {
        let (_, lon): (f64, f64) = geodesic.direct(lat, 0.0, bearing, meters);
        lon
    };
    let (mut low, mut high) = (0.0_f64, 180.0_f64);
    for _ in 0..REACH_SEARCH_STEPS {
        let third = (high - low) / 3.0;
        let (left, right) = (low + third, high - third);
        if reach(left) < reach(right) {
            low = left;
        } else {
            high = right;
        }
    }
    reach(low)
}

#[cfg(test)]
mod expand_tests;
#[cfg(test)]
mod tests;
