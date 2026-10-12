//! Library error type.

use crate::coord::Crs;

/// The result type returned across the public API.
pub type Result<T> = core::result::Result<T, Error>;

/// All errors produced by `geocoordinates`.
///
/// Conversions that can fail return [`Result`]; conversions that are merely
/// *approximate* do not fail — they return [`crate::Approx`] instead.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A latitude or longitude was not a valid geographic value: non-finite,
    /// or outside `[-90, 90]` / `[-180, 180]` degrees.
    ///
    /// A *valid* point that merely lies outside an operation's area of use is
    /// [`Error::OutsideDomain`] instead. (The UTM / UPS forward conversions still
    /// report their latitude-band rejection here until they move to
    /// `OutsideDomain`.)
    #[error("coordinate out of valid range: lat={lat}, lon={lon}")]
    OutOfRange {
        /// Offending latitude in degrees.
        lat: f64,
        /// Offending longitude in degrees.
        lon: f64,
    },

    /// A valid latitude/longitude lies outside the area an operation or
    /// projection is defined on — e.g. a UTM conversion outside the 80°S–84°N
    /// band, a UPS conversion outside the polar caps, or Baidu Mercator beyond
    /// ±74° latitude.
    #[error("coordinate outside the domain of {operation}: lat={lat}, lon={lon}")]
    OutsideDomain {
        /// Name of the operation or projection whose domain was left, such as
        /// `"UTM"`.
        operation: &'static str,
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
        field: &'static str,
        /// Human-readable description of the required domain.
        detail: String,
    },

    /// A discrete global-grid identifier was not valid for its grid system.
    #[error("invalid {grid} cell id: {value}")]
    InvalidCellId {
        /// Grid system name, such as `H3` or `S2`.
        grid: &'static str,
        /// Offending numeric identifier.
        value: u64,
    },

    /// Free-text / structured input could not be parsed into a coordinate.
    #[error("could not parse coordinate: {0}")]
    Parse(String),

    /// A coordinate was used where a different reference system was required —
    /// e.g. converting a [`Coordinate`](crate::Coordinate) tagged one [`Crs`]
    /// into a datum newtype of another. Prevents silently laundering a datum.
    #[error("crs mismatch: expected {expected}, found {found}")]
    CrsMismatch {
        /// The reference system that was required.
        expected: Crs,
        /// The reference system the coordinate actually carried.
        found: Crs,
    },

    /// A grid reference (MGRS / Plus Code / geohash / Maidenhead) was malformed
    /// or fell outside the grid's valid domain (e.g. a bad grid-zone letter).
    #[error("invalid grid reference: {0}")]
    InvalidGridRef(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_domain_names_the_operation_and_point() {
        let e = Error::OutsideDomain {
            operation: "UTM",
            lat: 85.5,
            lon: 10.0,
        };
        assert_eq!(
            e.to_string(),
            "coordinate outside the domain of UTM: lat=85.5, lon=10"
        );
    }

    #[test]
    fn errors_compare_by_value_and_clone_equal() {
        let e = Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        };
        assert_eq!(e.clone(), e);
        assert_ne!(
            e,
            Error::CrsMismatch {
                expected: Crs::Wgs84,
                found: Crs::Bd09,
            }
        );
        assert_ne!(Error::Parse("a".into()), Error::InvalidGridRef("a".into()));
    }
}
