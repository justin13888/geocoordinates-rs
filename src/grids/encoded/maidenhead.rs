//! Maidenhead locators (amateur-radio grid squares).

use super::{cell_center, validate_encoding_coordinate};
use crate::angle::{clamp_latitude, wrap_longitude};
use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// A validated Maidenhead locator (amateur radio grid square) of 1–3
/// character pairs, e.g. `FN20` or `JN58td`.
///
/// The canonical form has an uppercase field, digit square, and lowercase
/// subsquare. It is valid by construction: obtain one from
/// [`Maidenhead::encode`] or the validating `TryFrom<&str>` /
/// `TryFrom<String>` / [`FromStr`](core::str::FromStr);
/// [`Display`](core::fmt::Display) writes the canonical string back. Under the
/// `serde` feature it (de)serializes as that bare string, and deserialization
/// validates the same way.
///
/// ```
/// use geocoordinates::grids::Maidenhead;
///
/// let locator: Maidenhead = "jn58TD".parse()?;
/// assert_eq!(locator.to_string(), "JN58td");
/// assert!(Maidenhead::try_from("SN20").is_err()); // field letters stop at R
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
pub struct Maidenhead(pub(super) String);

impl Maidenhead {
    /// Encode a WGS-84 coordinate at exactly 1–3 character pairs (exact: the
    /// coordinate lies inside the returned square). The north pole and the
    /// last longitudes before 180° land in the final field, square, and
    /// subsquare, so the result always validates.
    ///
    /// ```
    /// use geocoordinates::grids::Maidenhead;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let point = Coordinate::wgs84(40.5, -75.0)?;
    /// assert_eq!(Maidenhead::encode(point, 2)?.as_str(), "FN20");
    /// assert!(matches!(Maidenhead::encode(point, 4), Err(Error::InvalidValue { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns [`Error::CrsMismatch`] for a non-WGS-84 coordinate, and
    /// [`Error::InvalidValue`] for `pairs` outside `1..=3`.
    pub fn encode(coord: Coordinate, pairs: u8) -> Result<Self> {
        validate_encoding_coordinate(coord)?;
        if !(1..=3).contains(&pairs) {
            return Err(Error::InvalidValue {
                field: "Maidenhead pairs",
                detail: "must be in 1..=3".into(),
            });
        }
        let mut lon = wrap_longitude(coord.lon()) + 180.0; // [0, 360]
        let mut lat = clamp_latitude(coord.lat()) + 90.0; // [0, 180]
        let mut s = String::with_capacity(usize::from(pairs) * 2);

        // Each index is clamped to its last value: the north pole sits on the
        // upper edge, and a longitude just west of 180° rounds up to 360 here.
        // Field: 18 columns of 20° lon × 10° lat (letters A–R).
        let lon_field = ((lon / 20.0) as u8).min(17);
        let lat_field = ((lat / 10.0) as u8).min(17);
        s.push(char::from(b'A' + lon_field));
        s.push(char::from(b'A' + lat_field));
        lon -= f64::from(lon_field) * 20.0;
        lat -= f64::from(lat_field) * 10.0;

        if pairs >= 2 {
            // Square: 10 columns of 2° lon × 1° lat (digits 0–9).
            let lon_sq = ((lon / 2.0) as u8).min(9);
            let lat_sq = (lat as u8).min(9);
            s.push(char::from(b'0' + lon_sq));
            s.push(char::from(b'0' + lat_sq));
            lon -= f64::from(lon_sq) * 2.0;
            lat -= f64::from(lat_sq);

            if pairs >= 3 {
                // Subsquare: 24 columns of 5′ lon × 2.5′ lat (letters a–x).
                let lon_sub = ((lon * 12.0) as u8).min(23);
                let lat_sub = ((lat * 24.0) as u8).min(23);
                s.push(char::from(b'a' + lon_sub));
                s.push(char::from(b'a' + lat_sub));
            }
        }
        Ok(Maidenhead(s))
    }

    /// The canonical Maidenhead locator string: uppercase field, digit square,
    /// lowercase subsquare.
    ///
    /// ```
    /// use geocoordinates::grids::Maidenhead;
    ///
    /// assert_eq!(Maidenhead::try_from("jn58TD")?.as_str(), "JN58td");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the grid-square center wrapped in [`Approx`]; the bound is
    /// the largest geodesic distance from the center to a square corner.
    /// Infallible — validated at construction.
    ///
    /// ```
    /// use geocoordinates::grids::Maidenhead;
    ///
    /// let area = Maidenhead::try_from("FN20")?.decode();
    /// assert_eq!((area.value().lat(), area.value().lon()), (40.5, -75.0));
    /// assert!(area.max_error().meters() < 102_000.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        // Canonical by construction: uppercase field, digit square, lowercase
        // subsquare, each within its range.
        let bytes = self.0.as_bytes();
        let mut lon = -180.0 + f64::from(bytes[0] - b'A') * 20.0;
        let mut lat = -90.0 + f64::from(bytes[1] - b'A') * 10.0;
        let mut lon_size = 20.0;
        let mut lat_size = 10.0;
        if bytes.len() >= 4 {
            lon += f64::from(bytes[2] - b'0') * 2.0;
            lat += f64::from(bytes[3] - b'0');
            lon_size = 2.0;
            lat_size = 1.0;
            if bytes.len() >= 6 {
                lon += f64::from(bytes[4] - b'a') * (2.0 / 24.0);
                lat += f64::from(bytes[5] - b'a') * (1.0 / 24.0);
                lon_size = 2.0 / 24.0;
                lat_size = 1.0 / 24.0;
            }
        }
        cell_center(lat, lat + lat_size, lon, lon + lon_size)
    }
}

impl TryFrom<&str> for Maidenhead {
    type Error = crate::Error;

    /// Validate and canonicalize a 2-, 4-, or 6-character locator
    /// (case-insensitive; surrounding whitespace is trimmed).
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for malformed locators.
    fn try_from(s: &str) -> Result<Self> {
        let t = s.trim();
        let bytes = t.as_bytes();
        let field = |b: u8| (b'A'..=b'R').contains(&b.to_ascii_uppercase());
        let sub = |b: u8| (b'a'..=b'x').contains(&b.to_ascii_lowercase());
        let ok = match bytes.len() {
            2 => field(bytes[0]) && field(bytes[1]),
            4 => {
                field(bytes[0])
                    && field(bytes[1])
                    && bytes[2].is_ascii_digit()
                    && bytes[3].is_ascii_digit()
            }
            6 => {
                field(bytes[0])
                    && field(bytes[1])
                    && bytes[2].is_ascii_digit()
                    && bytes[3].is_ascii_digit()
                    && sub(bytes[4])
                    && sub(bytes[5])
            }
            _ => false,
        };
        if !ok {
            return Err(Error::InvalidGridRef(format!(
                "invalid Maidenhead locator: {s}"
            )));
        }
        // Canonical form: field uppercase, square digits, subsquare lowercase.
        let mut c = t.to_ascii_uppercase();
        if c.len() >= 6 {
            c[4..].make_ascii_lowercase();
        }
        Ok(Maidenhead(c))
    }
}
