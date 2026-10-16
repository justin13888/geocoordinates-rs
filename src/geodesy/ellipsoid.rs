//! Reference ellipsoid definitions.
//!
//! ```
//! use geocoordinates::{Crs, geodesy::Ellipsoid};
//!
//! let bessel = Ellipsoid::for_crs(Crs::Tokyo).unwrap();
//! assert!(bessel.semi_major() < Ellipsoid::WGS84.semi_major());
//! ```

use crate::coord::Crs;
use crate::error::{Error, Result};
use crate::units::Length;

/// A reference ellipsoid defined by its semi-major axis and inverse flattening.
///
/// Valid by construction: build one with [`Ellipsoid::new`], take a named
/// constant ([`Ellipsoid::WGS84`], …), or look up the ellipsoid of a reference
/// system with [`Ellipsoid::for_crs`]. The semi-major axis is finite and
/// positive, and the inverse flattening is greater than one, or positive
/// infinity for a sphere. Deserialization goes through [`Ellipsoid::new`].
///
/// ```
/// use geocoordinates::{Crs, Length, geodesy::Ellipsoid};
///
/// let clarke = Ellipsoid::for_crs(Crs::Nad27).unwrap();
/// assert_eq!(clarke, Ellipsoid::CLARKE_1866);
/// assert!(Ellipsoid::new(Length::from_meters(-1.0), 298.0).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "EllipsoidRecord"))]
pub struct Ellipsoid {
    semi_major: Length,
    inverse_flattening: f64,
}

/// Unvalidated serde shape of [`Ellipsoid`]; deserialization goes through
/// [`Ellipsoid::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct EllipsoidRecord {
    semi_major: Length,
    inverse_flattening: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<EllipsoidRecord> for Ellipsoid {
    type Error = Error;

    fn try_from(r: EllipsoidRecord) -> Result<Self> {
        Ellipsoid::new(r.semi_major, r.inverse_flattening)
    }
}

/// Build a named constant from published parameters, which satisfy
/// [`Ellipsoid::new`]'s checks by inspection.
const fn named(semi_major_m: f64, inverse_flattening: f64) -> Ellipsoid {
    Ellipsoid {
        semi_major: Length::from_meters(semi_major_m),
        inverse_flattening,
    }
}

impl Ellipsoid {
    /// The WGS-84 ellipsoid.
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::Ellipsoid};
    ///
    /// assert_eq!(Ellipsoid::WGS84.semi_major().meters(), 6_378_137.0);
    /// assert_eq!(Ellipsoid::for_crs(Crs::Wgs84), Some(Ellipsoid::WGS84));
    /// ```
    pub const WGS84: Ellipsoid = named(6_378_137.0, 298.257_223_563);

    /// The GRS80 ellipsoid (NAD83/ETRS89).
    ///
    /// ```
    /// use geocoordinates::geodesy::Ellipsoid;
    ///
    /// // Same equatorial radius as WGS 84; the semi-minor axes differ by ~0.1 mm.
    /// let (grs, wgs) = (Ellipsoid::GRS80, Ellipsoid::WGS84);
    /// assert_eq!(grs.semi_major(), wgs.semi_major());
    /// assert!((grs.semi_minor().meters() - wgs.semi_minor().meters()).abs() < 1e-3);
    /// ```
    pub const GRS80: Ellipsoid = named(6_378_137.0, 298.257_222_101);

    /// The Krasovsky-1940 ellipsoid (Pulkovo-1942, SK-42, and derived datums).
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::Ellipsoid};
    ///
    /// assert_eq!(Ellipsoid::for_crs(Crs::Pulkovo42), Some(Ellipsoid::KRASOVSKY_1940));
    /// assert_eq!(Ellipsoid::KRASOVSKY_1940.inverse_flattening(), 298.3);
    /// ```
    pub const KRASOVSKY_1940: Ellipsoid = named(6_378_245.0, 298.3);

    /// The Airy-1830 ellipsoid (OSGB36 / Ordnance Survey of Great Britain).
    ///
    /// No built-in [`Crs`] uses it, so [`Ellipsoid::for_crs`] never returns it
    /// and no transform in this crate is parameterized by it; it is provided
    /// as a reference value only.
    ///
    /// ```
    /// use geocoordinates::geodesy::Ellipsoid;
    ///
    /// assert_eq!(Ellipsoid::AIRY_1830.semi_major().meters(), 6_377_563.396);
    /// assert!(Ellipsoid::AIRY_1830.semi_major() < Ellipsoid::WGS84.semi_major());
    /// ```
    pub const AIRY_1830: Ellipsoid = named(6_377_563.396, 299.324_964_6);

    /// The Bessel-1841 ellipsoid (Tokyo datum, older European datums).
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::Ellipsoid};
    ///
    /// assert_eq!(Ellipsoid::for_crs(Crs::Tokyo), Some(Ellipsoid::BESSEL_1841));
    /// assert_eq!(Ellipsoid::BESSEL_1841.semi_major().meters(), 6_377_397.155);
    /// ```
    pub const BESSEL_1841: Ellipsoid = named(6_377_397.155, 299.152_812_8);

    /// The Clarke-1866 ellipsoid (NAD27 / North American Datum 1927).
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::Ellipsoid};
    ///
    /// assert_eq!(Ellipsoid::for_crs(Crs::Nad27), Some(Ellipsoid::CLARKE_1866));
    /// assert_eq!(Ellipsoid::CLARKE_1866.semi_major().meters(), 6_378_206.4);
    /// ```
    pub const CLARKE_1866: Ellipsoid = named(6_378_206.4, 294.978_698_2);

    /// Build an ellipsoid from its semi-major axis and inverse flattening
    /// `1/f`. Pass `f64::INFINITY` as the inverse flattening for a sphere.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] when `semi_major` is not finite and positive
    /// (field `"ellipsoid semi-major axis"`), or when `inverse_flattening` is
    /// NaN or not greater than one (field `"ellipsoid inverse flattening"`).
    ///
    /// ```
    /// use geocoordinates::{Length, LengthUnit, geodesy::Ellipsoid};
    ///
    /// let sphere = Ellipsoid::new(Length::from_unit(6_371.0, LengthUnit::Kilometer), f64::INFINITY)?;
    /// assert_eq!(sphere.flattening(), 0.0);
    /// assert!(Ellipsoid::new(Length::from_meters(6_371_000.0), 0.5).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(semi_major: Length, inverse_flattening: f64) -> Result<Self> {
        let a = semi_major.meters();
        if !a.is_finite() || a <= 0.0 {
            return Err(Error::InvalidValue {
                field: "ellipsoid semi-major axis",
                detail: "must be finite and positive".into(),
            });
        }
        // −∞ is caught by `<= 1.0`; +∞ (a sphere) passes.
        if inverse_flattening.is_nan() || inverse_flattening <= 1.0 {
            return Err(Error::InvalidValue {
                field: "ellipsoid inverse flattening",
                detail: "must be greater than one (or positive infinity for a sphere)".into(),
            });
        }
        Ok(Ellipsoid {
            semi_major,
            inverse_flattening,
        })
    }

    /// The ellipsoid a reference system's coordinates are defined on, or
    /// `None` for GCJ-02 and BD-09.
    ///
    /// GCJ-02 and BD-09 are obfuscated renderings of WGS-84, not geodetic
    /// datums: their latitudes and longitudes are not positions on any
    /// ellipsoid. Convert them to WGS-84 first (see [`china`](crate::china)).
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::Ellipsoid};
    ///
    /// assert_eq!(Ellipsoid::for_crs(Crs::Tokyo), Some(Ellipsoid::BESSEL_1841));
    /// assert_eq!(Ellipsoid::for_crs(Crs::Gcj02), None);
    /// ```
    #[must_use]
    pub const fn for_crs(crs: Crs) -> Option<Ellipsoid> {
        // Exhaustive: a new `Crs` variant must decide its ellipsoid here.
        match crs {
            Crs::Wgs84 => Some(Ellipsoid::WGS84),
            Crs::Nad27 => Some(Ellipsoid::CLARKE_1866),
            Crs::Tokyo => Some(Ellipsoid::BESSEL_1841),
            Crs::Pulkovo42 => Some(Ellipsoid::KRASOVSKY_1940),
            Crs::Gcj02 | Crs::Bd09 => None,
        }
    }

    /// Semi-major axis `a` (the equatorial radius).
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Ellipsoid};
    ///
    /// let a = Length::from_meters(6_371_000.0);
    /// assert_eq!(Ellipsoid::new(a, f64::INFINITY)?.semi_major(), a);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn semi_major(&self) -> Length {
        self.semi_major
    }

    /// Inverse flattening `1/f` (positive infinity for a sphere).
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Ellipsoid};
    ///
    /// assert_eq!(Ellipsoid::WGS84.inverse_flattening(), 298.257_223_563);
    /// let sphere = Ellipsoid::new(Length::from_meters(6_371_000.0), f64::INFINITY)?;
    /// assert_eq!(sphere.inverse_flattening(), f64::INFINITY);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn inverse_flattening(&self) -> f64 {
        self.inverse_flattening
    }

    /// Flattening `f`.
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Ellipsoid};
    ///
    /// let f = Ellipsoid::new(Length::from_meters(6_378_137.0), 300.0)?.flattening();
    /// assert!((f - 1.0 / 300.0).abs() < 1e-18);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn flattening(&self) -> f64 {
        1.0 / self.inverse_flattening
    }

    /// Semi-minor axis `b` (the polar radius).
    ///
    /// ```
    /// use geocoordinates::geodesy::Ellipsoid;
    ///
    /// // WGS 84's polar radius is about 21.4 km shorter than its equatorial one.
    /// assert!((Ellipsoid::WGS84.semi_minor().meters() - 6_356_752.314_245).abs() < 1e-3);
    /// ```
    #[must_use]
    pub const fn semi_minor(&self) -> Length {
        Length::from_meters(self.semi_major.meters() * (1.0 - self.flattening()))
    }

    /// First eccentricity squared `e² = f(2 − f)`.
    ///
    /// ```
    /// use geocoordinates::{Length, geodesy::Ellipsoid};
    ///
    /// assert!((Ellipsoid::WGS84.eccentricity_sq() - 0.006_694_379_990_141).abs() < 1e-15);
    /// let sphere = Ellipsoid::new(Length::from_meters(6_371_000.0), f64::INFINITY)?;
    /// assert_eq!(sphere.eccentricity_sq(), 0.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn eccentricity_sq(&self) -> f64 {
        let f = self.flattening();
        f * (2.0 - f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    fn new(a: f64, inv_f: f64) -> Result<Ellipsoid> {
        Ellipsoid::new(Length::from_meters(a), inv_f)
    }

    #[test]
    fn wgs84_derived_quantities() {
        let e = Ellipsoid::WGS84;
        assert_close(e.semi_major().meters(), 6_378_137.0, 0.0);
        assert_close(e.inverse_flattening(), 298.257_223_563, 0.0);
        assert_close(e.flattening(), 1.0 / 298.257_223_563, 1e-18);
        assert_close(e.semi_minor().meters(), 6_356_752.314_245_18, 1e-6);
        assert_close(e.eccentricity_sq(), 0.006_694_379_990_141_32, 1e-15);
    }

    #[test]
    fn sphere_has_no_flattening() {
        // A sphere (inverse_flattening = ∞) has f = 0, b = a, e² = 0.
        let sphere = new(1000.0, f64::INFINITY).unwrap();
        assert_close(sphere.flattening(), 0.0, 0.0);
        assert_close(sphere.semi_minor().meters(), 1000.0, 1e-12);
        assert_close(sphere.eccentricity_sq(), 0.0, 0.0);
    }

    #[test]
    fn new_accepts_valid_parameters_and_keeps_them() {
        let e = new(6_378_206.4, 294.978_698_2).unwrap();
        assert_eq!(e, Ellipsoid::CLARKE_1866);
        // The boundary just above one is accepted.
        assert!(new(1.0, 1.0 + f64::EPSILON).is_ok());
    }

    #[test]
    fn new_rejects_a_bad_semi_major_axis() {
        for a in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                new(a, 298.0),
                Err(Error::InvalidValue {
                    field: "ellipsoid semi-major axis",
                    detail: "must be finite and positive".into(),
                }),
                "a = {a}"
            );
        }
    }

    #[test]
    fn new_rejects_a_bad_inverse_flattening() {
        for inv_f in [1.0, 0.5, 0.0, -298.0, f64::NAN, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    new(6_378_137.0, inv_f),
                    Err(Error::InvalidValue {
                        field: "ellipsoid inverse flattening",
                        ..
                    })
                ),
                "1/f = {inv_f}"
            );
        }
    }

    #[test]
    fn for_crs_maps_each_datum_to_its_ellipsoid() {
        assert_eq!(Ellipsoid::for_crs(Crs::Wgs84), Some(Ellipsoid::WGS84));
        assert_eq!(Ellipsoid::for_crs(Crs::Nad27), Some(Ellipsoid::CLARKE_1866));
        assert_eq!(Ellipsoid::for_crs(Crs::Tokyo), Some(Ellipsoid::BESSEL_1841));
        assert_eq!(
            Ellipsoid::for_crs(Crs::Pulkovo42),
            Some(Ellipsoid::KRASOVSKY_1940)
        );
        assert_eq!(Ellipsoid::for_crs(Crs::Gcj02), None);
        assert_eq!(Ellipsoid::for_crs(Crs::Bd09), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_and_validates() {
        let json = serde_json::to_string(&Ellipsoid::AIRY_1830).unwrap();
        assert_eq!(
            json,
            r#"{"semi_major":{"meters":6377563.396},"inverse_flattening":299.3249646}"#
        );
        let back: Ellipsoid = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Ellipsoid::AIRY_1830);
        let bad = r#"{"semi_major":{"meters":-1.0},"inverse_flattening":298.0}"#;
        assert!(serde_json::from_str::<Ellipsoid>(bad).is_err());
        let bad = r#"{"semi_major":{"meters":6378137.0},"inverse_flattening":0.5}"#;
        assert!(serde_json::from_str::<Ellipsoid>(bad).is_err());
    }
}
