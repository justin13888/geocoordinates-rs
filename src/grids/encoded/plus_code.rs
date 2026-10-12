//! Open Location Code / Plus Codes (full codes only).

use super::{cell_center, validate_encoding_coordinate};
use crate::angle::{clamp_latitude, wrap_longitude};
use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// A validated **full** Open Location Code / Plus Code, e.g. `87G7X2VV+2V`.
///
/// Google Maps' shareable grid representation. Only **full** codes are
/// supported (short codes need a reference location to recover). The
/// canonical form is uppercase. It is valid by construction: obtain one from
/// [`PlusCode::encode`] or the validating `TryFrom<&str>` / `TryFrom<String>` /
/// [`FromStr`](core::str::FromStr); [`Display`](core::fmt::Display) writes the
/// canonical string back. Under the `serde` feature it (de)serializes as that
/// bare string, and deserialization validates the same way.
///
/// ```
/// use geocoordinates::grids::PlusCode;
///
/// let code: PlusCode = "87g7x2vv+2v".parse()?;
/// assert_eq!(code.to_string(), "87G7X2VV+2V");
/// assert!(PlusCode::try_from("X2VV+2V").is_err()); // a short code
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
pub struct PlusCode(pub(super) String);

// --- Open Location Code constants (Google's reference values) ---
const ALPHABET: &[u8; 20] = b"23456789CFGHJMPQRVWX";
const SEPARATOR: char = '+';
const SEPARATOR_POSITION: usize = 8;
const PADDING: char = '0';
const PAIR_CODE_LENGTH: usize = 10;
const GRID_CODE_LENGTH: usize = 5;
const MAX_DIGIT_COUNT: usize = 15;
const ENCODING_BASE: i64 = 20;
const GRID_ROWS: i64 = 5;
const GRID_COLUMNS: i64 = 4;
const LATITUDE_MAX: f64 = 90.0;
const LONGITUDE_MAX: f64 = 180.0;
/// Integer units per degree for the pair section (`20^3`).
const PAIR_PRECISION: i64 = 8_000;
/// Place value of the most-significant pair digit (`20^4`).
const PAIR_FIRST_PLACE_VALUE: i64 = 160_000;
/// Place value of the most-significant latitude grid digit (`5^4`).
const GRID_LAT_FIRST_PLACE_VALUE: i64 = 625;
/// Place value of the most-significant longitude grid digit (`4^4`).
const GRID_LON_FIRST_PLACE_VALUE: i64 = 256;
/// Integer units per degree of latitude at full precision (`8000 * 5^5`).
const FINAL_LAT_PRECISION: i64 = 25_000_000;
/// Integer units per degree of longitude at full precision (`8000 * 4^5`).
const FINAL_LON_PRECISION: i64 = 8_192_000;
/// Full-precision latitude units spanning the globe (`180 * 25e6`).
const LAT_UNITS: i64 = 2 * 90 * FINAL_LAT_PRECISION;
/// Full-precision longitude units spanning the globe (`360 * 8_192_000`).
const LON_UNITS: i64 = 2 * 180 * FINAL_LON_PRECISION;

impl PlusCode {
    /// Encode a WGS-84 coordinate at the given code length (exact: the
    /// coordinate lies inside the returned cell).
    ///
    /// `length` is the number of significant digits: 2, 4, 6, 8, or 10
    /// through 15. As in Google's reference `locationToIntegers`, the
    /// full-precision latitude index is clamped below the north pole and the
    /// longitude index wraps at the antimeridian, so the result always
    /// validates.
    ///
    /// ```
    /// use geocoordinates::grids::PlusCode;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let point = Coordinate::wgs84(47.000_062_5, 8.000_062_5)?;
    /// assert_eq!(PlusCode::encode(point, 10)?.as_str(), "8FVC2222+22");
    /// assert_eq!(PlusCode::encode(point, 4)?.as_str(), "8FVC0000+");
    /// assert!(matches!(PlusCode::encode(point, 9), Err(Error::InvalidValue { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns [`Error::CrsMismatch`] for a non-WGS-84 coordinate, and
    /// [`Error::InvalidValue`] for any other `length`.
    pub fn encode(coord: Coordinate, length: u8) -> Result<Self> {
        validate_encoding_coordinate(coord)?;
        if !matches!(length, 2 | 4 | 6 | 8 | 10..=15) {
            return Err(Error::InvalidValue {
                field: "Plus Code length",
                detail: "must be 2, 4, 6, 8, or 10 through 15".into(),
            });
        }
        let lat = clamp_latitude(coord.lat());
        let lon = wrap_longitude(coord.lon());

        // Full-precision integer indices, floored so the point lies in its
        // cell. Latitude 90 clamps into the last row (so the code decodes to
        // the top cell) and longitude wraps at the antimeridian: `lon` is in
        // [-180, 180), so its index is in [0, LON_UNITS], and only a longitude
        // rounding up to 180 reaches LON_UNITS and wraps to 0.
        let mut lat_val = ((lat * FINAL_LAT_PRECISION as f64).floor() as i64 + LAT_UNITS / 2)
            .clamp(0, LAT_UNITS - 1);
        let mut lon_val =
            ((lon * FINAL_LON_PRECISION as f64).floor() as i64 + LON_UNITS / 2) % LON_UNITS;

        // Build the full 15-digit code least-significant first.
        let mut digits = [0u8; MAX_DIGIT_COUNT];
        for i in 0..GRID_CODE_LENGTH {
            let ndx = (lat_val % GRID_ROWS) * GRID_COLUMNS + (lon_val % GRID_COLUMNS);
            digits[MAX_DIGIT_COUNT - 1 - i] = ALPHABET[ndx as usize];
            lat_val /= GRID_ROWS;
            lon_val /= GRID_COLUMNS;
        }
        for i in 0..(PAIR_CODE_LENGTH / 2) {
            digits[PAIR_CODE_LENGTH - 1 - 2 * i] = ALPHABET[(lon_val % ENCODING_BASE) as usize];
            digits[PAIR_CODE_LENGTH - 2 - 2 * i] = ALPHABET[(lat_val % ENCODING_BASE) as usize];
            lat_val /= ENCODING_BASE;
            lon_val /= ENCODING_BASE;
        }

        Ok(PlusCode(assemble(&digits, usize::from(length))))
    }

    /// The canonical (uppercase) Plus Code string.
    ///
    /// ```
    /// use geocoordinates::grids::PlusCode;
    ///
    /// assert_eq!(PlusCode::try_from("8fvc2222+22")?.as_str(), "8FVC2222+22");
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
    /// use geocoordinates::grids::PlusCode;
    ///
    /// let area = PlusCode::try_from("8FVC2222+22")?.decode();
    /// assert!((area.lat() - 47.000_062_5).abs() < 1e-9);
    /// assert!(area.max_error_m() < 10.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        // Significant digits only (drop the separator and padding; `0` is never
        // an alphabet character, so it can only be padding).
        let digits: Vec<i64> = self
            .0
            .bytes()
            .filter_map(|b| ALPHABET.iter().position(|&a| a == b).map(|p| p as i64))
            .collect();

        let mut normal_lat = -(LATITUDE_MAX as i64) * PAIR_PRECISION;
        let mut normal_lon = -(LONGITUDE_MAX as i64) * PAIR_PRECISION;
        let mut grid_lat = 0i64;
        let mut grid_lon = 0i64;

        let pairs = digits.len().min(PAIR_CODE_LENGTH) / 2;
        let mut place = PAIR_FIRST_PLACE_VALUE;
        for pair in 0..pairs {
            let i = pair * 2;
            normal_lat += digits[i] * place;
            normal_lon += digits[i + 1] * place;
            if pair + 1 < pairs {
                place /= ENCODING_BASE;
            }
        }
        let mut lat_resolution = place as f64 / PAIR_PRECISION as f64;
        let mut lon_resolution = place as f64 / PAIR_PRECISION as f64;

        if digits.len() > PAIR_CODE_LENGTH {
            let mut row_place = GRID_LAT_FIRST_PLACE_VALUE;
            let mut col_place = GRID_LON_FIRST_PLACE_VALUE;
            let grid_digits = digits.len().min(MAX_DIGIT_COUNT);
            let last = grid_digits - PAIR_CODE_LENGTH - 1;
            for (offset, &digit) in digits[PAIR_CODE_LENGTH..grid_digits].iter().enumerate() {
                grid_lat += (digit / GRID_COLUMNS) * row_place;
                grid_lon += (digit % GRID_COLUMNS) * col_place;
                if offset < last {
                    row_place /= GRID_ROWS;
                    col_place /= GRID_COLUMNS;
                }
            }
            lat_resolution = row_place as f64 / FINAL_LAT_PRECISION as f64;
            lon_resolution = col_place as f64 / FINAL_LON_PRECISION as f64;
        }

        // South-west corner, then the cell.
        let lat_lo = normal_lat as f64 / PAIR_PRECISION as f64
            + grid_lat as f64 / FINAL_LAT_PRECISION as f64;
        let lon_lo = normal_lon as f64 / PAIR_PRECISION as f64
            + grid_lon as f64 / FINAL_LON_PRECISION as f64;
        cell_center(
            lat_lo,
            lat_lo + lat_resolution,
            lon_lo,
            lon_lo + lon_resolution,
        )
    }
}

impl TryFrom<&str> for PlusCode {
    type Error = crate::Error;

    /// Validate and canonicalize (trim, uppercase) a **full** Open Location Code.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for malformed or short codes.
    fn try_from(s: &str) -> Result<Self> {
        let upper = s.trim().to_ascii_uppercase();
        if is_valid_full(&upper) {
            Ok(PlusCode(upper))
        } else {
            Err(Error::InvalidGridRef(format!("invalid Plus Code: {s}")))
        }
    }
}

/// Insert the separator after position 8 and trim/pad the 15 digits to `length`.
fn assemble(digits: &[u8; MAX_DIGIT_COUNT], length: usize) -> String {
    let mut full = String::with_capacity(MAX_DIGIT_COUNT + 1);
    for (i, &d) in digits.iter().enumerate() {
        if i == SEPARATOR_POSITION {
            full.push(SEPARATOR);
        }
        full.push(d as char);
    }
    if length >= SEPARATOR_POSITION {
        // `length` digits plus the separator.
        full.chars().take(length + 1).collect()
    } else {
        // Pad to the separator: `length` digits, `0`s, then `+`.
        let mut s: String = full.chars().take(length).collect();
        for _ in 0..(SEPARATOR_POSITION - length) {
            s.push(PADDING);
        }
        s.push(SEPARATOR);
        s
    }
}

/// Whether `code` (already uppercased) is a valid **full** Open Location Code.
fn is_valid_full(code: &str) -> bool {
    let bytes = code.as_bytes();
    // Exactly one separator, at the full-code position (8) — rejects short codes.
    if code.matches(SEPARATOR).count() != 1 {
        return false;
    }
    let Some(sep) = code.find(SEPARATOR) else {
        return false;
    };
    if sep != SEPARATOR_POSITION {
        return false;
    }
    // Every character is an alphabet digit, the separator, or padding.
    if bytes
        .iter()
        .any(|&b| b as char != SEPARATOR && b as char != PADDING && !ALPHABET.contains(&b))
    {
        return false;
    }
    // Padding (if present) is a contiguous block ending at the separator,
    // starting on an even, non-zero index, with nothing after the separator.
    if let Some(pad_start) = code.find(PADDING) {
        if pad_start == 0 || pad_start % 2 == 1 {
            return false;
        }
        if code[pad_start..sep].bytes().any(|b| b as char != PADDING) {
            return false;
        }
        if code.len() != sep + 1 {
            return false;
        }
    }
    // After the separator: 0 or ≥2 alphabet digits (never exactly 1, no padding).
    let after = &code[sep + 1..];
    if after.len() == 1 || after.bytes().any(|b| !ALPHABET.contains(&b)) {
        return false;
    }
    // Range: the first latitude digit ≤ 8 (180°/20) and longitude ≤ 17 (360°/20).
    let first_lat = ALPHABET.iter().position(|&a| a == bytes[0]).unwrap_or(0) as i64;
    if first_lat * ENCODING_BASE >= 2 * LATITUDE_MAX as i64 {
        return false;
    }
    let first_lon = ALPHABET.iter().position(|&a| a == bytes[1]).unwrap_or(0) as i64;
    first_lon * ENCODING_BASE < 2 * LONGITUDE_MAX as i64
}
