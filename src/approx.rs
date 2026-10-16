//! The [`Approx`] wrapper for approximate (lossy or iterative) results.

use crate::{Error, Length, Result};

/// Wraps the result of an **approximate** conversion together with an estimated
/// error bound.
///
/// Any conversion that inverts a lossy or secret transform (e.g. GCJ-02 →
/// WGS-84), iterates to a tolerance, or decodes a cell with spatial extent
/// (e.g. geohash) returns `Approx<T>` rather than a bare `T`. This makes the
/// approximation impossible to ignore at the call site, and exposes the bound
/// via [`Approx::max_error`].
///
/// `Approx<T>` deliberately does **not** dereference to `T`: reading the
/// wrapped value takes an explicit [`value`](Self::value) or
/// [`into_inner`](Self::into_inner), so an approximate latitude never reads as
/// an exact one.
///
/// The bound is valid by construction: [`Approx::new`] (and deserialization)
/// reject a negative or NaN bound, and accept `+∞` as "no bound is known".
///
/// ```
/// use geocoordinates::china::{Gcj02, Wgs84};
/// use geocoordinates::{Approx, Length};
///
/// let wgs: Approx<Wgs84> = Gcj02::new(39.916_404, 116.410_244)?.to_wgs84_refined();
/// // The wrapped value is read explicitly...
/// assert!((wgs.value().lat() - 39.915).abs() < 1e-5);
/// // ...and the bound stays attached until it is explicitly discarded.
/// assert_eq!(wgs.max_error(), Length::from_meters(0.5));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
///
/// ```compile_fail
/// let wgs = geocoordinates::china::Gcj02::new(39.9, 116.4).unwrap().to_wgs84_refined();
/// let _ = wgs.lat(); // no `Deref`: an approximate value is not read as exact
/// ```
///
/// Note: the derived [`PartialEq`] compares [`max_error`](Approx::max_error)
/// too, so two approximations of the same point with different bounds are not
/// equal. Compare [`value`](Approx::value) explicitly for a bound-agnostic test.
///
/// Under the `serde` feature the shape is `{"value":…,"max_error_m":…}`, the
/// bound in meters as an optional number: a finite bound is written as itself,
/// and an unbounded one (`+∞`, e.g. a Pulkovo-1942 datum shift) as `null` /
/// none, which deserializes back to `+∞`. The bound therefore survives formats
/// such as JSON that have no representation for infinity. Deserialization
/// applies the same checks as [`new`](Self::new).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        try_from = "ApproxRepr<T>",
        bound(deserialize = "T: serde::Deserialize<'de>")
    )
)]
pub struct Approx<T> {
    value: T,
    max_error: Length,
}

/// The unchecked wire shape of an [`Approx`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct ApproxRepr<T> {
    value: T,
    #[serde(with = "max_error_serde")]
    max_error_m: f64,
}

/// The borrowed wire shape an [`Approx`] serializes through.
#[cfg(feature = "serde")]
#[derive(serde::Serialize)]
struct ApproxRef<'a, T> {
    value: &'a T,
    #[serde(with = "max_error_serde")]
    max_error_m: f64,
}

#[cfg(feature = "serde")]
impl<T> TryFrom<ApproxRepr<T>> for Approx<T> {
    type Error = Error;

    fn try_from(repr: ApproxRepr<T>) -> Result<Self> {
        Self::new(repr.value, Length::from_meters(repr.max_error_m))
    }
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for Approx<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> core::result::Result<S::Ok, S::Error> {
        ApproxRef {
            value: &self.value,
            max_error_m: self.max_error.meters(),
        }
        .serialize(s)
    }
}

/// (De)serializes an error bound with `f64::INFINITY` as none, because JSON
/// writes infinity as `null` and cannot read `null` back as a number.
#[cfg(feature = "serde")]
mod max_error_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(bound: &f64, s: S) -> Result<S::Ok, S::Error> {
        if *bound == f64::INFINITY {
            s.serialize_none()
        } else {
            s.serialize_some(bound)
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::INFINITY))
    }
}

impl<T> Approx<T> {
    /// Wrap `value` with its estimated maximum positional error.
    ///
    /// For crates that compute their own approximations (e.g. an EXIF reader
    /// that rounds a rational GPS tag) and want to hand callers the same
    /// honest wrapper the crate's own conversions return. Pass
    /// `Length::from_meters(f64::INFINITY)` when no bound is known.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] (field `max_error`) when the bound is
    /// NaN or negative.
    ///
    /// ```
    /// use geocoordinates::{Approx, Coordinate, Error, Length};
    ///
    /// let c = Coordinate::wgs84(51.5, -0.12)?;
    /// let a = Approx::new(c, Length::from_meters(3.0))?;
    /// assert_eq!(a.max_error().meters(), 3.0);
    /// // "Unknown" is spelled +∞, never a made-up number.
    /// assert!(Approx::new(c, Length::from_meters(f64::INFINITY)).is_ok());
    /// assert!(matches!(
    ///     Approx::new(c, Length::from_meters(-1.0)),
    ///     Err(Error::InvalidValue { field: "max_error", .. })
    /// ));
    /// assert!(Approx::new(c, Length::from_meters(f64::NAN)).is_err());
    /// # Ok::<(), Error>(())
    /// ```
    pub fn new(value: T, max_error: Length) -> Result<Self> {
        let m = max_error.meters();
        if m.is_nan() || m < 0.0 {
            return Err(Error::InvalidValue {
                field: "max_error",
                detail: format!("must be a non-negative length or +infinity, got {m} m"),
            });
        }
        Ok(Self { value, max_error })
    }

    /// Construct from a bound the crate computed itself, in meters.
    ///
    /// Infallible: internal bounds are non-negative by construction, which the
    /// debug assertion checks.
    #[must_use]
    pub(crate) fn from_valid(value: T, max_error_m: f64) -> Self {
        debug_assert!(
            max_error_m >= 0.0,
            "Approx::from_valid invalid bound: {max_error_m}"
        );
        Self {
            value,
            max_error: Length::from_meters(max_error_m),
        }
    }

    /// Borrow the underlying value.
    ///
    /// Compare values with this rather than `==` on the wrapper, which also
    /// compares the bounds:
    ///
    /// ```
    /// use geocoordinates::china::Gcj02;
    ///
    /// let gcj = Gcj02::new(39.916_404, 116.410_244)?;
    /// let (fast, refined) = (gcj.to_wgs84_fast(), gcj.to_wgs84_refined());
    /// assert_ne!(fast, refined);
    /// assert!((fast.value().lat() - refined.value().lat()).abs() < 1e-4);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Consume the wrapper, discarding the error estimate.
    ///
    /// ```
    /// use geocoordinates::china::{Gcj02, Wgs84};
    ///
    /// let wgs: Wgs84 = Gcj02::new(39.916_404, 116.410_244)?.to_wgs84_refined().into_inner();
    /// assert!((wgs.lon() - 116.404).abs() < 1e-5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }

    /// The estimated upper bound on positional error.
    ///
    /// Never negative or NaN; `+∞` means no bound is known.
    ///
    /// ```
    /// use geocoordinates::china::Gcj02;
    /// use geocoordinates::Length;
    ///
    /// // The single-step inverse is coarser than the refined one.
    /// let gcj = Gcj02::new(39.916_404, 116.410_244)?;
    /// assert_eq!(gcj.to_wgs84_fast().max_error(), Length::from_meters(6.0));
    /// assert_eq!(gcj.to_wgs84_refined().max_error().meters(), 0.5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn max_error(&self) -> Length {
        self.max_error
    }

    /// Map the inner value while preserving the error estimate.
    ///
    /// ```
    /// use geocoordinates::china::Gcj02;
    /// use geocoordinates::{Approx, Coordinate};
    ///
    /// let wgs = Gcj02::new(39.916_404, 116.410_244)?.to_wgs84_refined();
    /// let coord: Approx<Coordinate> = wgs.map(|w| w.to_coordinate());
    /// assert_eq!(coord.max_error().meters(), 0.5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Approx<U> {
        Approx {
            value: f(self.value),
            max_error: self.max_error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Approx;
    use crate::{Error, Length};

    #[test]
    fn new_rejects_nan_and_negative_and_accepts_zero_and_infinity() {
        for bad in [-1.0, -f64::INFINITY, f64::NAN, -1e-300] {
            assert!(
                matches!(
                    Approx::new(1_u8, Length::from_meters(bad)),
                    Err(Error::InvalidValue {
                        field: "max_error",
                        ..
                    })
                ),
                "accepted {bad}"
            );
        }
        for ok in [0.0, -0.0, 1e-9, 2.5, f64::INFINITY] {
            let a = Approx::new(1_u8, Length::from_meters(ok)).unwrap();
            assert_eq!(a.max_error().meters().to_bits(), ok.to_bits());
            assert_eq!(*a.value(), 1);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn finite_bound_serializes_as_a_number() {
        let a = Approx::new(1_u8, Length::from_meters(2.5)).unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"value":1,"max_error_m":2.5}"#);
        assert_eq!(serde_json::from_str::<Approx<u8>>(&json).unwrap(), a);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn unbounded_serializes_as_null_and_reads_back_as_infinity() {
        let a = Approx::new(1_u8, Length::from_meters(f64::INFINITY)).unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"value":1,"max_error_m":null}"#);
        assert_eq!(serde_json::from_str::<Approx<u8>>(&json).unwrap(), a);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn deserialize_rejects_a_negative_bound() {
        let err = serde_json::from_str::<Approx<u8>>(r#"{"value":1,"max_error_m":-2.0}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("max_error"), "{err}");
    }
}
