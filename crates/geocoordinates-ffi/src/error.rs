//! The FFI error type and its translation from the core crate's error.

use geocoordinates as gc;

/// Errors surfaced across the FFI boundary — a flattened mirror of [`gc::Error`].
///
/// Datum-bearing variants carry their reference systems as their canonical
/// short names (e.g. `"BD-09"`); everything else collapses into [`GeoError::Other`].
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum GeoError {
    /// A coordinate carried the wrong reference system for the requested operation.
    #[error("crs mismatch: expected {expected}, found {found}")]
    CrsMismatch {
        /// The reference system that was required.
        expected: String,
        /// The reference system the coordinate actually carried.
        found: String,
    },
    /// A latitude/longitude fell outside its valid domain.
    #[error("coordinate out of valid range: lat={lat}, lon={lon}")]
    OutOfRange {
        /// Offending latitude in degrees.
        lat: f64,
        /// Offending longitude in degrees.
        lon: f64,
    },
    /// Any other library error, with its message preserved.
    // NB: the field is `detail`, not `message`: UniFFI's Kotlin backend emits an
    // `override val message` getter on every error variant, which would collide
    // with a field named `message` and break Kotlin/Java compilation.
    #[error("{detail}")]
    Other {
        /// The underlying error's display message.
        detail: String,
    },
}

impl From<gc::Error> for GeoError {
    fn from(e: gc::Error) -> Self {
        match e {
            gc::Error::CrsMismatch { expected, found } => GeoError::CrsMismatch {
                expected: expected.to_string(),
                found: found.to_string(),
            },
            gc::Error::OutOfRange { lat, lon } => GeoError::OutOfRange { lat, lon },
            other => GeoError::Other {
                detail: other.to_string(),
            },
        }
    }
}
