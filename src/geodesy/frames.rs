//! Local tangent-plane frames: ENU, NED, and AER.
//!
//! These are defined relative to a reference origin, so they are expressed as
//! methods taking the origin rather than `From` impls. The forward direction
//! (coordinate → ENU / NED / AER), the frame-to-frame conversions, and the
//! inverse (`try_to_coordinate`) are all **exact** to floating-point round-off:
//! a rotation of the ECEF difference vector, and back through
//! [`Ecef::try_to_coordinate`]'s closed-form inverse. Construction remains
//! fallible so both positions must be valid WGS-84 coordinates with compatible
//! ellipsoidal heights.
//!
//! ```
//! use geocoordinates::{Coordinate, geodesy::{Aer, Enu, Ned}};
//!
//! let origin = Coordinate::wgs84(40.0, -75.0)?;
//! let target = Coordinate::wgs84(40.01, -75.0)?; // ~1.1 km due north
//! let enu = Enu::try_from_coordinate(target, origin)?;
//! let ned: Ned = enu.into(); // exact repackaging; `to_ned` is the named form
//! let aer: Aer = enu.to_aer();
//! assert_eq!(ned.north_m, enu.north_m);
//! assert!(enu.east_m.abs() < 1e-6 && aer.elevation_deg < 0.0); // the Earth curves away
//! # Ok::<(), geocoordinates::Error>(())
//! ```

use super::ecef::Ecef;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::units::Length;

/// East-North-Up offset from a reference origin, in meters.
///
/// ```
/// use geocoordinates::geodesy::Enu;
///
/// let up_and_east = Enu { east_m: 3.0, north_m: 0.0, up_m: 4.0 };
/// assert_eq!(up_and_east.to_aer().range.meters(), 5.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Enu {
    /// East offset (meters).
    pub east_m: f64,
    /// North offset (meters).
    pub north_m: f64,
    /// Up offset (meters).
    pub up_m: f64,
}

/// North-East-Down offset from a reference origin, in meters.
///
/// ```
/// use geocoordinates::geodesy::Ned;
///
/// // 10 m below the origin: a positive `down_m`.
/// let below = Ned { north_m: 0.0, east_m: 0.0, down_m: 10.0 };
/// assert_eq!(below.to_enu().up_m, -10.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ned {
    /// North offset (meters).
    pub north_m: f64,
    /// East offset (meters).
    pub east_m: f64,
    /// Down offset (meters).
    pub down_m: f64,
}

/// Azimuth-Elevation-Range relative to a reference origin.
///
/// ```
/// use geocoordinates::{Length, geodesy::Aer};
///
/// let due_east = Aer { azimuth_deg: 90.0, elevation_deg: 0.0, range: Length::from_meters(100.0) };
/// let enu = due_east.to_enu();
/// assert!((enu.east_m - 100.0).abs() < 1e-9 && enu.north_m.abs() < 1e-9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aer {
    /// Azimuth (degrees clockwise from north).
    pub azimuth_deg: f64,
    /// Elevation (degrees above the local horizontal).
    pub elevation_deg: f64,
    /// Slant range.
    pub range: Length,
}

impl Enu {
    /// The ENU offset of `target` relative to `origin` (exact, WGS-84).
    ///
    /// # Errors
    /// - [`Error::CrsMismatch`] (expecting [`Crs::Wgs84`](crate::Crs::Wgs84))
    ///   when either coordinate is not WGS-84;
    /// - [`Error::InvalidValue`] (field `"height"`) when either carries an
    ///   orthometric height.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, Height, geodesy::Enu};
    ///
    /// let origin = Coordinate::wgs84(51.5, 0.0)?;
    /// let mast = Coordinate::wgs84(51.5, 0.0)?.with_height(Height::Ellipsoidal(120.0))?;
    /// let enu = Enu::try_from_coordinate(mast, origin)?;
    /// assert!((enu.up_m - 120.0).abs() < 1e-6 && enu.east_m.abs() < 1e-6);
    /// // Local frames are WGS-84 only.
    /// let nad27 = Coordinate::new(51.5, 0.0, Crs::Nad27)?;
    /// assert!(matches!(
    ///     Enu::try_from_coordinate(nad27, origin),
    ///     Err(Error::CrsMismatch { .. })
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
        Ok(Enu {
            east_m: -sin_lon * dx + cos_lon * dy,
            north_m: -sin_lat * cos_lon * dx - sin_lat * sin_lon * dy + cos_lat * dz,
            up_m: cos_lat * cos_lon * dx + cos_lat * sin_lon * dy + sin_lat * dz,
        })
    }

    /// Recover the absolute coordinate of this ENU offset about `origin`
    /// (exact, through [`Ecef::try_to_coordinate`]'s closed-form inverse).
    ///
    /// # Errors
    /// - [`Error::CrsMismatch`] and [`Error::InvalidValue`] (field `"height"`)
    ///   for an `origin` [`try_from_coordinate`](Self::try_from_coordinate)
    ///   would reject;
    /// - [`Error::InvalidValue`] (field `"ENU"`) when a component is not
    ///   finite, and as for [`Ecef::try_to_coordinate`] when the offset is so
    ///   large that the position overflows.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, geodesy::Enu};
    ///
    /// let origin = Coordinate::wgs84(51.5, 0.0)?;
    /// let target = Coordinate::wgs84(51.51, 0.01)?;
    /// let back = Enu::try_from_coordinate(target, origin)?.try_to_coordinate(origin)?;
    /// assert!((back.lat() - 51.51).abs() < 1e-9 && (back.lon() - 0.01).abs() < 1e-9);
    /// let bad = Enu { east_m: f64::NAN, north_m: 0.0, up_m: 0.0 };
    /// assert!(matches!(
    ///     bad.try_to_coordinate(origin),
    ///     Err(Error::InvalidValue { field: "ENU", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_coordinate(self, origin: Coordinate) -> Result<Coordinate> {
        validate_frame_coordinate(origin)?;
        if !self.east_m.is_finite() || !self.north_m.is_finite() || !self.up_m.is_finite() {
            return Err(Error::InvalidValue {
                field: "ENU",
                detail: "components must be finite".into(),
            });
        }
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
    /// let enu = Enu { east_m: 1.0, north_m: 2.0, up_m: 3.0 };
    /// assert_eq!(enu.to_ned(), Ned { north_m: 2.0, east_m: 1.0, down_m: -3.0 });
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
    /// let aer = Enu { east_m: -100.0, north_m: 0.0, up_m: 100.0 }.to_aer();
    /// assert!((aer.azimuth_deg - 270.0).abs() < 1e-9);
    /// assert!((aer.elevation_deg - 45.0).abs() < 1e-9);
    /// assert!((aer.range.meters() - 100.0 * 2f64.sqrt()).abs() < 1e-9);
    ///
    /// // A zero offset: the azimuth depends on the sign of the zero `north`.
    /// let zero = Enu { east_m: 0.0, north_m: 0.0, up_m: 0.0 }.to_aer();
    /// assert_eq!((zero.azimuth_deg, zero.elevation_deg), (0.0, 0.0));
    /// let neg = Enu { east_m: 0.0, north_m: -0.0, up_m: 0.0 }.to_aer();
    /// assert_eq!((neg.azimuth_deg, neg.elevation_deg), (180.0, 0.0));
    /// ```
    #[must_use]
    pub fn to_aer(self) -> Aer {
        // `hypot` neither overflows nor underflows where the squares would.
        let horizontal = self.east_m.hypot(self.north_m);
        let range = horizontal.hypot(self.up_m);
        let azimuth_deg =
            crate::angle::normalize_degrees(self.east_m.atan2(self.north_m).to_degrees());
        // atan2 stays in [-90°, 90°] whatever the rounding, where
        // asin(up / range) turns NaN once |up| / range rounds past one; and a
        // zero offset gets a defined 0° elevation.
        let elevation_deg = self.up_m.atan2(horizontal).to_degrees();
        Aer {
            azimuth_deg,
            elevation_deg,
            range: Length::from_meters(range),
        }
    }
}

impl Ned {
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
    /// assert!((Ned::try_from_coordinate(ground, origin)?.down_m - 50.0).abs() < 1e-6);
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
    /// let origin = Coordinate::wgs84(10.0, 20.0)?;
    /// let below = Ned { north_m: 0.0, east_m: 0.0, down_m: 25.0 }.try_to_coordinate(origin)?;
    /// assert!(matches!(below.height(), Some(Height::Ellipsoidal(h)) if (h + 25.0).abs() < 1e-6));
    /// let tokyo = Coordinate::new(10.0, 20.0, Crs::Tokyo)?;
    /// assert!(matches!(
    ///     Ned { north_m: 0.0, east_m: 0.0, down_m: 25.0 }.try_to_coordinate(tokyo),
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
    /// let ned = Ned { north_m: 2.0, east_m: 1.0, down_m: -3.0 };
    /// assert_eq!(ned.to_enu(), Enu { east_m: 1.0, north_m: 2.0, up_m: 3.0 });
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
    /// let aer = Ned { north_m: 0.0, east_m: 0.0, down_m: 30.0 }.to_aer();
    /// assert_eq!((aer.elevation_deg, aer.range.meters()), (-90.0, 30.0));
    /// ```
    #[must_use]
    pub fn to_aer(self) -> Aer {
        self.to_enu().to_aer()
    }
}

impl Aer {
    /// The azimuth/elevation/range of `target` relative to `origin` (exact).
    ///
    /// # Errors
    /// As for [`Enu::try_from_coordinate`].
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, Height, geodesy::Aer};
    ///
    /// let observer = Coordinate::wgs84(0.0, 0.0)?;
    /// let balloon = Coordinate::wgs84(0.0, 0.0)?.with_height(Height::Ellipsoidal(1000.0))?;
    /// let aer = Aer::try_from_coordinate(balloon, observer)?;
    /// assert!((aer.elevation_deg - 90.0).abs() < 1e-9);
    /// assert!((aer.range.meters() - 1000.0).abs() < 1e-6);
    /// let gcj = Coordinate::new(0.0, 0.0, Crs::Gcj02)?;
    /// assert!(matches!(
    ///     Aer::try_from_coordinate(gcj, observer),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Self> {
        Ok(Enu::try_from_coordinate(target, origin)?.to_aer())
    }

    /// Recover the absolute coordinate of this AER offset about `origin`
    /// (exact; see [`Enu::try_to_coordinate`]).
    ///
    /// # Errors
    /// As for [`Enu::try_to_coordinate`]; a non-finite azimuth, elevation, or
    /// range is reported with field `"ENU"`.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, Length, geodesy::Aer};
    ///
    /// let origin = Coordinate::wgs84(45.0, 7.0)?;
    /// let north = Aer { azimuth_deg: 0.0, elevation_deg: 0.0, range: Length::from_meters(1000.0) };
    /// let p = north.try_to_coordinate(origin)?;
    /// assert!(p.lat() > 45.0 && (p.lon() - 7.0).abs() < 1e-9);
    /// let bad = Aer { range: Length::from_meters(f64::INFINITY), ..north };
    /// assert!(matches!(
    ///     bad.try_to_coordinate(origin),
    ///     Err(Error::InvalidValue { field: "ENU", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_coordinate(self, origin: Coordinate) -> Result<Coordinate> {
        self.to_enu().try_to_coordinate(origin)
    }

    /// Convert to the ENU convention (exact).
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Aer};
    ///
    /// let overhead = Aer { azimuth_deg: 0.0, elevation_deg: 90.0, range: Length::from_meters(50.0) };
    /// let enu = overhead.to_enu();
    /// assert!((enu.up_m - 50.0).abs() < 1e-12 && enu.north_m.abs() < 1e-12);
    /// ```
    #[must_use]
    pub fn to_enu(self) -> Enu {
        let az = self.azimuth_deg.to_radians();
        let el = self.elevation_deg.to_radians();
        let r = self.range.meters();
        Enu {
            east_m: r * el.cos() * az.sin(),
            north_m: r * el.cos() * az.cos(),
            up_m: r * el.sin(),
        }
    }

    /// Convert to the NED convention (exact).
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Aer};
    ///
    /// let below = Aer { azimuth_deg: 0.0, elevation_deg: -90.0, range: Length::from_meters(5.0) };
    /// assert!((below.to_ned().down_m - 5.0).abs() < 1e-12);
    /// ```
    #[must_use]
    pub fn to_ned(self) -> Ned {
        self.to_enu().to_ned()
    }
}

fn validate_frame_coordinate(coord: Coordinate) -> Result<()> {
    if coord.crs() != crate::Crs::Wgs84 {
        return Err(Error::CrsMismatch {
            expected: crate::Crs::Wgs84,
            found: coord.crs(),
        });
    }
    if matches!(coord.height(), Some(crate::Height::Orthometric(_))) {
        return Err(Error::InvalidValue {
            field: "height",
            detail: "local frames require ellipsoidal height".into(),
        });
    }
    Ok(())
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
mod tests {
    use super::*;
    use crate::coord::Height;
    use crate::test_support::assert_close;

    #[test]
    fn enu_directions() {
        let origin = Coordinate::wgs84(40.0, -75.0).unwrap();
        // A point due north of the origin has +north, ~0 east.
        let north =
            Enu::try_from_coordinate(Coordinate::wgs84(40.1, -75.0).unwrap(), origin).unwrap();
        assert!(north.north_m > 10_000.0);
        assert_close(north.east_m, 0.0, 1e-6);
        // A point due east is predominantly east; the small north component is
        // the parallel's poleward curvature (~5 m over ~8.5 km).
        let east =
            Enu::try_from_coordinate(Coordinate::wgs84(40.0, -74.9).unwrap(), origin).unwrap();
        assert!(east.east_m > 8_000.0);
        assert!(east.north_m.abs() < 50.0);
    }

    #[test]
    fn enu_round_trip() {
        let origin = Coordinate::wgs84(40.7128, -74.006).unwrap();
        let target = Coordinate::wgs84(40.73, -74.0)
            .unwrap()
            .with_height(Height::Ellipsoidal(120.0))
            .unwrap();
        let back = Enu::try_from_coordinate(target, origin)
            .unwrap()
            .try_to_coordinate(origin)
            .unwrap();
        assert_close(back.lat(), 40.73, 1e-9);
        assert_close(back.lon(), -74.0, 1e-9);
        match back.height() {
            Some(Height::Ellipsoidal(h)) => assert_close(h, 120.0, 1e-4),
            other => panic!("expected ellipsoidal height, got {other:?}"),
        }
    }

    #[test]
    fn enu_to_aer_reference() {
        let north = Enu {
            east_m: 0.0,
            north_m: 100.0,
            up_m: 0.0,
        }
        .to_aer();
        assert_close(north.azimuth_deg, 0.0, 1e-9);
        assert_close(north.elevation_deg, 0.0, 1e-9);
        assert_close(north.range.meters(), 100.0, 1e-9);

        let east = Enu {
            east_m: 100.0,
            north_m: 0.0,
            up_m: 0.0,
        }
        .to_aer();
        assert_close(east.azimuth_deg, 90.0, 1e-9);

        let up = Enu {
            east_m: 0.0,
            north_m: 0.0,
            up_m: 100.0,
        }
        .to_aer();
        assert_close(up.elevation_deg, 90.0, 1e-9);
        assert_close(up.range.meters(), 100.0, 1e-9);
    }

    #[test]
    fn aer_to_enu_reference() {
        let enu = Aer {
            azimuth_deg: 45.0,
            elevation_deg: 0.0,
            range: Length::from_meters(100.0),
        }
        .to_enu();
        let component = 100.0 * 45.0_f64.to_radians().sin();
        assert_close(enu.east_m, component, 1e-9);
        assert_close(enu.north_m, component, 1e-9);
        assert_close(enu.up_m, 0.0, 1e-9);
    }

    #[test]
    fn aer_enu_round_trip() {
        let aer = Aer {
            azimuth_deg: 120.0,
            elevation_deg: 30.0,
            range: Length::from_meters(5000.0),
        };
        let back = aer.to_enu().to_aer();
        assert_close(back.azimuth_deg, 120.0, 1e-9);
        assert_close(back.elevation_deg, 30.0, 1e-9);
        assert_close(back.range.meters(), 5000.0, 1e-9);
    }

    #[test]
    fn enu_ned_repackaging() {
        let enu = Enu {
            east_m: 1.0,
            north_m: 2.0,
            up_m: 3.0,
        };
        let ned = enu.to_ned();
        assert_close(ned.north_m, 2.0, 0.0);
        assert_close(ned.east_m, 1.0, 0.0);
        assert_close(ned.down_m, -3.0, 0.0);
        // Round-trips back to ENU.
        let enu2 = ned.to_enu();
        assert_close(enu2.up_m, 3.0, 0.0);
    }

    #[test]
    fn zero_enu_aer_is_well_defined() {
        // A zero offset has zero range and a defined (not NaN) elevation.
        let aer = Enu {
            east_m: 0.0,
            north_m: 0.0,
            up_m: 0.0,
        }
        .to_aer();
        assert_close(aer.range.meters(), 0.0, 0.0);
        assert_close(aer.elevation_deg, 0.0, 0.0);
    }

    /// N4: elevation and range hold where squaring the components would
    /// underflow or overflow, and elevation is never NaN.
    #[test]
    fn aer_survives_extreme_magnitudes() {
        for up in [1e-200, 1e200, -1e-200, -1e200] {
            let aer = Enu {
                east_m: 0.0,
                north_m: 0.0,
                up_m: up,
            }
            .to_aer();
            assert_close(aer.elevation_deg, 90.0_f64.copysign(up), 0.0);
            assert_close(aer.range.meters(), up.abs(), 0.0);
        }
        let flat = Enu {
            east_m: 3e200,
            north_m: 4e200,
            up_m: 0.0,
        }
        .to_aer();
        assert_close(flat.range.meters(), 5e200, 1e186);
        assert_close(flat.elevation_deg, 0.0, 0.0);
        // A sweep of ordinary offsets never produces a NaN elevation, and
        // matches asin(up / range) wherever that is defined.
        for i in 1..2000 {
            let x = f64::from(i) * 0.123_456_789;
            let enu = Enu {
                east_m: x.sin() * 1e-3,
                north_m: x.cos() * 1e-3,
                up_m: x,
            };
            let aer = enu.to_aer();
            assert!(aer.elevation_deg.is_finite(), "{enu:?}");
            let r = aer.range.meters();
            assert_close(
                aer.elevation_deg,
                (enu.up_m / r).min(1.0).asin().to_degrees(),
                1e-6,
            );
        }
    }

    #[test]
    fn coordinate_frames_reject_wrong_crs_and_height_semantics() {
        let origin = Coordinate::wgs84(40.0, -75.0).unwrap();
        assert!(Enu::try_from_coordinate(Coordinate::gcj02(40.1, -75.0).unwrap(), origin).is_err());
        let orthometric = origin.with_height(Height::Orthometric(10.0)).unwrap();
        assert!(Enu::try_from_coordinate(origin, orthometric).is_err());
    }

    /// The frame check itself (not the pairwise CRS comparison or the ECEF
    /// step) rejects a matching non-WGS-84 pair and an orthometric origin.
    #[test]
    fn frame_check_rejects_a_matching_non_wgs84_pair_and_orthometric_origin() {
        let gcj = Coordinate::gcj02(40.0, -75.0).unwrap();
        let mismatch = Err(Error::CrsMismatch {
            expected: crate::Crs::Wgs84,
            found: crate::Crs::Gcj02,
        });
        assert_eq!(Enu::try_from_coordinate(gcj, gcj), mismatch);
        let zero = Enu {
            east_m: 0.0,
            north_m: 0.0,
            up_m: 0.0,
        };
        assert_eq!(zero.try_to_coordinate(gcj), mismatch.map(|_: Enu| gcj));
        let orthometric = Coordinate::wgs84(40.0, -75.0)
            .unwrap()
            .with_height(Height::Orthometric(10.0))
            .unwrap();
        assert_eq!(
            zero.try_to_coordinate(orthometric),
            Err(Error::InvalidValue {
                field: "height",
                detail: "local frames require ellipsoidal height".into(),
            })
        );
    }
}

#[cfg(test)]
mod component_tests;
