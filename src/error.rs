//! Library error type.

use crate::coord::Crs;

/// The result type returned across the public API.
///
/// The error type defaults to [`Error`] but can be overridden, so importing
/// this alias never breaks a signature that names its own error type. The
/// [`prelude`](crate::prelude) does not export it, so a glob import of the
/// prelude never shadows the standard library's `Result`.
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
///
/// Naming every current variant without a wildcard arm does not compile:
///
/// ```compile_fail,E0004
/// use geocoordinates::Error;
///
/// fn name(e: Error) -> &'static str {
///     // Expected: E0004, non-exhaustive patterns: `_` not covered.
///     match e {
///         Error::OutOfRange { .. } => "out of range",
///         Error::OutsideDomain { .. } => "outside domain",
///         Error::InvalidValue { .. } => "invalid value",
///         Error::InvalidCellId { .. } => "invalid cell id",
///         Error::Parse { .. } => "parse",
///         Error::CrsMismatch { .. } => "crs mismatch",
///         Error::InvalidGridRef { .. } => "invalid grid ref",
///     }
/// }
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
    /// `operation` names the operation or projection, so a caller can branch
    /// on it without reading the message. Its values are stable across
    /// releases: `"UTM"`, `"UPS"`, `"Baidu Mercator"`, `"BD-09"` (a forward
    /// BD-09 conversion) and `"BD-09 inverse"`. Each names a projection rather
    /// than a parameter, so it keeps the projection's own spelling instead of
    /// snake_case; a new operation adds a value, and an existing one is never
    /// renamed outside a breaking release.
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
        /// Name of the operation or projection whose domain was left: one of
        /// the stable names listed above, such as `"UTM"`.
        operation: &'static str,
        /// Latitude in degrees.
        lat: f64,
        /// Longitude in degrees.
        lon: f64,
    },

    /// A caller-provided option or numeric value was outside the operation's
    /// supported domain.
    ///
    /// `field` is the snake_case name of what was invalid, read from the
    /// signature: the parameter (`Dms::from_degrees(value, ..)` reports
    /// `"value"`, `Geohash::encode(.., length)` reports `"length"`), the field
    /// of the record or option struct (`"radius_px"` of `ClusterIndexOptions`),
    /// or, where a constructor checks a quantity derived from several
    /// parameters, that quantity's name (`"magnitude"` for an `Enu` / `Ned`
    /// vector too long to be a position). The values are stable across
    /// releases, so a caller may match on them; `detail` is for humans and its
    /// wording is not stable.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::angle::{Axis, Dms, Hemisphere};
    ///
    /// let err = Dms::new(10, 60, 0.0, Hemisphere::North).unwrap_err();
    /// assert!(matches!(err, Error::InvalidValue { field: "minutes", .. }));
    /// let err = Dms::from_degrees(f64::NAN, Axis::Latitude).unwrap_err();
    /// assert!(matches!(err, Error::InvalidValue { field: "value", .. }));
    /// ```
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue {
        /// The snake_case name of the invalid parameter, record field or
        /// option; stable across releases.
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
    /// A position that parsed but is off the globe is not a parse error:
    /// every text reader (`geo:` URI, free text, NMEA, GeoJSON / WKT / KML,
    /// EXIF) reports it as [`Error::OutOfRange`] with the values as read, and
    /// the interchange parsers report another invalid component as
    /// [`Error::InvalidValue`]. The one exception is a GPX `lat` / `lon` out of
    /// range, which the `gpx` reader rejects as `Syntax`.
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

    /// A grid reference (MGRS / Plus Code / geohash / Maidenhead / quadkey) was
    /// malformed or fell outside the grid's valid domain (e.g. a bad grid-zone
    /// letter).
    ///
    /// `grid` names the system that rejected it, so a caller handling several
    /// can branch on it without reading `detail`. Its values are stable:
    ///
    /// | `grid` | Producer |
    /// |---|---|
    /// | `"MGRS"` | [`Mgrs`](crate::grids::Mgrs) |
    /// | `"geohash"` | [`Geohash`](crate::grids::Geohash) |
    /// | `"Plus Code"` | [`PlusCode`](crate::grids::PlusCode) |
    /// | `"Maidenhead"` | [`Maidenhead`](crate::grids::Maidenhead) |
    /// | `"quadkey"` | `map::Tile::from_quadkey` (`map` feature) |
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::grids::Geohash;
    ///
    /// // `a` is not in the geohash base-32 alphabet.
    /// let err = Geohash::try_from("u4pruyda").unwrap_err();
    /// assert!(matches!(err, Error::InvalidGridRef { grid: "geohash", .. }));
    /// assert_eq!(
    ///     err.to_string(),
    ///     r#"invalid geohash reference: "u4pruyda" holds 'a', not a base-32 digit"#
    /// );
    /// ```
    #[error("invalid {grid} reference: {detail}")]
    InvalidGridRef {
        /// The grid system that rejected the reference: one of the stable
        /// names in the table above.
        grid: &'static str,
        /// The rejected input, quoted as a Rust string literal, followed by
        /// why it was rejected; for people, not for matching.
        detail: String,
    },
}

impl Error {
    /// An [`Error::InvalidGridRef`] from `grid`, whose `detail` is the quoted
    /// `input` followed by `reason` (a predicate such as `"is empty"`), so
    /// every producer writes it the same way.
    pub(crate) fn invalid_grid_ref(
        grid: &'static str,
        input: &str,
        reason: impl core::fmt::Display,
    ) -> Self {
        Error::InvalidGridRef {
            grid,
            detail: format!("{input:?} {reason}"),
        }
    }

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
/// | `geo:` URI ([`parse_geo_uri`](crate::parse::parse_geo_uri)) | [`Syntax`](Self::Syntax), [`Unsupported`](Self::Unsupported) (a `crs` other than `wgs84`) |
/// | NMEA 0183 (`parse::sensors::parse_nmea_sentence`) | [`Syntax`](Self::Syntax), [`Checksum`](Self::Checksum), [`NoFix`](Self::NoFix), [`Unsupported`](Self::Unsupported) (a sentence other than `GGA` / `RMC` / `GLL`) |
/// | GeoJSON, WKT, GPX, KML (`parse::interchange`) | [`Syntax`](Self::Syntax), [`TooDeep`](Self::TooDeep) |
/// | EXIF / XMP GPS tags (`exif`) | [`NoFix`](Self::NoFix), [`Syntax`](Self::Syntax) (XMP strings only) |
/// | Gazetteer files (`reverse::Gazetteer::from_bytes`) | [`Syntax`](Self::Syntax), [`Checksum`](Self::Checksum) (payload CRC-32), [`Unsupported`](Self::Unsupported) (a format version or flag this build does not read) |
///
/// The enum is `#[non_exhaustive]`, like [`Error`]: each new input format
/// brings failure modes of its own, so a kind can be added without a breaking
/// release, and a `match` on it needs a wildcard arm. Treat an unknown kind as
/// [`Syntax`](Self::Syntax), the generic "the text does not read", which the
/// FFI mirror does too.
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
///
/// // A match needs a wildcard arm, which also covers kinds added later.
/// let reason = match kind {
///     ParseErrorKind::Ambiguous => "no single reading",
///     ParseErrorKind::Checksum => "corrupt",
///     ParseErrorKind::NoFix => "no position reported",
///     _ => "does not read",
/// };
/// assert_eq!(reason, "no single reading");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// The input does not follow the format's grammar, or a field holds a
    /// value the format does not allow (minutes of 60, a negative HDOP, a
    /// latitude past 90° in free text).
    Syntax,
    /// A checksum is malformed or does not match its data: an NMEA `*HH`
    /// checksum, or a gazetteer file's payload CRC-32.
    Checksum,
    /// A well-formed sentence that reports no position: NMEA `RMC` / `GLL`
    /// status `V`, mode indicator `N`, or `GGA` fix quality `0`; or EXIF / XMP
    /// GPS tags with no usable position. A streaming reader can skip it rather
    /// than treat it as corrupt input.
    NoFix,
    /// Free text with no single reading: concatenated `DDMM.mmm` without a
    /// hemisphere letter, a fraction on a field before the last (`40.5 30`),
    /// or exponent notation (`4e1`).
    Ambiguous,
    /// Well-formed input of a type or option this crate does not handle: an
    /// NMEA sentence other than `GGA` / `RMC` / `GLL`, a `geo:` URI `crs`
    /// other than `wgs84`, or a gazetteer file of a format version this build
    /// does not read.
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
            Error::InvalidGridRef {
                grid: "MGRS",
                detail: "a".into(),
            }
        );
        assert_ne!(parse(ParseErrorKind::Syntax), parse(ParseErrorKind::NoFix));
        // The kind is not in the message: `detail` alone is.
        assert_eq!(
            parse(ParseErrorKind::TooDeep).to_string(),
            "could not parse coordinate: a"
        );
    }

    /// Every non-test source file under `src/`, with its path and text. A file
    /// is a test file when it is named `tests.rs` or `*_tests.rs`, or sits in a
    /// `tests` directory.
    fn library_sources() -> Vec<(std::path::PathBuf, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
            for entry in std::fs::read_dir(dir).expect("readable directory") {
                let path = entry.expect("directory entry").path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if path.is_dir() {
                    if name != "tests" {
                        walk(&path, out);
                    }
                } else if name.ends_with(".rs")
                    && name != "tests.rs"
                    && !name.ends_with("_tests.rs")
                {
                    let text = std::fs::read_to_string(&path).expect("readable source");
                    out.push((path, text));
                }
            }
        }
        let mut out = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut out,
        );
        assert!(out.len() > 50, "the walk found only {} files", out.len());
        out
    }

    /// Each string literal written right after `key` (as in `field: "x"`),
    /// with the file and line it appears on.
    fn literals_after(key: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for (path, text) in library_sources() {
            for (n, line) in text.lines().enumerate() {
                let mut rest = line;
                while let Some(at) = rest.find(key) {
                    rest = &rest[at + key.len()..];
                    if let Some(value) = rest.split('"').next().filter(|_| rest.contains('"')) {
                        found.push((value.to_owned(), format!("{}:{}", path.display(), n + 1)));
                    }
                }
            }
        }
        found
    }

    /// The argument list `text` opens with, its `(` already consumed, split
    /// at its top-level commas, or `None` when it never closes. A string
    /// literal is skipped whole, so a comma or bracket inside one is not read.
    fn arguments(text: &str) -> Option<Vec<&str>> {
        let (mut depth, mut start, mut in_string, mut escaped) = (1_usize, 0, false, false);
        let mut args = Vec::new();
        for (i, c) in text.char_indices() {
            if in_string {
                (in_string, escaped) = (escaped || c != '"', !escaped && c == '\\');
                continue;
            }
            match c {
                '"' => in_string = true,
                '(' | '[' | '{' => depth += 1,
                ',' if depth == 1 => {
                    args.push(&text[start..i]);
                    start = i + 1;
                }
                ')' | ']' | '}' if depth == 1 => {
                    args.push(&text[start..i]);
                    args.retain(|a| !a.trim().is_empty());
                    return Some(args);
                }
                ')' | ']' | '}' => depth -= 1,
                _ => {}
            }
        }
        None
    }

    /// Each string literal passed for the parameter named `param` (as in
    /// `field: &'static str`) of a library function, with the file and byte
    /// offset of the call: the producers that hand a name to a helper rather
    /// than write `field: "x"` inline. A call resolves to the helper of that
    /// name in its own file, or else to one declared `pub` in another.
    fn helper_arguments(param: &str) -> Vec<(String, String)> {
        let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let sources = library_sources();
        let wanted = format!("{param}: &'static str");
        // (file index, name, parameter position, takes `self`, `pub`)
        let mut helpers = Vec::new();
        for (file, (_, text)) in sources.iter().enumerate() {
            for (at, _) in text.match_indices("fn ") {
                let rest = &text[at + 3..];
                let name: String = rest.chars().take_while(|&c| is_ident(c)).collect();
                let params = rest[name.len()..].strip_prefix('(').and_then(arguments);
                let params = params.unwrap_or_default();
                if let Some(index) = params.iter().position(|p| p.trim() == wanted) {
                    let method = params[0].trim().ends_with("self");
                    let shared = text[..at]
                        .rsplit('\n')
                        .next()
                        .is_some_and(|l| l.contains("pub"));
                    helpers.push((file, name, index, method, shared));
                }
            }
        }
        let mut found = Vec::new();
        for (file, (path, text)) in sources.iter().enumerate() {
            for (home, name, index, method, shared) in &helpers {
                let local = helpers.iter().any(|h| h.0 == file && h.1 == *name);
                if *home != file && (!shared || local) {
                    continue;
                }
                for (at, _) in text.match_indices(&format!("{name}(")) {
                    let before = text[..at].chars().next_back().unwrap_or(' ');
                    // A definition, a longer name, or another type's method.
                    if is_ident(before) || text[..at].ends_with("fn ") || before == '.' && !method {
                        continue;
                    }
                    let args = arguments(&text[at + name.len() + 1..]).unwrap_or_default();
                    let arg = args.get(index - usize::from(before == '.'));
                    if let Some(literal) = arg.and_then(|a| a.trim().strip_prefix('"')) {
                        let value = literal.split('"').next().unwrap_or_default();
                        found.push((value.to_owned(), format!("{}@{at}", path.display())));
                    }
                }
            }
        }
        found
    }

    fn is_snake_case(s: &str) -> bool {
        let mut chars = s.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    #[test]
    fn snake_case_check_accepts_only_identifiers() {
        for good in ["value", "semi_major", "x_m", "level2"] {
            assert!(is_snake_case(good), "{good}");
        }
        for bad in [
            "",
            "S2 level",
            "decimal degrees",
            "Value",
            "2d",
            "_x",
            "semi-major",
        ] {
            assert!(!is_snake_case(bad), "{bad}");
        }
    }

    #[test]
    fn invalid_value_fields_are_snake_case_names() {
        let fields = literals_after("field: \"");
        // The scan sees the producers and the doctests that match on them.
        assert!(
            fields.iter().any(|(v, _)| v == "minutes"),
            "the scan found no `field:` literal"
        );
        let bad: Vec<_> = fields.iter().filter(|(v, _)| !is_snake_case(v)).collect();
        assert!(bad.is_empty(), "`field` values not in snake_case: {bad:?}");
    }

    /// A `field` name a producer hands to a helper (`checked_decimals(..,
    /// "decimals")`) is held to the same rule as one written inline.
    #[test]
    fn invalid_value_fields_include_helper_arguments() {
        let fields = helper_arguments("field");
        // A private helper (`fix.rs`), one whose name recurs privately in
        // other files (`finite`, `baidu_mercator.rs`), and a crate-wide one
        // called from another file (`ellipsoid_of_argument`).
        for expected in ["horizontal", "minute_decimals", "x", "from_crs"] {
            assert!(
                fields.iter().any(|(v, _)| v == expected),
                "the helper scan missed {expected:?}: {fields:?}"
            );
        }
        let bad: Vec<_> = fields.iter().filter(|(v, _)| !is_snake_case(v)).collect();
        assert!(bad.is_empty(), "`field` values not in snake_case: {bad:?}");
    }

    /// `InvalidGridRef` / `InvalidCellId` name their grid by the stable names
    /// the variants document, inline or through `Error::invalid_grid_ref`.
    #[test]
    fn grid_names_are_the_documented_names() {
        const GRID_REF: [&str; 5] = ["MGRS", "geohash", "Plus Code", "Maidenhead", "quadkey"];
        const CELL_ID: [&str; 2] = ["H3", "S2"];
        // Inline, either variant; through the helper, `InvalidGridRef` only.
        for (found, sentinel, cell_ids) in [
            (literals_after("grid: \""), "H3", true),
            (helper_arguments("grid"), "Plus Code", false),
        ] {
            assert!(found.iter().any(|(v, _)| v == sentinel), "{found:?}");
            let documented = |v: &str| GRID_REF.contains(&v) || cell_ids && CELL_ID.contains(&v);
            let bad: Vec<_> = found.iter().filter(|(v, _)| !documented(v)).collect();
            assert!(bad.is_empty(), "undocumented `grid` values: {bad:?}");
        }
    }

    #[test]
    fn outside_domain_operations_are_the_documented_names() {
        const DOCUMENTED: [&str; 5] = ["UTM", "UPS", "Baidu Mercator", "BD-09", "BD-09 inverse"];
        let mut operations = literals_after("operation: \"");
        assert!(
            operations.iter().any(|(v, _)| v == "UTM"),
            "the scan found no `operation:` literal"
        );
        // `bd09.rs` names its operation through `within_poles`.
        let through_helper = helper_arguments("operation");
        assert!(
            through_helper.iter().any(|(v, _)| v == "BD-09 inverse"),
            "the helper scan found no `operation` argument"
        );
        operations.extend(through_helper);
        let bad: Vec<_> = operations
            .iter()
            .filter(|(v, _)| !DOCUMENTED.contains(&v.as_str()))
            .collect();
        assert!(bad.is_empty(), "undocumented `operation` values: {bad:?}");
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
