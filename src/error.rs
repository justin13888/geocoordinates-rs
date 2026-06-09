//! Library error type.

use crate::coord::Crs;

/// The result type returned across the public API.
pub type Result<T> = core::result::Result<T, Error>;

/// All errors produced by `gcoordinates`.
///
/// Conversions that can fail return [`Result`]; conversions that are merely
/// *approximate* do not fail — they return [`crate::Approx`] instead.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A latitude/longitude (or projected coordinate) fell outside its valid
    /// domain.
    #[error("coordinate out of valid range: lat={lat}, lon={lon}")]
    OutOfRange {
        /// Offending latitude in degrees.
        lat: f64,
        /// Offending longitude in degrees.
        lon: f64,
    },

    /// Free-text / structured input could not be parsed into a coordinate.
    #[error("could not parse coordinate: {0}")]
    Parse(String),

    /// The requested runtime conversion is not supported by [`crate::convert`].
    #[error("unsupported conversion: {from:?} -> {to:?}")]
    UnsupportedConversion {
        /// Source reference system.
        from: Crs,
        /// Target reference system.
        to: Crs,
    },

    /// An optional capability was requested whose cargo feature is disabled.
    #[error("feature `{0}` is not enabled")]
    FeatureDisabled(&'static str),
    // TODO(impl): add variants as modules are fleshed out (projection domain
    // errors, MGRS grid-zone errors, geoid data not loaded, PROJ errors, ...).
}
