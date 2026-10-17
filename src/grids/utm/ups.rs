//! UPS (Universal Polar Stereographic): the polar caps where UTM is undefined.

use super::{A_AXIS, F, UtmHemisphere, check_range};
use crate::angle::{clamp_latitude, wrap_longitude};
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::grids::require_wgs84;

/// Largest valid UPS easting or northing (meters); the smallest is 0.
const UPS_MAX: f64 = 4_000_000.0;
/// UPS false easting/northing (meters), shared by both polar zones.
const UPS_FALSE: f64 = 2_000_000.0;
/// UPS central scale factor — the point scale factor at the pole, and the
/// minimum anywhere in the projection (k grows away from the pole).
pub(crate) const UPS_K0: f64 = 0.994;

/// A UPS coordinate for the polar regions where UTM is undefined.
///
/// Valid by construction — build one with [`Ups::new`] or
/// [`Ups::try_from_coordinate`]. The domain is easting and northing each in
/// `0..=4 000 000` m.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::grids::Ups;
///
/// let back = Ups::try_from_coordinate(Coordinate::wgs84(-85.0, 30.0)?)?.to_coordinate();
/// assert!((back.lat() + 85.0).abs() < 1e-9 && (back.lon() - 30.0).abs() < 1e-9);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "UpsRecord"))]
pub struct Ups {
    hemisphere: UtmHemisphere,
    easting: f64,
    northing: f64,
}

/// Unvalidated serde shape of [`Ups`]; deserialization goes through
/// [`Ups::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct UpsRecord {
    hemisphere: UtmHemisphere,
    easting: f64,
    northing: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<UpsRecord> for Ups {
    type Error = Error;

    fn try_from(r: UpsRecord) -> Result<Self> {
        Ups::new(r.hemisphere, r.easting, r.northing)
    }
}

impl Ups {
    /// Build a UPS coordinate from its parts, validating each against the UPS
    /// domain.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::grids::{Ups, UtmHemisphere};
    ///
    /// // The false origin is the pole itself.
    /// let pole = Ups::new(UtmHemisphere::North, 2_000_000.0, 2_000_000.0)?;
    /// assert!((pole.to_coordinate().lat() - 90.0).abs() < 1e-9);
    /// let err = Ups::new(UtmHemisphere::North, 2_000_000.0, f64::NAN);
    /// assert!(matches!(err, Err(Error::InvalidValue { field: "northing", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// [`Error::InvalidValue`] naming the offending field, `"easting"` or
    /// `"northing"`, when it lies outside `0..=4 000 000` m (NaN and the
    /// infinities included).
    pub fn new(hemisphere: UtmHemisphere, easting: f64, northing: f64) -> Result<Ups> {
        check_range("easting", easting, 0.0, UPS_MAX)?;
        check_range("northing", northing, 0.0, UPS_MAX)?;
        Ok(Ups {
            hemisphere,
            easting,
            northing,
        })
    }

    /// North or south polar zone.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::grids::{Ups, UtmHemisphere};
    ///
    /// let ups = Ups::try_from_coordinate(Coordinate::wgs84(-85.0, 30.0)?)?;
    /// assert_eq!(ups.hemisphere(), UtmHemisphere::South);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn hemisphere(&self) -> UtmHemisphere {
        self.hemisphere
    }

    /// Easting in meters (2 000 000 m false easting applied).
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::grids::Ups;
    ///
    /// // On the prime meridian the easting is the false easting.
    /// let ups = Ups::try_from_coordinate(Coordinate::wgs84(85.0, 0.0)?)?;
    /// assert!((ups.easting_m() - 2_000_000.0).abs() < 1e-6);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn easting_m(&self) -> f64 {
        self.easting
    }

    /// Northing in meters (2 000 000 m false northing applied).
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::grids::Ups;
    ///
    /// // 85°N on the prime meridian lies about 556 km grid-south of the pole.
    /// let ups = Ups::try_from_coordinate(Coordinate::wgs84(85.0, 0.0)?)?;
    /// assert!((ups.northing_m() - 1_444_542.609).abs() < 1e-3);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn northing_m(&self) -> f64 {
        self.northing
    }

    /// Geodetic WGS-84 → UPS. The canonical inherent form behind
    /// [`TryFrom<Coordinate>`](Ups).
    ///
    /// ```
    /// use geocoordinates::grids::Ups;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let ups = Ups::try_from_coordinate(Coordinate::wgs84(87.0, 45.0)?)?;
    /// assert!((ups.easting_m() - 2_235_568.725).abs() < 1e-3);
    /// // 83.9°N is still UTM territory.
    /// let utm_band = Ups::try_from_coordinate(Coordinate::wgs84(83.9, 0.0)?);
    /// assert!(matches!(utm_band, Err(Error::OutsideDomain { operation: "UPS", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// [`Error::CrsMismatch`] for a non-WGS-84 coordinate;
    /// [`Error::OutsideDomain`] (operation `"UPS"`) for latitudes outside the
    /// polar zones (strictly between 80°S and 84°N, which is UTM territory).
    pub fn try_from_coordinate(coord: Coordinate) -> Result<Ups> {
        require_wgs84(&coord)?;
        let (lat, lon) = (coord.lat(), coord.lon());
        let north = lat >= 0.0;
        if (north && lat < 84.0) || (!north && lat > -80.0) {
            return Err(Error::OutsideDomain {
                operation: "UPS",
                lat,
                lon,
            });
        }
        let (easting, northing) = ups_forward(lat, lon, north);
        // The caps reach at most ~1 120 km from the pole, well inside the
        // 0..=4 000 km `Ups::new` domain around the 2 000 km false origin.
        Ok(Ups {
            hemisphere: if north {
                UtmHemisphere::North
            } else {
                UtmHemisphere::South
            },
            easting,
            northing,
        })
    }

    /// UPS → geodetic WGS-84 coordinate, by the inverse polar stereographic
    /// projection. Latitude comes from a conformal-latitude series truncated at
    /// `e⁶`, accurate to about 0.05 mm; not exact.
    ///
    /// Infallible: the domain was validated at construction. The longitude is
    /// wrapped into `[-180, 180)`.
    ///
    /// ```
    /// use geocoordinates::grids::{Ups, UtmHemisphere};
    ///
    /// // 556 km grid-north of the north pole is the 180° meridian.
    /// let ups = Ups::new(UtmHemisphere::North, 2_000_000.0, 2_555_457.391)?;
    /// let coord = ups.to_coordinate();
    /// assert!((coord.lat() - 85.0).abs() < 1e-6);
    /// assert_eq!(coord.lon(), -180.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_coordinate(&self) -> Coordinate {
        let north = self.hemisphere == UtmHemisphere::North;
        let (lat, lon) = ups_inverse(self.easting, self.northing, north);
        Coordinate::from_valid(clamp_latitude(lat), wrap_longitude(lon), None, Crs::Wgs84)
    }
}

impl TryFrom<Coordinate> for Ups {
    type Error = crate::Error;

    /// Geodetic → UPS. Thin sugar over [`Ups::try_from_coordinate`].
    fn try_from(coord: Coordinate) -> Result<Self> {
        Ups::try_from_coordinate(coord)
    }
}

/// The WGS-84 constants of the UPS projection, computed once: `f64::sqrt`
/// and `f64::powf` are not `const fn`.
struct PolarConstants {
    /// First eccentricity `e`.
    e: f64,
    /// `2·a·k₀ / √((1+e)^(1+e)·(1−e)^(1−e))`: the polar radius `ρ` per unit
    /// of Snyder's `t` (Snyder 21-33).
    big_a: f64,
}

static POLAR: std::sync::LazyLock<PolarConstants> = std::sync::LazyLock::new(|| {
    let e = (F * (2.0 - F)).sqrt();
    let big_a = 2.0 * A_AXIS * UPS_K0 / ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt();
    PolarConstants { e, big_a }
});

/// Polar stereographic forward (geodetic → UPS easting/northing).
fn ups_forward(lat: f64, lon: f64, north: bool) -> (f64, f64) {
    let PolarConstants { e, big_a } = *POLAR;
    let phi = lat.abs().to_radians();
    let lam = lon.to_radians();
    // Conformal radius from the pole (Snyder 21-33 / 21-34).
    let t = ((core::f64::consts::FRAC_PI_4 - phi / 2.0).tan())
        / ((1.0 - e * phi.sin()) / (1.0 + e * phi.sin())).powf(e / 2.0);
    let r = big_a * t;
    let (e_off, n_off) = if north {
        (r * lam.sin(), -r * lam.cos())
    } else {
        (r * lam.sin(), r * lam.cos())
    };
    (UPS_FALSE + e_off, UPS_FALSE + n_off)
}

/// Polar stereographic inverse (UPS easting/northing → geodetic lat/lon).
fn ups_inverse(easting: f64, northing: f64, north: bool) -> (f64, f64) {
    let PolarConstants { e, big_a } = *POLAR;
    let x = easting - UPS_FALSE;
    let y = northing - UPS_FALSE;
    let r = (x * x + y * y).sqrt();
    let t = r / big_a;
    // Conformal latitude, then series back to geodetic (Snyder 3-5).
    let chi = core::f64::consts::FRAC_PI_2 - 2.0 * t.atan();
    let e2 = e * e;
    let e4 = e2 * e2;
    let e6 = e4 * e2;
    let lat_n = chi
        + (e2 / 2.0 + 5.0 * e4 / 24.0 + e6 / 12.0) * (2.0 * chi).sin()
        + (7.0 * e4 / 48.0 + 29.0 * e6 / 240.0) * (4.0 * chi).sin()
        + (7.0 * e6 / 120.0) * (6.0 * chi).sin();
    // North forward sets y = −r·cosλ, south sets y = +r·cosλ; invert each.
    let lon = if north { x.atan2(-y) } else { x.atan2(y) };
    let lat = if north { lat_n } else { -lat_n };
    (lat.to_degrees(), lon.to_degrees())
}
