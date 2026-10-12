//! The FFI error type and its translation from the core crate's error.

use geocoordinates as gc;

use crate::Crs;

/// Errors surfaced across the FFI boundary — a one-to-one mirror of [`gc::Error`].
///
/// Every core variant has its own typed variant here, with the same fields and
/// the same display message, so binding users match on the variant rather than
/// on message text. Reference systems cross as the [`Crs`] enum.
/// [`GeoError::Other`] is only the fallback for a core variant this mirror does
/// not know yet (`gc::Error` is `#[non_exhaustive]`).
#[derive(Debug, Clone, PartialEq, thiserror::Error, uniffi::Error)]
pub enum GeoError {
    /// A latitude or longitude was not a valid geographic value: non-finite, or
    /// outside `[-90, 90]` / `[-180, 180]` degrees.
    #[error("coordinate out of valid range: lat={lat}, lon={lon}")]
    OutOfRange {
        /// Offending latitude in degrees.
        lat: f64,
        /// Offending longitude in degrees.
        lon: f64,
    },
    /// A valid latitude/longitude lies outside the area an operation or
    /// projection is defined on (e.g. UTM outside 80°S–84°N).
    #[error("coordinate outside the domain of {operation}: lat={lat}, lon={lon}")]
    OutsideDomain {
        /// Name of the operation or projection whose domain was left.
        operation: String,
        /// Latitude in degrees.
        lat: f64,
        /// Longitude in degrees.
        lon: f64,
    },
    /// A caller-provided option or numeric value was outside the operation's
    /// supported domain.
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue {
        /// Name of the invalid field or option.
        field: String,
        /// Human-readable description of the required domain.
        detail: String,
    },
    /// A discrete global-grid identifier was not valid for its grid system.
    #[error("invalid {grid} cell id: {value}")]
    InvalidCellId {
        /// Grid system name, such as `H3` or `S2`.
        grid: String,
        /// Offending numeric identifier.
        value: u64,
    },
    /// Free-text / structured input could not be parsed into a coordinate.
    #[error("could not parse coordinate: {detail}")]
    Parse {
        /// What was wrong with the input.
        detail: String,
    },
    /// A coordinate carried the wrong reference system for the requested operation.
    #[error(
        "crs mismatch: expected {}, found {}",
        gc::Crs::from(*.expected),
        gc::Crs::from(*.found)
    )]
    CrsMismatch {
        /// The reference system that was required.
        expected: Crs,
        /// The reference system the coordinate actually carried.
        found: Crs,
    },
    /// A grid reference (MGRS / Plus Code / geohash / Maidenhead) was malformed
    /// or fell outside the grid's valid domain.
    #[error("invalid grid reference: {detail}")]
    InvalidGridRef {
        /// The offending grid reference text.
        detail: String,
    },
    /// A core error variant this mirror does not yet have, with its message
    /// preserved.
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
            gc::Error::OutOfRange { lat, lon } => GeoError::OutOfRange { lat, lon },
            gc::Error::OutsideDomain {
                operation,
                lat,
                lon,
            } => GeoError::OutsideDomain {
                operation: operation.to_string(),
                lat,
                lon,
            },
            gc::Error::InvalidValue { field, detail } => GeoError::InvalidValue {
                field: field.to_string(),
                detail,
            },
            gc::Error::InvalidCellId { grid, value } => GeoError::InvalidCellId {
                grid: grid.to_string(),
                value,
            },
            gc::Error::Parse(detail) => GeoError::Parse { detail },
            gc::Error::CrsMismatch { expected, found } => GeoError::CrsMismatch {
                expected: expected.into(),
                found: found.into(),
            },
            gc::Error::InvalidGridRef(detail) => GeoError::InvalidGridRef { detail },
            // `gc::Error` is `#[non_exhaustive]`: a variant added upstream before
            // this mirror learns it keeps its message rather than failing the build.
            other => GeoError::Other {
                detail: other.to_string(),
            },
        }
    }
}
