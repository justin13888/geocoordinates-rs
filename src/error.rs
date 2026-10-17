//! Library error type.

use crate::coord::Crs;

/// The result type returned across the public API.
///
/// The error type defaults to [`Error`] but can be overridden, so importing
/// this alias never breaks a signature that names its own error type. The
/// [`prelude`](crate::prelude) does not export it.
///
/// ```
/// use geocoordinates::{Coordinate, Result};
///
/// fn origin_shift(lat: f64, lon: f64) -> Result<Coordinate> {
///     Coordinate::wgs84(lat + 1.0, lon)
/// }
/// assert_eq!(origin_shift(10.0, 20.0)?.lat(), 11.0);
/// assert!(origin_shift(89.5, 0.0).is_err());
///
/// // A second parameter overrides the error type.
/// let parsed: Result<u8, std::num::ParseIntError> = "300".parse();
/// assert!(parsed.is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// All errors produced by `geocoordinates`.
///
/// Conversions that can fail return [`Result`]; conversions that are merely
/// *approximate* do not fail — they return [`crate::Approx`] instead.
///
/// The enum is `#[non_exhaustive]`, so a `match` on it needs a wildcard arm:
///
/// ```
/// use geocoordinates::{Coordinate, Error};
///
/// match Coordinate::wgs84(95.0, 10.0) {
///     Err(Error::OutOfRange { lat, lon }) => assert_eq!((lat, lon), (95.0, 10.0)),
///     Err(other) => panic!("unexpected error: {other}"),
///     Ok(c) => panic!("accepted an invalid latitude: {c:?}"),
/// }
/// let e = Coordinate::wgs84(95.0, 10.0).unwrap_err();
/// assert_eq!(e.to_string(), "coordinate out of valid range: lat=95, lon=10");
/// ```
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A latitude or longitude was not a valid geographic value: non-finite,
    /// or outside `[-90, 90]` / `[-180, 180]` degrees.
    ///
    /// A *valid* point that merely lies outside an operation's area of use is
    /// [`Error::OutsideDomain`] instead.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// assert_eq!(
    ///     Coordinate::wgs84(0.0, -180.5),
    ///     Err(Error::OutOfRange { lat: 0.0, lon: -180.5 })
    /// );
    /// ```
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
    ///
    /// ```
    /// use geocoordinates::grids::Utm;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let arctic = Coordinate::wgs84(85.5, 10.0)?;
    /// assert!(matches!(
    ///     Utm::try_from_coordinate(arctic),
    ///     Err(Error::OutsideDomain { operation: "UTM", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
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
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::angle::{Dms, Hemisphere};
    ///
    /// let err = Dms::new(10, 60, 0.0, Hemisphere::North).unwrap_err();
    /// assert!(matches!(err, Error::InvalidValue { field: "minutes", .. }));
    /// ```
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue {
        /// Name of the invalid field or option.
        field: &'static str,
        /// Human-readable description of the required domain.
        detail: String,
    },

    /// A discrete global-grid identifier was not valid for its grid system.
    ///
    /// Produced by the `h3` / `s2` cell-id constructors:
    ///
    /// ```
    /// # #[cfg(feature = "h3")]
    /// # {
    /// use geocoordinates::Error;
    /// use geocoordinates::dgg::H3Cell;
    ///
    /// assert_eq!(H3Cell::new(0), Err(Error::InvalidCellId { grid: "H3", value: 0 }));
    /// # }
    /// ```
    #[error("invalid {grid} cell id: {value}")]
    InvalidCellId {
        /// Grid system name, such as `H3` or `S2`.
        grid: &'static str,
        /// Offending numeric identifier.
        value: u64,
    },

    /// Free-text, sensor, or interchange input could not be parsed into a
    /// coordinate.
    ///
    /// [`kind`](Error::Parse::kind) says why, so a caller branches on it
    /// rather than on the message: a streaming NMEA reader skips
    /// [`ParseErrorKind::NoFix`] sentences but reports
    /// [`ParseErrorKind::Checksum`] ones. `detail` is for humans and its
    /// wording is not stable.
    ///
    /// A position that parsed but is not a valid coordinate is not a parse
    /// error everywhere: the interchange parsers report it as
    /// [`Error::OutOfRange`] or [`Error::InvalidValue`] instead.
    ///
    /// ```
    /// use geocoordinates::parse::parse_coordinate;
    /// use geocoordinates::{Error, ParseErrorKind};
    ///
    /// let err = parse_coordinate("somewhere north").unwrap_err();
    /// assert!(matches!(err, Error::Parse { kind: ParseErrorKind::Syntax, .. }));
    /// ```
    #[error("could not parse coordinate: {detail}")]
    Parse {
        /// Why the input was rejected.
        kind: ParseErrorKind,
        /// Human-readable description of what was wrong with the input.
        detail: String,
    },

    /// A coordinate was used where a different reference system was required —
    /// e.g. converting a [`Coordinate`](crate::Coordinate) tagged one [`Crs`]
    /// into a datum newtype of another. Prevents silently laundering a datum.
    ///
    /// ```
    /// use geocoordinates::china::Wgs84;
    /// use geocoordinates::{Coordinate, Crs, Error};
    ///
    /// let mars = Coordinate::gcj02(39.9165, 116.4102)?;
    /// assert_eq!(
    ///     Wgs84::try_from_coordinate(mars),
    ///     Err(Error::CrsMismatch { expected: Crs::Wgs84, found: Crs::Gcj02 })
    /// );
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[error("crs mismatch: expected {expected}, found {found}")]
    CrsMismatch {
        /// The reference system that was required.
        expected: Crs,
        /// The reference system the coordinate actually carried.
        found: Crs,
    },

    /// A grid reference (MGRS / Plus Code / geohash / Maidenhead) was malformed
    /// or fell outside the grid's valid domain (e.g. a bad grid-zone letter).
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::grids::Geohash;
    ///
    /// // `a` is not in the geohash base-32 alphabet.
    /// assert!(matches!(Geohash::try_from("u4pruyda"), Err(Error::InvalidGridRef(_))));
    /// ```
    #[error("invalid grid reference: {0}")]
    InvalidGridRef(String),
}

impl Error {
    /// An [`Error::Parse`] of the given kind.
    pub(crate) fn parse(kind: ParseErrorKind, detail: impl Into<String>) -> Self {
        Error::Parse {
            kind,
            detail: detail.into(),
        }
    }
}

/// Why an [`Error::Parse`] rejected its input.
///
/// Every parser in this crate picks one of these for each rejection:
///
/// | Parser | Kinds it produces |
/// |---|---|
/// | Free text ([`parse_text`](crate::parse::parse_text), [`parse_coordinate`](crate::parse::parse_coordinate)) | [`Syntax`](Self::Syntax), [`Ambiguous`](Self::Ambiguous) |
/// | `geo:` URI ([`from_geo_uri`](crate::parse::from_geo_uri)) | [`Syntax`](Self::Syntax), [`Unsupported`](Self::Unsupported) (a `crs` other than `wgs84`) |
/// | NMEA 0183 (`parse::sensors::from_nmea_sentence`) | [`Syntax`](Self::Syntax), [`Checksum`](Self::Checksum), [`NoFix`](Self::NoFix), [`Unsupported`](Self::Unsupported) (a sentence other than `GGA` / `RMC` / `GLL`) |
/// | GeoJSON, WKT, GPX, KML (`parse::interchange`) | [`Syntax`](Self::Syntax), [`TooDeep`](Self::TooDeep) |
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every
/// variant, so adding one here is a deliberate, compile-forcing change on both
/// sides.
///
/// ```
/// use geocoordinates::parse::parse_text;
/// use geocoordinates::{Error, ParseErrorKind};
///
/// // `40.5 30` is neither 40.5° nor 40° 30′ with certainty, so it is rejected.
/// let Err(Error::Parse { kind, .. }) = parse_text("40.5 30, 10") else {
///     panic!("expected a parse error");
/// };
/// assert_eq!(kind, ParseErrorKind::Ambiguous);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ParseErrorKind {
    /// The input does not follow the format's grammar, or a field holds a
    /// value the format does not allow (minutes of 60, a negative HDOP, a
    /// latitude past 90° in free text).
    Syntax,
    /// An NMEA `*HH` checksum is malformed or does not match the sentence.
    Checksum,
    /// A well-formed sentence that reports no position: NMEA `RMC` / `GLL`
    /// status `V`, mode indicator `N`, or `GGA` fix quality `0`. A streaming
    /// reader can skip it rather than treat it as corrupt input.
    NoFix,
    /// Free text with no single reading: concatenated `DDMM.mmm` without a
    /// hemisphere letter, a fraction on a field before the last (`40.5 30`),
    /// or exponent notation (`4e1`).
    Ambiguous,
    /// Well-formed input of a type or option this crate does not handle: an
    /// NMEA sentence other than `GGA` / `RMC` / `GLL`, or a `geo:` URI `crs`
    /// other than `wgs84`.
    Unsupported,
    /// A document nested deeper than its format's limit: 128 levels of WKT
    /// parentheses or GeoJSON arrays and objects, 32 levels of KML elements.
    TooDeep,
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
        let parse = |kind| Error::Parse {
            kind,
            detail: "a".into(),
        };
        assert_ne!(
            parse(ParseErrorKind::Syntax),
            Error::InvalidGridRef("a".into())
        );
        assert_ne!(parse(ParseErrorKind::Syntax), parse(ParseErrorKind::NoFix));
        // The kind is not in the message: `detail` alone is.
        assert_eq!(
            parse(ParseErrorKind::TooDeep).to_string(),
            "could not parse coordinate: a"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn parse_error_kind_round_trips_through_serde() {
        for kind in [
            ParseErrorKind::Syntax,
            ParseErrorKind::Checksum,
            ParseErrorKind::NoFix,
            ParseErrorKind::Ambiguous,
            ParseErrorKind::Unsupported,
            ParseErrorKind::TooDeep,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(serde_json::from_str::<ParseErrorKind>(&json).unwrap(), kind);
        }
        assert_eq!(
            serde_json::to_string(&ParseErrorKind::NoFix).unwrap(),
            r#""NoFix""#
        );
    }
}
