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

/// A reference ellipsoid defined by its semi-major axis and inverse flattening.
///
/// Valid by construction: look up the ellipsoid of a reference system with
/// [`Ellipsoid::for_crs`], or take one of the named constants it returns
/// ([`Ellipsoid::WGS84`], …). No public function takes an `Ellipsoid`, so
/// there is no public constructor from raw parameters. The semi-major axis is
/// finite and positive, and the inverse flattening is greater than one, or
/// positive infinity for a sphere. Under the `serde` feature, deserialization
/// checks both and rejects a record that fails either; a sphere's infinite
/// inverse flattening is written as `null` (JSON has no infinity) and `null`
/// reads back as infinity.
///
/// ```
/// use geocoordinates::{Crs, geodesy::Ellipsoid};
///
/// let clarke = Ellipsoid::for_crs(Crs::Nad27).unwrap();
/// assert_eq!(clarke, Ellipsoid::CLARKE_1866);
/// # #[cfg(feature = "serde")]
/// # {
/// // A hand-built record with a negative semi-major axis does not deserialize.
/// let bad = r#"{"semi_major_m":-1.0,"inverse_flattening":298.0}"#;
/// assert!(serde_json::from_str::<Ellipsoid>(bad).is_err());
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "EllipsoidRecord"))]
pub struct Ellipsoid {
    #[cfg_attr(feature = "serde", serde(rename = "semi_major_m"))]
    semi_major: Length,
    #[cfg_attr(
        feature = "serde",
        serde(serialize_with = "crate::serde_util::infinity_as_none::serialize")
    )]
    inverse_flattening: f64,
}

/// Unvalidated serde shape of [`Ellipsoid`]; deserialization goes through
/// [`Ellipsoid::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct EllipsoidRecord {
    #[serde(rename = "semi_major_m")]
    semi_major: Length,
    #[serde(with = "crate::serde_util::infinity_as_none")]
    inverse_flattening: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<EllipsoidRecord> for Ellipsoid {
    type Error = Error;

    fn try_from(r: EllipsoidRecord) -> Result<Self> {
        Ellipsoid::new(r.semi_major, r.inverse_flattening)
    }
}

/// Build a named constant from published parameters, which satisfy the
/// type's invariant (checked by `Ellipsoid::new`) by inspection.
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

    /// The Krasovsky-1940 ellipsoid (Pulkovo-1942, SK-42, and derived datums).
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::Ellipsoid};
    ///
    /// assert_eq!(Ellipsoid::for_crs(Crs::Pulkovo42), Some(Ellipsoid::KRASOVSKY_1940));
    /// assert_eq!(Ellipsoid::KRASOVSKY_1940.inverse_flattening(), 298.3);
    /// ```
    pub const KRASOVSKY_1940: Ellipsoid = named(6_378_245.0, 298.3);

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
    /// (field `"semi_major"`), or when `inverse_flattening` is
    /// NaN or not greater than one (field `"inverse_flattening"`).
    ///
    /// Crate-private: no public function takes an `Ellipsoid`, so callers get
    /// one from [`Ellipsoid::for_crs`] or a named constant. It is the
    /// validation serde deserialization goes through.
    #[cfg(any(feature = "serde", test))]
    pub(crate) fn new(semi_major: Length, inverse_flattening: f64) -> Result<Self> {
        let a = semi_major.meters();
        if !a.is_finite() || a <= 0.0 {
            return Err(Error::InvalidValue {
                field: "semi_major",
                detail: "must be finite and positive".into(),
            });
        }
        // −∞ is caught by `<= 1.0`; +∞ (a sphere) passes.
        if inverse_flattening.is_nan() || inverse_flattening <= 1.0 {
            return Err(Error::InvalidValue {
                field: "inverse_flattening",
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
    /// assert_eq!(Ellipsoid::WGS84.semi_major(), Length::from_meters(6_378_137.0));
    /// ```
    #[must_use]
    pub const fn semi_major(self) -> Length {
        self.semi_major
    }

    /// Inverse flattening `1/f` (positive infinity for a sphere).
    ///
    /// ```
    /// use geocoordinates::geodesy::Ellipsoid;
    ///
    /// assert_eq!(Ellipsoid::WGS84.inverse_flattening(), 298.257_223_563);
    /// assert_eq!(Ellipsoid::KRASOVSKY_1940.inverse_flattening(), 298.3);
    /// ```
    #[must_use]
    pub const fn inverse_flattening(self) -> f64 {
        self.inverse_flattening
    }

    /// Flattening `f`.
    ///
    /// ```
    /// use geocoordinates::geodesy::Ellipsoid;
    ///
    /// let f = Ellipsoid::KRASOVSKY_1940.flattening();
    /// assert!((f - 1.0 / 298.3).abs() < 1e-18);
    /// ```
    #[must_use]
    pub const fn flattening(self) -> f64 {
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
    pub const fn semi_minor(self) -> Length {
        Length::from_meters(self.semi_major.meters() * (1.0 - self.flattening()))
    }

    /// First eccentricity squared `e² = f(2 − f)`.
    ///
    /// ```
    /// use geocoordinates::geodesy::Ellipsoid;
    ///
    /// assert!((Ellipsoid::WGS84.eccentricity_sq() - 0.006_694_379_990_141).abs() < 1e-15);
    /// ```
    #[must_use]
    pub const fn eccentricity_sq(self) -> f64 {
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
                    field: "semi_major",
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
                        field: "inverse_flattening",
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
        assert_eq!(
            serde_json::to_value(Ellipsoid::BESSEL_1841).unwrap(),
            serde_json::json!({"semi_major_m": 6_377_397.155, "inverse_flattening": 299.152_812_8})
        );
        let json = serde_json::to_string(&Ellipsoid::BESSEL_1841).unwrap();
        assert_eq!(
            json,
            r#"{"semi_major_m":6377397.155,"inverse_flattening":299.1528128}"#
        );
        let back: Ellipsoid = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Ellipsoid::BESSEL_1841);
        let bad = r#"{"semi_major_m":-1.0,"inverse_flattening":298.0}"#;
        assert!(serde_json::from_str::<Ellipsoid>(bad).is_err());
        let bad = r#"{"semi_major_m":6378137.0,"inverse_flattening":0.5}"#;
        assert!(serde_json::from_str::<Ellipsoid>(bad).is_err());
        // The pre-0.15 wrapped shape no longer reads.
        let old = r#"{"semi_major":{"meters":6377397.155},"inverse_flattening":299.1528128}"#;
        assert!(serde_json::from_str::<Ellipsoid>(old).is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_a_sphere() {
        let sphere = Ellipsoid::new(Length::from_meters(6_371_000.0), f64::INFINITY).unwrap();
        let json = serde_json::to_string(&sphere).unwrap();
        // JSON has no infinity: the sphere's inverse flattening is `null`.
        assert_eq!(
            json,
            r#"{"semi_major_m":6371000.0,"inverse_flattening":null}"#
        );
        let back: Ellipsoid = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sphere);
        assert_eq!(back.inverse_flattening(), f64::INFINITY);
        assert_eq!(back.flattening(), 0.0);
        // The field stays required; only an explicit `null` means a sphere.
        let missing = r#"{"semi_major_m":6371000.0}"#;
        assert!(serde_json::from_str::<Ellipsoid>(missing).is_err());
    }
}
