//! Earth-Centered, Earth-Fixed (ECEF) geocentric coordinates.
//!
//! ECEF is the bridge format for almost every datum transformation. Both
//! directions are exact closed forms, accurate to floating-point round-off:
//! geodetic → ECEF is the textbook prime-vertical formula, and ECEF → geodetic
//! is Vermeille's closed-form solution as refined in Karney's GeographicLib
//! (`Geocentric::Reverse`), which holds at every altitude, at the poles, and
//! deep inside the Earth.
//!
//! The ellipsoid is never a separate argument: it is the ellipsoid of the
//! coordinate's [`Crs`] ([`Ellipsoid::for_crs`]), so a NAD27 coordinate is
//! always placed on Clarke 1866 and a WGS-84 one on WGS 84. GCJ-02 and BD-09
//! are obfuscated renderings rather than datums and have no ECEF position;
//! convert them to WGS-84 first.
//!
//! ```
//! use geocoordinates::{Coordinate, Crs, geodesy::Ecef};
//!
//! // The same latitude and longitude sit at different places on two datums.
//! let wgs = Ecef::try_from_coordinate(Coordinate::new(35.0, 139.0, Crs::Wgs84)?)?;
//! let tokyo = Ecef::try_from_coordinate(Coordinate::new(35.0, 139.0, Crs::Tokyo)?)?;
//! assert!((wgs.x_m - tokyo.x_m).abs() > 100.0);
//! # Ok::<(), geocoordinates::Error>(())
//! ```

use super::ellipsoid::Ellipsoid;
use crate::coord::{Coordinate, Crs, Height};
use crate::error::{Error, Result};

fn height_meters(coord: Coordinate) -> Result<f64> {
    match coord.height() {
        Some(Height::Ellipsoidal(h)) => Ok(h),
        Some(Height::Orthometric(_)) => Err(Error::InvalidValue {
            field: "height",
            detail: "ECEF conversion requires ellipsoidal height".into(),
        }),
        None => Ok(0.0),
    }
}

/// The ellipsoid of a coordinate's own `crs`, or [`Error::CrsMismatch`]
/// (expecting WGS-84) for the China obfuscation systems, which have none.
pub(crate) fn ellipsoid_of(crs: Crs) -> Result<Ellipsoid> {
    Ellipsoid::for_crs(crs).ok_or(Error::CrsMismatch {
        expected: Crs::Wgs84,
        found: crs,
    })
}

/// The ellipsoid of a `crs` the caller passed as an argument, or
/// [`Error::InvalidValue`] naming that argument (`field`) for the China
/// obfuscation systems, which have none. No coordinate is on the wrong datum
/// here, so the error is not a [`Error::CrsMismatch`].
pub(crate) fn ellipsoid_of_argument(crs: Crs, field: &'static str) -> Result<Ellipsoid> {
    Ellipsoid::for_crs(crs).ok_or_else(|| Error::InvalidValue {
        field,
        detail: format!("{crs} is not a geodetic datum and has no ellipsoid"),
    })
}

/// A geocentric ECEF position in meters.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, geodesy::Ecef};
///
/// let ecef = Ecef::try_from_coordinate(Coordinate::wgs84(0.0, 0.0)?)?;
/// assert_eq!((ecef.x_m, ecef.y_m, ecef.z_m), (6_378_137.0, 0.0, 0.0));
/// let back = ecef.try_to_coordinate(Crs::Wgs84)?;
/// assert_eq!((back.lat(), back.lon()), (0.0, 0.0));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ecef {
    /// X axis (meters), through the prime meridian at the equator.
    pub x_m: f64,
    /// Y axis (meters), 90° east at the equator.
    pub y_m: f64,
    /// Z axis (meters), through the north pole.
    pub z_m: f64,
}

impl Ecef {
    /// Construct from X/Y/Z in meters.
    ///
    /// ```
    /// use geocoordinates::geodesy::Ecef;
    ///
    /// let p = Ecef::new(1.0, 2.0, 3.0);
    /// assert_eq!((p.x_m, p.y_m, p.z_m), (1.0, 2.0, 3.0));
    /// ```
    #[must_use]
    pub fn new(x_m: f64, y_m: f64, z_m: f64) -> Self {
        Self { x_m, y_m, z_m }
    }

    /// ECEF → geodetic [`Coordinate`] (lat/lon/ellipsoidal height) on the
    /// ellipsoid of `crs`, tagged with `crs`.
    ///
    /// **Exact** to floating-point round-off at any altitude (Vermeille's
    /// closed form, as refined in GeographicLib's `Geocentric::Reverse`). ECEF
    /// itself is datum-agnostic, so `crs` names the datum the position is
    /// expressed in, and with it the ellipsoid.
    ///
    /// # Errors
    /// - [`Error::InvalidValue`] (field `"crs"`) when `crs` is GCJ-02 or
    ///   BD-09, which are not geodetic datums and have no ellipsoid;
    /// - [`Error::InvalidValue`] (field `"ECEF"`) when a component is not
    ///   finite, and (field `"height"`) when the position is so far from the
    ///   Earth that its height overflows.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, Height, geodesy::Ecef};
    ///
    /// // 1000 m above the WGS-84 north pole.
    /// let pole = Ecef::new(0.0, 0.0, 6_356_752.314_245 + 1000.0).try_to_coordinate(Crs::Wgs84)?;
    /// assert_eq!(pole.lat(), 90.0);
    /// assert!(matches!(pole.height(), Some(Height::Ellipsoidal(h)) if (h - 1000.0).abs() < 1e-6));
    /// // GCJ-02 is no datum, so an ECEF position cannot be expressed in it.
    /// assert!(matches!(
    ///     Ecef::new(6_378_137.0, 0.0, 0.0).try_to_coordinate(Crs::Gcj02),
    ///     Err(Error::InvalidValue { field: "crs", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_coordinate(self, crs: Crs) -> Result<Coordinate> {
        let ellipsoid = ellipsoid_of_argument(crs, "crs")?;
        if !self.x_m.is_finite() || !self.y_m.is_finite() || !self.z_m.is_finite() {
            return Err(Error::InvalidValue {
                field: "ECEF",
                detail: "x, y, and z must be finite".into(),
            });
        }
        let (lat, h) = geodetic_latitude_and_height(self, ellipsoid);
        let lon = self.y_m.atan2(self.x_m).to_degrees();
        Coordinate::new(lat.to_degrees(), lon, crs)?.with_height(Height::Ellipsoidal(h))
    }

    /// Geodetic [`Coordinate`] → ECEF on the ellipsoid of its own [`Crs`]
    /// (exact, closed form). Missing height means zero ellipsoidal height.
    ///
    /// # Errors
    /// - [`Error::CrsMismatch`] (expecting [`Crs::Wgs84`]) when `coord` is
    ///   GCJ-02 or BD-09, which are not geodetic datums and have no ellipsoid;
    /// - [`Error::InvalidValue`] (field `"height"`) when `coord` carries an
    ///   orthometric height: no geoid model is available to turn it into an
    ///   ellipsoidal one.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, Height, geodesy::Ecef};
    ///
    /// let raised = Coordinate::wgs84(0.0, 90.0)?.with_height(Height::Ellipsoidal(100.0))?;
    /// let p = Ecef::try_from_coordinate(raised)?;
    /// assert!(p.x_m.abs() < 1e-6 && (p.y_m - 6_378_237.0).abs() < 1e-6 && p.z_m == 0.0);
    /// // An orthometric height needs a geoid model this crate does not ship.
    /// let msl = Coordinate::wgs84(0.0, 90.0)?.with_height(Height::Orthometric(100.0))?;
    /// assert!(matches!(
    ///     Ecef::try_from_coordinate(msl),
    ///     Err(Error::InvalidValue { field: "height", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_from_coordinate(coord: Coordinate) -> Result<Self> {
        let ellipsoid = ellipsoid_of(coord.crs())?;
        let h = height_meters(coord)?;
        let phi = coord.lat().to_radians();
        let lambda = coord.lon().to_radians();
        let a = ellipsoid.semi_major().meters();
        let e2 = ellipsoid.eccentricity_sq();
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        let n = a / (1.0 - e2 * sin_phi * sin_phi).sqrt();
        Ok(Ecef {
            x_m: (n + h) * cos_phi * lambda.cos(),
            y_m: (n + h) * cos_phi * lambda.sin(),
            z_m: (n * (1.0 - e2) + h) * sin_phi,
        })
    }
}

/// Geodetic latitude (radians) and ellipsoidal height (meters) of a finite
/// ECEF position on an **oblate** ellipsoid (`f > 0`; every ellipsoid
/// [`Ellipsoid::for_crs`] returns is one).
///
/// A port of the oblate branch of `Geocentric::IntReverse` from Karney's
/// GeographicLib (MIT licence), itself Vermeille's 2002 closed-form solution
/// with Karney's guards against cancellation and underflow.
fn geodetic_latitude_and_height(p: Ecef, ellipsoid: Ellipsoid) -> (f64, f64) {
    let r = p.x_m.hypot(p.y_m);
    let (lat, h) = vermeille(r, p.z_m, ellipsoid);
    if h.is_finite() {
        (lat, h)
    } else {
        // The closed form's eighth powers overflow somewhere past 1e40 m.
        // That far out the Earth is a point: the distance to its center is
        // the height to round-off, and the direction is the latitude.
        (p.z_m.atan2(r), r.hypot(p.z_m))
    }
}

/// Vermeille's closed form for the distance `r` from the polar axis and the
/// height `z` above the equatorial plane. Non-finite once its intermediate
/// powers overflow.
fn vermeille(r: f64, z: f64, ellipsoid: Ellipsoid) -> (f64, f64) {
    let a = ellipsoid.semi_major().meters();
    let e2 = ellipsoid.eccentricity_sq();
    let e2m = 1.0 - e2; // (1 − f)²
    let e4 = e2 * e2;
    let pp = (r / a) * (r / a);
    let q = e2m * (z / a) * (z / a);
    let rr = (pp + q - e4) / 6.0;
    if e4 * q == 0.0 && rr <= 0.0 {
        // On the equatorial plane inside the evolute (r ≤ a·e²): the general
        // formulas give 0/0, so take their limit. The foot point is off the
        // equator, on the side of z's sign.
        let zz = ((e4 - pp) / e2m).sqrt();
        let xx = pp.sqrt();
        let hh = zz.hypot(xx);
        let lat = zz.atan2(xx);
        let h = -a * e2m * hh / e2;
        return (if z < 0.0 { -lat } else { lat }, h);
    }
    // Multiplying the equations for s and t by r³ and r avoids dividing by
    // r = 0.
    let s = e4 * pp * q / 4.0;
    let r2 = rr * rr;
    let r3 = rr * r2;
    let disc = s * (2.0 * r3 + s);
    let u = if disc >= 0.0 {
        // Take the root's sign that maximizes |t³|, avoiding cancellation.
        let t3 = s + r3;
        let t3 = t3 + disc.sqrt().copysign(t3);
        let t = t3.cbrt();
        rr + t + if t == 0.0 { 0.0 } else { r2 / t }
    } else {
        // Three real roots: take the one that avoids cancellation.
        let ang = (-disc).sqrt().atan2(-(s + r3));
        rr + 2.0 * rr * (ang / 3.0).cos()
    };
    let v = (u * u + e4 * q).sqrt();
    // u + v, rearranged when u is negative to avoid cancellation. (u is never
    // ±0 with q = 0 here, so v − u cannot vanish.)
    let uv = if u.is_sign_negative() {
        e4 * q / (v - u)
    } else {
        u + v
    };
    let w = (e2 * (uv - q) / (2.0 * v)).max(0.0);
    let k = uv / ((uv + w * w).sqrt() + w);
    let k2 = k + e2;
    let lat = (z / k).atan2(r / k2);
    let h = (1.0 - e2m / k) * (k * r / k2).hypot(z);
    (lat, h)
}

#[cfg(test)]
mod tests;
