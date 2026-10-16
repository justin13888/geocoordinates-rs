//! Local tangent-plane frames: ENU, NED, and AER.
//!
//! These are defined relative to a reference origin, so they are expressed as
//! methods taking the origin rather than `From` impls. The forward direction
//! (coordinate → ENU / NED / AER), the frame-to-frame conversions, and the
//! inverse (`try_to_coordinate`) are all **exact** to floating-point round-off:
//! a rotation of the ECEF difference vector, and back through
//! [`Ecef::try_to_coordinate`]'s closed-form inverse, on the ellipsoid of the
//! origin's own datum. The origin and the target must share one classic datum
//! (WGS-84, NAD27, Tokyo, Pulkovo-1942) and carry ellipsoidal heights, if any.
//!
//! Each frame value is valid by construction: build one with [`Enu::new`],
//! [`Ned::new`], or [`Aer::new`], which reject non-finite components, an
//! elevation outside `[-90°, 90°]`, and a negative range.
//!
//! ```
//! use geocoordinates::{Coordinate, geodesy::{Aer, Enu, Ned}};
//!
//! let origin = Coordinate::wgs84(40.0, -75.0)?;
//! let target = Coordinate::wgs84(40.01, -75.0)?; // ~1.1 km due north
//! let enu = Enu::try_from_coordinate(target, origin)?;
//! let ned: Ned = enu.into(); // exact repackaging; `to_ned` is the named form
//! let aer: Aer = enu.to_aer();
//! assert_eq!(ned.north_m(), enu.north_m());
//! assert!(enu.east_m().abs() < 1e-6 && aer.elevation_deg() < 0.0); // the Earth curves away
//! # Ok::<(), geocoordinates::Error>(())
//! ```

use super::ecef::{Ecef, ellipsoid_of};
use crate::coord::{Coordinate, Height};
use crate::error::{Error, Result};
use crate::units::Length;

/// East-North-Up offset from a reference origin, in meters.
///
/// Valid by construction — build one with [`Enu::new`] or
/// [`Enu::try_from_coordinate`]. Every component is finite, and so is the
/// offset's magnitude (its [`Aer`] range).
///
/// ```
/// use geocoordinates::geodesy::Enu;
///
/// let up_and_east = Enu::new(3.0, 0.0, 4.0)?;
/// assert_eq!(up_and_east.to_aer().range().meters(), 5.0);
/// assert!(Enu::new(f64::NAN, 0.0, 0.0).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "EnuRecord"))]
pub struct Enu {
    east_m: f64,
    north_m: f64,
    up_m: f64,
}

/// North-East-Down offset from a reference origin, in meters.
///
/// Valid by construction — build one with [`Ned::new`] or
/// [`Ned::try_from_coordinate`]. Every component is finite, and so is the
/// offset's magnitude (its [`Aer`] range).
///
/// ```
/// use geocoordinates::geodesy::Ned;
///
/// // 10 m below the origin: a positive `down_m`.
/// let below = Ned::new(0.0, 0.0, 10.0)?;
/// assert_eq!(below.to_enu().up_m(), -10.0);
/// assert!(Ned::new(0.0, 0.0, f64::INFINITY).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "NedRecord"))]
pub struct Ned {
    north_m: f64,
    east_m: f64,
    down_m: f64,
}

/// Azimuth-Elevation-Range relative to a reference origin.
///
/// Valid by construction — build one with [`Aer::new`] or
/// [`Aer::try_from_coordinate`]. The azimuth is in `[0, 360)`, the elevation
/// in `[-90, 90]`, and the range finite and non-negative.
///
/// ```
/// use geocoordinates::{Length, geodesy::Aer};
///
/// let due_east = Aer::new(90.0, 0.0, Length::from_meters(100.0))?;
/// let enu = due_east.to_enu();
/// assert!((enu.east_m() - 100.0).abs() < 1e-9 && enu.north_m().abs() < 1e-9);
/// assert!(Aer::new(90.0, 91.0, Length::from_meters(100.0)).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "AerRecord"))]
pub struct Aer {
    azimuth_deg: f64,
    elevation_deg: f64,
    range: Length,
}

/// `value` if it is finite, else [`Error::InvalidValue`] naming `field`.
fn finite(value: f64, field: &'static str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidValue {
            field,
            detail: "must be finite".into(),
        })
    }
}

/// The Euclidean norm of a local offset, as [`Enu::to_aer`] computes its range.
/// `hypot` neither overflows nor underflows where the squares would.
fn magnitude(a: f64, b: f64, c: f64) -> f64 {
    a.hypot(b).hypot(c)
}

/// Checks the three finite components of an ENU or NED offset have a finite
/// magnitude, so the offset always has a valid [`Aer`] range.
fn check_magnitude(a: f64, b: f64, c: f64) -> Result<()> {
    if magnitude(a, b, c).is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidValue {
            field: "magnitude",
            detail: "the offset's length overflows f64".into(),
        })
    }
}

impl Enu {
    /// Construct from east, north, and up offsets in meters.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] naming `"east"`, `"north"`, or `"up"` when that
    /// component is not finite, and `"magnitude"` when the components are
    /// finite but the offset's length overflows `f64` (beyond about 1.8e308 m).
    ///
    /// ```
    /// use geocoordinates::{Error, geodesy::Enu};
    ///
    /// let enu = Enu::new(1.0, 2.0, 3.0)?;
    /// assert_eq!((enu.east_m(), enu.north_m(), enu.up_m()), (1.0, 2.0, 3.0));
    /// assert!(matches!(
    ///     Enu::new(0.0, f64::NAN, 0.0),
    ///     Err(Error::InvalidValue { field: "north", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(east_m: f64, north_m: f64, up_m: f64) -> Result<Self> {
        let east_m = finite(east_m, "east")?;
        let north_m = finite(north_m, "north")?;
        let up_m = finite(up_m, "up")?;
        check_magnitude(east_m, north_m, up_m)?;
        Ok(Self {
            east_m,
            north_m,
            up_m,
        })
    }

    /// East offset, in meters.
    ///
    /// ```
    /// assert_eq!(geocoordinates::geodesy::Enu::new(1.0, 2.0, 3.0)?.east_m(), 1.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn east_m(&self) -> f64 {
        self.east_m
    }

    /// North offset, in meters.
    ///
    /// ```
    /// assert_eq!(geocoordinates::geodesy::Enu::new(1.0, 2.0, 3.0)?.north_m(), 2.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn north_m(&self) -> f64 {
        self.north_m
    }

    /// Up offset, in meters.
    ///
    /// ```
    /// assert_eq!(geocoordinates::geodesy::Enu::new(1.0, 2.0, 3.0)?.up_m(), 3.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn up_m(&self) -> f64 {
        self.up_m
    }

    /// The ENU offset of `target` relative to `origin` (exact), on the
    /// ellipsoid of the datum they share.
    ///
    /// # Errors
    /// - [`Error::CrsMismatch`] (expecting [`Crs::Wgs84`](crate::Crs::Wgs84))
    ///   when either coordinate is GCJ-02 or BD-09, which have no ellipsoid;
    /// - [`Error::CrsMismatch`] (expecting the origin's datum) when the two
    ///   coordinates are on different datums;
    /// - [`Error::InvalidValue`] (field `"height"`) when either carries an
    ///   orthometric height, or when the heights are so large that the offset
    ///   overflows.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, Height, geodesy::Enu};
    ///
    /// let origin = Coordinate::wgs84(51.5, 0.0)?;
    /// let mast = Coordinate::wgs84(51.5, 0.0)?.with_height(Height::Ellipsoidal(120.0))?;
    /// let enu = Enu::try_from_coordinate(mast, origin)?;
    /// assert!((enu.up_m() - 120.0).abs() < 1e-6 && enu.east_m().abs() < 1e-6);
    /// // Any classic datum works, as long as both points share it.
    /// let nad27 = Coordinate::new(51.5, 0.0, Crs::Nad27)?;
    /// assert!(Enu::try_from_coordinate(nad27, nad27)?.up_m().abs() < 1e-9);
    /// assert!(matches!(
    ///     Enu::try_from_coordinate(nad27, origin),
    ///     Err(Error::CrsMismatch { expected: Crs::Wgs84, found: Crs::Nad27 })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Self> {
        validate_frame_coordinate(target)?;
        validate_frame_coordinate(origin)?;
        if target.crs() != origin.crs() {
            return Err(Error::CrsMismatch {
                expected: origin.crs(),
                found: target.crs(),
            });
        }
        let (sin_lat, cos_lat) = origin.lat().to_radians().sin_cos();
        let (sin_lon, cos_lon) = origin.lon().to_radians().sin_cos();
        let t = Ecef::try_from_coordinate(target)?;
        let o = Ecef::try_from_coordinate(origin)?;
        let (dx, dy, dz) = (t.x_m - o.x_m, t.y_m - o.y_m, t.z_m - o.z_m);
        Enu::new(
            -sin_lon * dx + cos_lon * dy,
            -sin_lat * cos_lon * dx - sin_lat * sin_lon * dy + cos_lat * dz,
            cos_lat * cos_lon * dx + cos_lat * sin_lon * dy + sin_lat * dz,
        )
        .map_err(|_| Error::InvalidValue {
            field: "height",
            detail: "the offset between target and origin overflows".into(),
        })
    }

    /// Recover the absolute coordinate of this ENU offset about `origin`
    /// (exact, through [`Ecef::try_to_coordinate`]'s closed-form inverse),
    /// tagged with the origin's datum.
    ///
    /// # Errors
    /// - [`Error::CrsMismatch`] and [`Error::InvalidValue`] (field `"height"`)
    ///   for an `origin` [`try_from_coordinate`](Self::try_from_coordinate)
    ///   would reject;
    /// - as for [`Ecef::try_to_coordinate`] when the offset is so large that
    ///   the position overflows.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, geodesy::Enu};
    ///
    /// let origin = Coordinate::new(51.5, 0.0, Crs::Tokyo)?;
    /// let target = Coordinate::new(51.51, 0.01, Crs::Tokyo)?;
    /// let back = Enu::try_from_coordinate(target, origin)?.try_to_coordinate(origin)?;
    /// assert!((back.lat() - 51.51).abs() < 1e-9 && (back.lon() - 0.01).abs() < 1e-9);
    /// assert_eq!(back.crs(), Crs::Tokyo);
    /// let gcj = Coordinate::new(51.5, 0.0, Crs::Gcj02)?;
    /// assert!(matches!(
    ///     Enu::new(0.0, 0.0, 0.0)?.try_to_coordinate(gcj),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_coordinate(self, origin: Coordinate) -> Result<Coordinate> {
        validate_frame_coordinate(origin)?;
        let (sin_lat, cos_lat) = origin.lat().to_radians().sin_cos();
        let (sin_lon, cos_lon) = origin.lon().to_radians().sin_cos();
        // Rotate the ENU offset back to an ECEF difference (the transpose).
        let dx = -sin_lon * self.east_m - sin_lat * cos_lon * self.north_m
            + cos_lat * cos_lon * self.up_m;
        let dy = cos_lon * self.east_m - sin_lat * sin_lon * self.north_m
            + cos_lat * sin_lon * self.up_m;
        let dz = cos_lat * self.north_m + sin_lat * self.up_m;
        let o = Ecef::try_from_coordinate(origin)?;
        Ecef::new(o.x_m + dx, o.y_m + dy, o.z_m + dz).try_to_coordinate(origin.crs())
    }

    /// Convert to the NED convention (exact).
    ///
    /// ```
    /// use geocoordinates::geodesy::{Enu, Ned};
    ///
    /// let enu = Enu::new(1.0, 2.0, 3.0)?;
    /// assert_eq!(enu.to_ned(), Ned::new(2.0, 1.0, -3.0)?);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_ned(self) -> Ned {
        Ned {
            north_m: self.north_m,
            east_m: self.east_m,
            down_m: -self.up_m,
        }
    }

    /// Convert to azimuth/elevation/range (exact). Azimuth is normalized to
    /// `[0, 360)`. A zero offset has elevation 0° and a finite azimuth that
    /// follows the signs of its zeros, as [`f64::atan2`] does: 0° when `north`
    /// is `+0.0`, but 180° when `north` is `-0.0`.
    ///
    /// ```
    /// use geocoordinates::geodesy::Enu;
    ///
    /// let aer = Enu::new(-100.0, 0.0, 100.0)?.to_aer();
    /// assert!((aer.azimuth_deg() - 270.0).abs() < 1e-9);
    /// assert!((aer.elevation_deg() - 45.0).abs() < 1e-9);
    /// assert!((aer.range().meters() - 100.0 * 2f64.sqrt()).abs() < 1e-9);
    ///
    /// // A zero offset: the azimuth depends on the sign of the zero `north`.
    /// let zero = Enu::new(0.0, 0.0, 0.0)?.to_aer();
    /// assert_eq!((zero.azimuth_deg(), zero.elevation_deg()), (0.0, 0.0));
    /// let neg = Enu::new(0.0, -0.0, 0.0)?.to_aer();
    /// assert_eq!((neg.azimuth_deg(), neg.elevation_deg()), (180.0, 0.0));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_aer(self) -> Aer {
        let horizontal = self.east_m.hypot(self.north_m);
        // Finite: `Enu::new` checked this same magnitude.
        let range = magnitude(self.east_m, self.north_m, self.up_m);
        let azimuth_deg =
            crate::angle::normalize_degrees(self.east_m.atan2(self.north_m).to_degrees());
        // atan2 stays in [-90°, 90°] whatever the rounding, where
        // asin(up / range) turns NaN once |up| / range rounds past one; and a
        // zero offset gets a defined 0° elevation. The clamp only guards the
        // invariant against the degree conversion's rounding.
        let elevation_deg = self.up_m.atan2(horizontal).to_degrees().clamp(-90.0, 90.0);
        Aer {
            azimuth_deg,
            elevation_deg,
            range: Length::from_meters(range),
        }
    }
}

impl Ned {
    /// Construct from north, east, and down offsets in meters.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] naming `"north"`, `"east"`, or `"down"` when
    /// that component is not finite, and `"magnitude"` as for [`Enu::new`].
    ///
    /// ```
    /// use geocoordinates::{Error, geodesy::Ned};
    ///
    /// let ned = Ned::new(2.0, 1.0, -3.0)?;
    /// assert_eq!((ned.north_m(), ned.east_m(), ned.down_m()), (2.0, 1.0, -3.0));
    /// assert!(matches!(
    ///     Ned::new(0.0, 0.0, f64::NAN),
    ///     Err(Error::InvalidValue { field: "down", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(north_m: f64, east_m: f64, down_m: f64) -> Result<Self> {
        let north_m = finite(north_m, "north")?;
        let east_m = finite(east_m, "east")?;
        let down_m = finite(down_m, "down")?;
        check_magnitude(north_m, east_m, down_m)?;
        Ok(Self {
            north_m,
            east_m,
            down_m,
        })
    }

    /// North offset, in meters.
    ///
    /// ```
    /// assert_eq!(geocoordinates::geodesy::Ned::new(2.0, 1.0, -3.0)?.north_m(), 2.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn north_m(&self) -> f64 {
        self.north_m
    }

    /// East offset, in meters.
    ///
    /// ```
    /// assert_eq!(geocoordinates::geodesy::Ned::new(2.0, 1.0, -3.0)?.east_m(), 1.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn east_m(&self) -> f64 {
        self.east_m
    }

    /// Down offset, in meters (positive below the origin).
    ///
    /// ```
    /// assert_eq!(geocoordinates::geodesy::Ned::new(2.0, 1.0, -3.0)?.down_m(), -3.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn down_m(&self) -> f64 {
        self.down_m
    }

    /// The NED offset of `target` relative to `origin` (exact).
    ///
    /// # Errors
    /// As for [`Enu::try_from_coordinate`].
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, Height, geodesy::Ned};
    ///
    /// let origin = Coordinate::wgs84(-33.9, 151.2)?.with_height(Height::Ellipsoidal(50.0))?;
    /// let ground = Coordinate::wgs84(-33.9, 151.2)?;
    /// assert!((Ned::try_from_coordinate(ground, origin)?.down_m() - 50.0).abs() < 1e-6);
    /// let msl = Coordinate::wgs84(-33.9, 151.2)?.with_height(Height::Orthometric(50.0))?;
    /// assert!(matches!(
    ///     Ned::try_from_coordinate(ground, msl),
    ///     Err(Error::InvalidValue { field: "height", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Self> {
        Ok(Enu::try_from_coordinate(target, origin)?.to_ned())
    }

    /// Recover the absolute coordinate of this NED offset about `origin`
    /// (exact; see [`Enu::try_to_coordinate`]).
    ///
    /// # Errors
    /// As for [`Enu::try_to_coordinate`].
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, Height, geodesy::Ned};
    ///
    /// let origin = Coordinate::new(10.0, 20.0, Crs::Pulkovo42)?;
    /// let below = Ned::new(0.0, 0.0, 25.0)?.try_to_coordinate(origin)?;
    /// assert!(matches!(below.height(), Some(Height::Ellipsoidal(h)) if (h + 25.0).abs() < 1e-6));
    /// let bd09 = Coordinate::new(10.0, 20.0, Crs::Bd09)?;
    /// assert!(matches!(
    ///     Ned::new(0.0, 0.0, 25.0)?.try_to_coordinate(bd09),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_coordinate(self, origin: Coordinate) -> Result<Coordinate> {
        self.to_enu().try_to_coordinate(origin)
    }

    /// Convert to the ENU convention (exact).
    ///
    /// ```
    /// use geocoordinates::geodesy::{Enu, Ned};
    ///
    /// let ned = Ned::new(2.0, 1.0, -3.0)?;
    /// assert_eq!(ned.to_enu(), Enu::new(1.0, 2.0, 3.0)?);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_enu(self) -> Enu {
        Enu {
            east_m: self.east_m,
            north_m: self.north_m,
            up_m: -self.down_m,
        }
    }

    /// Convert to azimuth/elevation/range (exact; see [`Enu::to_aer`]).
    ///
    /// ```
    /// use geocoordinates::geodesy::Ned;
    ///
    /// // Straight down: elevation −90°.
    /// let aer = Ned::new(0.0, 0.0, 30.0)?.to_aer();
    /// assert_eq!((aer.elevation_deg(), aer.range().meters()), (-90.0, 30.0));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_aer(self) -> Aer {
        self.to_enu().to_aer()
    }
}

mod aer;

fn validate_frame_coordinate(coord: Coordinate) -> Result<()> {
    ellipsoid_of(coord.crs())?;
    if matches!(coord.height(), Some(Height::Orthometric(_))) {
        return Err(Error::InvalidValue {
            field: "height",
            detail: "local frames require ellipsoidal height".into(),
        });
    }
    Ok(())
}

/// Unvalidated serde shape of [`Enu`]; deserialization goes through
/// [`Enu::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct EnuRecord {
    east_m: f64,
    north_m: f64,
    up_m: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<EnuRecord> for Enu {
    type Error = Error;

    fn try_from(r: EnuRecord) -> Result<Self> {
        Enu::new(r.east_m, r.north_m, r.up_m)
    }
}

/// Unvalidated serde shape of [`Ned`]; deserialization goes through
/// [`Ned::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct NedRecord {
    north_m: f64,
    east_m: f64,
    down_m: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<NedRecord> for Ned {
    type Error = Error;

    fn try_from(r: NedRecord) -> Result<Self> {
        Ned::new(r.north_m, r.east_m, r.down_m)
    }
}

/// Unvalidated serde shape of [`Aer`]; deserialization goes through
/// [`Aer::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct AerRecord {
    azimuth_deg: f64,
    elevation_deg: f64,
    range: Length,
}

#[cfg(feature = "serde")]
impl TryFrom<AerRecord> for Aer {
    type Error = Error;

    fn try_from(r: AerRecord) -> Result<Self> {
        Aer::new(r.azimuth_deg, r.elevation_deg, r.range)
    }
}

// Frame-to-frame conversions are exact and origin-independent (pure rotation /
// repackaging), so they also implement `From`, per the conversion convention.
impl From<Enu> for Ned {
    fn from(enu: Enu) -> Ned {
        enu.to_ned()
    }
}
impl From<Ned> for Enu {
    fn from(ned: Ned) -> Enu {
        ned.to_enu()
    }
}
impl From<Enu> for Aer {
    fn from(enu: Enu) -> Aer {
        enu.to_aer()
    }
}
impl From<Aer> for Enu {
    fn from(aer: Aer) -> Enu {
        aer.to_enu()
    }
}
impl From<Ned> for Aer {
    fn from(ned: Ned) -> Aer {
        ned.to_aer()
    }
}
impl From<Aer> for Ned {
    fn from(aer: Aer) -> Ned {
        aer.to_ned()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod component_tests;
