//! The [`Approx`] wrapper for approximate (lossy or iterative) results.

use core::ops::Deref;

/// Wraps the result of an **approximate** conversion together with an estimated
/// error bound.
///
/// Any conversion that inverts a lossy or secret transform (e.g. GCJ-02 →
/// WGS-84), iterates to a tolerance, or decodes a cell with spatial extent
/// (e.g. geohash) returns `Approx<T>` rather than a bare `T`. This makes the
/// approximation impossible to ignore at the call site, and exposes the bound
/// via [`Approx::max_error_m`].
///
/// `Approx<T>` derefs to `T`, so `approx.lat()` and similar accessors work
/// directly.
///
/// Note: the derived [`PartialEq`] compares [`max_error_m`](Approx::max_error_m)
/// too, so two approximations of the same point with different bounds are not
/// equal. Compare [`value`](Approx::value) explicitly for a bound-agnostic test.
///
/// Under the `serde` feature, `max_error_m` serializes as an optional number:
/// a finite bound is written as itself, and an unbounded one
/// (`f64::INFINITY`, e.g. a Pulkovo-1942 datum shift) as `null` / none, which
/// deserializes back to `f64::INFINITY`. The bound therefore survives formats
/// such as JSON that have no representation for infinity.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Approx<T> {
    value: T,
    #[cfg_attr(feature = "serde", serde(with = "max_error_serde"))]
    max_error_m: f64,
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
    /// Construct from a value and its estimated maximum error in meters.
    #[must_use]
    pub(crate) fn new(value: T, max_error_m: f64) -> Self {
        Self { value, max_error_m }
    }

    /// Borrow the underlying value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Consume the wrapper, discarding the error estimate.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }

    /// The estimated upper bound on positional error, in meters.
    #[must_use]
    pub fn max_error_m(&self) -> f64 {
        self.max_error_m
    }

    /// Map the inner value while preserving the error estimate.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Approx<U> {
        Approx {
            value: f(self.value),
            max_error_m: self.max_error_m,
        }
    }
}

impl<T> Deref for Approx<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::Approx;

    #[test]
    fn finite_bound_serializes_as_a_number() {
        let a = Approx::new(1_u8, 2.5);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"value":1,"max_error_m":2.5}"#);
        assert_eq!(serde_json::from_str::<Approx<u8>>(&json).unwrap(), a);
    }

    #[test]
    fn unbounded_serializes_as_null_and_reads_back_as_infinity() {
        let a = Approx::new(1_u8, f64::INFINITY);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"value":1,"max_error_m":null}"#);
        assert_eq!(serde_json::from_str::<Approx<u8>>(&json).unwrap(), a);
    }
}
