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
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Approx<T> {
    value: T,
    max_error_m: f64,
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
