//! Geohash (base-32 interleaved bisection).

use super::{cell_center, midpoint, validate_encoding_coordinate};
use crate::angle::{clamp_latitude, wrap_longitude};
use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// Geohash base-32 alphabet (omits `a`, `i`, `l`, `o`).
const GEOHASH_ALPHABET: &[u8; 32] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// The longest geohash accepted by [`Geohash::encode`] and
/// `TryFrom<&str>`: twelve characters already resolve a cell of about
/// 37 mm × 19 mm, finer than any real positioning fix, so a longer code adds
/// only spurious precision. (An `f64` still holds it: thirteen characters
/// resolve 360°/2³³ ≈ 4.2e-8° of longitude, far above `f64`'s ≈ 2.8e-14°
/// near ±180°.)
pub(super) const GEOHASH_MAX_LENGTH: u8 = 12;

/// A validated geohash string (base-32, 1–12 characters), e.g. `dr5regy`.
///
/// The canonical form is lowercase. It is valid by construction: obtain one
/// from [`Geohash::encode`] or the validating `TryFrom<&str>` /
/// `TryFrom<String>` / [`FromStr`](core::str::FromStr); [`Display`](core::fmt::Display)
/// writes the canonical string back. Under the `serde` feature it
/// (de)serializes as that bare string, and deserialization validates the same
/// way.
///
/// ```
/// use geocoordinates::grids::Geohash;
///
/// let code = Geohash::try_from("DR5REGY")?;
/// assert_eq!(code.to_string(), "dr5regy");
/// assert!(Geohash::try_from("dr5rega").is_err()); // 'a' is not base-32
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
pub struct Geohash(pub(super) String);

impl Geohash {
    /// Encode a WGS-84 coordinate at `length` characters, 1 through 12
    /// (exact: the coordinate lies inside the returned cell).
    ///
    /// ```
    /// use geocoordinates::grids::Geohash;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let point = Coordinate::wgs84(57.64911, 10.40744)?;
    /// assert_eq!(Geohash::encode(point, 11)?.as_str(), "u4pruydqqvj");
    /// assert!(matches!(Geohash::encode(point, 13), Err(Error::InvalidValue { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns [`Error::CrsMismatch`] for a non-WGS-84 coordinate, and
    /// [`Error::InvalidValue`] for a `length` outside `1..=12`.
    pub fn encode(coord: Coordinate, length: u8) -> Result<Self> {
        validate_encoding_coordinate(coord)?;
        if !(1..=GEOHASH_MAX_LENGTH).contains(&length) {
            return Err(Error::InvalidValue {
                field: "geohash length",
                detail: "must be in 1..=12".into(),
            });
        }
        let lat = clamp_latitude(coord.lat());
        let lon = wrap_longitude(coord.lon());
        let (mut lat_lo, mut lat_hi) = (-90.0_f64, 90.0_f64);
        let (mut lon_lo, mut lon_hi) = (-180.0_f64, 180.0_f64);
        let mut hash = String::with_capacity(usize::from(length));
        let mut value = 0usize;
        for i in 0..(usize::from(length) * 5) {
            value <<= 1;
            // Even bit indices refine longitude, odd indices refine latitude.
            if i % 2 == 0 {
                let mid = midpoint(lon_lo, lon_hi);
                if lon >= mid {
                    value |= 1;
                    lon_lo = mid;
                } else {
                    lon_hi = mid;
                }
            } else {
                let mid = midpoint(lat_lo, lat_hi);
                if lat >= mid {
                    value |= 1;
                    lat_lo = mid;
                } else {
                    lat_hi = mid;
                }
            }
            if i % 5 == 4 {
                hash.push(GEOHASH_ALPHABET[value] as char);
                value = 0;
            }
        }
        Ok(Geohash(hash))
    }

    /// The canonical (lowercase) geohash string.
    ///
    /// ```
    /// use geocoordinates::grids::Geohash;
    ///
    /// assert_eq!(Geohash::try_from(" EZS42 ")?.as_str(), "ezs42");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the cell center wrapped in [`Approx`]; the bound is the
    /// largest geodesic distance from the center to a cell corner. Infallible —
    /// validated at construction.
    ///
    /// ```
    /// use geocoordinates::grids::Geohash;
    ///
    /// let area = Geohash::try_from("ezs42")?.decode();
    /// assert!((area.value().lat() - 42.605).abs() < 0.03);
    /// assert!(area.max_error().meters() < 3_100.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        let (mut lat_lo, mut lat_hi) = (-90.0_f64, 90.0_f64);
        let (mut lon_lo, mut lon_hi) = (-180.0_f64, 180.0_f64);
        let mut even = true;
        for byte in self.0.bytes() {
            // Every byte is an alphabet character: validated at construction.
            let code = GEOHASH_ALPHABET
                .iter()
                .position(|&a| a == byte)
                .unwrap_or(0);
            for shift in (0..5).rev() {
                let bit = (code >> shift) & 1;
                if even {
                    let mid = midpoint(lon_lo, lon_hi);
                    if bit == 1 {
                        lon_lo = mid;
                    } else {
                        lon_hi = mid;
                    }
                } else {
                    let mid = midpoint(lat_lo, lat_hi);
                    if bit == 1 {
                        lat_lo = mid;
                    } else {
                        lat_hi = mid;
                    }
                }
                even = !even;
            }
        }
        cell_center(lat_lo, lat_hi, lon_lo, lon_hi)
    }
}

impl TryFrom<&str> for Geohash {
    type Error = crate::Error;

    /// Validate and canonicalize (trim, lowercase) a 1–12 character geohash.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for empty, overlong, or
    /// non-base-32 input.
    fn try_from(s: &str) -> Result<Self> {
        let lower = s.trim().to_ascii_lowercase();
        if lower.is_empty()
            || lower.len() > usize::from(GEOHASH_MAX_LENGTH)
            || lower.bytes().any(|b| !GEOHASH_ALPHABET.contains(&b))
        {
            return Err(Error::InvalidGridRef(format!("invalid geohash: {s}")));
        }
        Ok(Geohash(lower))
    }
}
