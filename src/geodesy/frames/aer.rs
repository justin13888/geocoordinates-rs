//! Construction, accessors, and conversions of [`Aer`].

use super::{Aer, Enu, Ned, finite, magnitude};
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::units::Length;

impl Aer {
    /// Construct from an azimuth and an elevation in degrees and a slant
    /// range. The azimuth is normalized to `[0, 360)`, so `-90.0` and `270.0`
    /// build the same value.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] naming:
    /// - `"azimuth"` when the azimuth is not finite;
    /// - `"elevation"` when the elevation is not finite or lies outside
    ///   `[-90, 90]`;
    /// - `"range"` when the range is not finite or is negative.
    ///
    /// ```
    /// use geocoordinates::{Error, Length, geodesy::Aer};
    ///
    /// let aer = Aer::new(-90.0, 10.0, Length::from_meters(250.0))?;
    /// assert_eq!(aer.azimuth_deg(), 270.0);
    /// assert!(matches!(
    ///     Aer::new(0.0, 0.0, Length::from_meters(-1.0)),
    ///     Err(Error::InvalidValue { field: "range", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(azimuth_deg: f64, elevation_deg: f64, range: Length) -> Result<Self> {
        let azimuth_deg = crate::angle::normalize_degrees(finite(azimuth_deg, "azimuth")?);
        if !(-90.0..=90.0).contains(&elevation_deg) {
            return Err(Error::InvalidValue {
                field: "elevation",
                detail: format!("{elevation_deg} is not in [-90, 90]"),
            });
        }
        let meters = range.meters();
        if !(meters.is_finite() && meters >= 0.0) {
            return Err(Error::InvalidValue {
                field: "range",
                detail: format!("{meters} m is not finite and non-negative"),
            });
        }
        Ok(Self {
            azimuth_deg,
            elevation_deg,
            // `+ 0.0` turns a `-0.0` range into `+0.0`.
            range: Length::from_meters(meters + 0.0),
        })
    }

    /// Azimuth, in degrees clockwise from north, in `[0, 360)`.
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Aer};
    ///
    /// assert_eq!(Aer::new(450.0, 0.0, Length::from_meters(1.0))?.azimuth_deg(), 90.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn azimuth_deg(&self) -> f64 {
        self.azimuth_deg
    }

    /// Elevation, in degrees above the local horizontal, in `[-90, 90]`.
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Aer};
    ///
    /// assert_eq!(Aer::new(0.0, -5.0, Length::from_meters(1.0))?.elevation_deg(), -5.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn elevation_deg(&self) -> f64 {
        self.elevation_deg
    }

    /// Slant range: finite and non-negative.
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Aer};
    ///
    /// assert_eq!(Aer::new(0.0, 0.0, Length::from_meters(7.5))?.range().meters(), 7.5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn range(&self) -> Length {
        self.range
    }

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
    /// assert!((aer.elevation_deg() - 90.0).abs() < 1e-9);
    /// assert!((aer.range().meters() - 1000.0).abs() < 1e-6);
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
    /// As for [`Enu::try_to_coordinate`].
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, Height, Length, geodesy::Aer};
    ///
    /// let origin = Coordinate::wgs84(45.0, 7.0)?;
    /// let north = Aer::new(0.0, 0.0, Length::from_meters(1000.0))?;
    /// let p = north.try_to_coordinate(origin)?;
    /// assert!(p.lat() > 45.0 && (p.lon() - 7.0).abs() < 1e-9);
    /// let msl = origin.with_height(Height::Orthometric(10.0))?;
    /// assert!(matches!(
    ///     north.try_to_coordinate(msl),
    ///     Err(Error::InvalidValue { field: "height", .. })
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
    /// let overhead = Aer::new(0.0, 90.0, Length::from_meters(50.0))?;
    /// let enu = overhead.to_enu();
    /// assert!((enu.up_m() - 50.0).abs() < 1e-12 && enu.north_m().abs() < 1e-12);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_enu(self) -> Enu {
        let (sin_az, cos_az) = self.azimuth_deg.to_radians().sin_cos();
        let (sin_el, cos_el) = self.elevation_deg.to_radians().sin_cos();
        let r = self.range.meters();
        // Each component is at most `r`, so finite.
        let (mut east_m, mut north_m, mut up_m) =
            (r * cos_el * sin_az, r * cos_el * cos_az, r * sin_el);
        // Only a range within a few ulps of `f64::MAX` can round the
        // components' magnitude past it; pull them in by round-off until it is
        // finite, as `Enu::new` requires. Each step shrinks the largest
        // component by at least one ulp, so this ends within a few steps.
        while !magnitude(east_m, north_m, up_m).is_finite() {
            let shrink = 1.0 - f64::EPSILON;
            (east_m, north_m, up_m) = (east_m * shrink, north_m * shrink, up_m * shrink);
        }
        Enu {
            east_m,
            north_m,
            up_m,
        }
    }

    /// Convert to the NED convention (exact).
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Aer};
    ///
    /// let below = Aer::new(0.0, -90.0, Length::from_meters(5.0))?;
    /// assert!((below.to_ned().down_m() - 5.0).abs() < 1e-12);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_ned(self) -> Ned {
        self.to_enu().to_ned()
    }
}
