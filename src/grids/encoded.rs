//! Encoded/discrete location systems: **Plus Codes** (Open Location Code),
//! **Geohash**, and **Maidenhead** locators.
//!
//! Encoding a point into a cell is exact; `decode` returns the cell **center**
//! wrapped in [`Approx`], carrying the cell's half-diagonal (in meters) as the
//! error bound. Strings are validated at construction (`TryFrom<&str>` /
//! [`FromStr`]).
//!
//! [`FromStr`]: std::str::FromStr

use core::str::FromStr;

use crate::angle::{clamp_latitude, wrap_longitude};
use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// A validated geohash string (base-32), e.g. `dr5regy`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Geohash(String);

/// A validated Maidenhead locator (amateur radio grid square), e.g. `FN20`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Maidenhead(String);

/// A validated Open Location Code / Plus Code, e.g. `87G7X2VV+2V`.
///
/// Google Maps' shareable grid representation. Only **full** codes are
/// supported (short codes need a reference location to recover).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlusCode(String);

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
/// Meters per degree of latitude (spherical approximation).
const METERS_PER_DEGREE: f64 = 111_320.0;

impl PlusCode {
    /// Encode a coordinate at the given code length (exact).
    ///
    /// `length` is the number of significant digits (the canonical values are
    /// 2, 4, 6, 8, 10, and 11–15); all other values are rejected. The
    /// coordinate must be valid WGS-84. Longitude is wrapped and the valid
    /// north-pole endpoint is nudged into the final encodable cell.
    pub fn encode(coord: Coordinate, length: usize) -> Result<Self> {
        validate_encoding_coordinate(coord)?;
        if !matches!(length, 2 | 4 | 6 | 8 | 10..=15) {
            return Err(Error::InvalidValue {
                field: "Plus Code length",
                detail: "must be 2, 4, 6, 8, or 10 through 15".into(),
            });
        }
        let mut lat = clamp_latitude(coord.lat);
        let lon = wrap_longitude(coord.lon);
        // Latitude 90 must drop just below so the code can be decoded.
        if lat >= LATITUDE_MAX {
            lat = LATITUDE_MAX - latitude_precision(length);
        }

        // Integer grid units (round to minimize floating-point error).
        let mut lat_val = ((lat + LATITUDE_MAX) * FINAL_LAT_PRECISION as f64).round() as i64;
        let mut lon_val = ((lon + LONGITUDE_MAX) * FINAL_LON_PRECISION as f64).round() as i64;

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

        Ok(PlusCode(assemble(&digits, length)))
    }

    /// The canonical Plus Code string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the cell center wrapped in [`Approx`], carrying the cell's
    /// half-diagonal (meters) as the error bound. Infallible — validated at
    /// construction.
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

        // South-west corner, then the cell center.
        let lat_lo = normal_lat as f64 / PAIR_PRECISION as f64
            + grid_lat as f64 / FINAL_LAT_PRECISION as f64;
        let lon_lo = normal_lon as f64 / PAIR_PRECISION as f64
            + grid_lon as f64 / FINAL_LON_PRECISION as f64;
        let lat = lat_lo + lat_resolution / 2.0;
        let lon = lon_lo + lon_resolution / 2.0;

        // Error bound: half the cell diagonal, in meters at the cell latitude.
        let lat_m = lat_resolution * METERS_PER_DEGREE;
        let lon_m = lon_resolution * METERS_PER_DEGREE * lat.to_radians().cos();
        let max_error_m = 0.5 * lat_m.hypot(lon_m);

        Approx::new(Coordinate::wgs84(lat, lon), max_error_m)
    }
}

impl TryFrom<&str> for PlusCode {
    type Error = crate::Error;

    /// Validate and canonicalize (uppercase) a **full** Open Location Code.
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

impl FromStr for PlusCode {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

/// The latitude resolution (degrees) of a code of the given length — used only
/// to nudge latitude 90 below the maximum.
fn latitude_precision(length: usize) -> f64 {
    if length <= PAIR_CODE_LENGTH {
        20f64.powi(2 - (length as i32) / 2)
    } else {
        20f64.powi(-3) / (GRID_ROWS as f64).powi((length - PAIR_CODE_LENGTH) as i32)
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

// --- Geohash (base-32) ---

/// Geohash base-32 alphabet (omits `a`, `i`, `l`, `o`).
const GEOHASH_ALPHABET: &[u8; 32] = b"0123456789bcdefghjkmnpqrstuvwxyz";

impl Geohash {
    /// Encode a coordinate at the given character length (exact).
    pub fn encode(coord: Coordinate, length: usize) -> Result<Self> {
        validate_encoding_coordinate(coord)?;
        if !(1..=22).contains(&length) {
            return Err(Error::InvalidValue {
                field: "geohash length",
                detail: "must be in 1..=22".into(),
            });
        }
        let lat = clamp_latitude(coord.lat);
        let lon = wrap_longitude(coord.lon);
        let (mut lat_lo, mut lat_hi) = (-90.0_f64, 90.0_f64);
        let (mut lon_lo, mut lon_hi) = (-180.0_f64, 180.0_f64);
        let mut hash = String::with_capacity(length);
        let mut value = 0usize;
        for i in 0..(length * 5) {
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

    /// The canonical geohash string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the cell center; error bound is the cell half-diagonal.
    /// Infallible — validated at construction.
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        let (mut lat_lo, mut lat_hi) = (-90.0_f64, 90.0_f64);
        let (mut lon_lo, mut lon_hi) = (-180.0_f64, 180.0_f64);
        let mut even = true;
        for byte in self.0.bytes() {
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

    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for non-base-32 input.
    fn try_from(s: &str) -> Result<Self> {
        let lower = s.trim().to_ascii_lowercase();
        if lower.is_empty() || lower.bytes().any(|b| !GEOHASH_ALPHABET.contains(&b)) {
            return Err(Error::InvalidGridRef(format!("invalid geohash: {s}")));
        }
        Ok(Geohash(lower))
    }
}

impl FromStr for Geohash {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

// --- Maidenhead locator ---

impl Maidenhead {
    /// Encode a WGS-84 coordinate at exactly 1–3 character pairs.
    pub fn encode(coord: Coordinate, pairs: usize) -> Result<Self> {
        validate_encoding_coordinate(coord)?;
        if !(1..=3).contains(&pairs) {
            return Err(Error::InvalidValue {
                field: "Maidenhead pairs",
                detail: "must be in 1..=3".into(),
            });
        }
        let mut lon = wrap_longitude(coord.lon) + 180.0; // [0, 360)
        let mut lat = clamp_latitude(coord.lat) + 90.0; // [0, 180]
        let mut s = String::with_capacity(pairs * 2);

        // Field: 18 columns of 20° lon × 10° lat (letters A–R). Latitude is
        // clamped because the north pole (lat 90) sits on the upper edge.
        let lon_field = (lon / 20.0) as usize;
        let lat_field = ((lat / 10.0) as usize).min(17);
        push_offset(&mut s, b'A', lon_field);
        push_offset(&mut s, b'A', lat_field);
        lon -= lon_field as f64 * 20.0;
        lat -= lat_field as f64 * 10.0;

        if pairs >= 2 {
            // Square: 10 columns of 2° lon × 1° lat (digits 0–9).
            let lon_sq = (lon / 2.0) as usize;
            let lat_sq = (lat as usize).min(9);
            push_offset(&mut s, b'0', lon_sq);
            push_offset(&mut s, b'0', lat_sq);
            lon -= lon_sq as f64 * 2.0;
            lat -= lat_sq as f64;

            if pairs >= 3 {
                // Subsquare: 24 columns of 5′ lon × 2.5′ lat (letters a–x).
                let lon_sub = (lon * 12.0) as usize;
                let lat_sub = ((lat * 24.0) as usize).min(23);
                push_offset(&mut s, b'a', lon_sub);
                push_offset(&mut s, b'a', lat_sub);
            }
        }
        Ok(Maidenhead(s))
    }

    /// The canonical Maidenhead locator string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the grid-square center wrapped in [`Approx`]. Infallible —
    /// validated at construction.
    #[must_use]
    pub fn decode(&self) -> Approx<Coordinate> {
        let bytes = self.0.as_bytes();
        let mut lon = -180.0 + f64::from(bytes[0].to_ascii_uppercase() - b'A') * 20.0;
        let mut lat = -90.0 + f64::from(bytes[1].to_ascii_uppercase() - b'A') * 10.0;
        let mut lon_size = 20.0;
        let mut lat_size = 10.0;
        if bytes.len() >= 4 {
            lon += f64::from(bytes[2] - b'0') * 2.0;
            lat += f64::from(bytes[3] - b'0');
            lon_size = 2.0;
            lat_size = 1.0;
            if bytes.len() >= 6 {
                lon += f64::from(bytes[4].to_ascii_lowercase() - b'a') * (2.0 / 24.0);
                lat += f64::from(bytes[5].to_ascii_lowercase() - b'a') * (1.0 / 24.0);
                lon_size = 2.0 / 24.0;
                lat_size = 1.0 / 24.0;
            }
        }
        cell_center(lat, lat + lat_size, lon, lon + lon_size)
    }
}

fn validate_encoding_coordinate(coord: Coordinate) -> Result<()> {
    coord.validate()?;
    if coord.crs != crate::Crs::Wgs84 {
        return Err(Error::CrsMismatch {
            expected: crate::Crs::Wgs84,
            found: coord.crs,
        });
    }
    Ok(())
}

impl TryFrom<&str> for Maidenhead {
    type Error = crate::Error;

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
        let mut c: Vec<u8> = t.bytes().collect();
        c[0] = c[0].to_ascii_uppercase();
        c[1] = c[1].to_ascii_uppercase();
        if c.len() >= 6 {
            c[4] = c[4].to_ascii_lowercase();
            c[5] = c[5].to_ascii_lowercase();
        }
        Ok(Maidenhead(String::from_utf8(c).unwrap_or_default()))
    }
}

impl FromStr for Maidenhead {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

/// Midpoint of a `[lo, hi]` range.
fn midpoint(lo: f64, hi: f64) -> f64 {
    (lo + hi) / 2.0
}

/// Push `base + index` (as an ASCII byte) onto `s`.
fn push_offset(s: &mut String, base: u8, index: usize) {
    s.push((base + index as u8) as char);
}

/// The center of a `[lat_lo, lat_hi] × [lon_lo, lon_hi]` cell, with the
/// half-diagonal (meters, at the cell latitude) as the error bound.
fn cell_center(lat_lo: f64, lat_hi: f64, lon_lo: f64, lon_hi: f64) -> Approx<Coordinate> {
    let lat = midpoint(lat_lo, lat_hi);
    let lon = midpoint(lon_lo, lon_hi);
    let lat_half_m = (lat_hi - lat_lo) / 2.0 * METERS_PER_DEGREE;
    let lon_half_m = (lon_hi - lon_lo) / 2.0 * METERS_PER_DEGREE * lat.to_radians().cos();
    Approx::new(Coordinate::wgs84(lat, lon), lat_half_m.hypot(lon_half_m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{assert_close, assert_within_meters};

    #[test]
    fn encode_reference_vectors() {
        // Canonical Open Location Code test vectors.
        assert_eq!(
            PlusCode::encode(Coordinate::wgs84(20.375, 2.775), 6)
                .unwrap()
                .as_str(),
            "7FG49Q00+"
        );
        assert_eq!(
            PlusCode::encode(Coordinate::wgs84(47.0000625, 8.0000625), 10)
                .unwrap()
                .as_str(),
            "8FVC2222+22"
        );
    }

    #[test]
    fn decode_known_code() {
        // 8FVC2222+22 is the length-10 cell whose center is the encode input.
        let area = PlusCode::try_from("8FVC2222+22").unwrap().decode();
        assert_close(area.lat, 47.0000625, 1e-7);
        assert_close(area.lon, 8.0000625, 1e-7);
        assert!(area.max_error_m() > 0.0 && area.max_error_m() < 20.0);
    }

    #[test]
    fn round_trip_stays_within_bound() {
        // The original must lie within the decoded cell's reported error bound.
        let coords = [
            Coordinate::wgs84(40.7128, -74.006),
            Coordinate::wgs84(-33.8688, 151.2093),
            Coordinate::wgs84(0.0, 0.0),
            Coordinate::wgs84(51.5074, -0.1278),
            Coordinate::wgs84(89.9999, 179.9999), // near a pole / the antimeridian
        ];
        for c in coords {
            for length in [10, 11, 12] {
                let code = PlusCode::encode(c, length).unwrap();
                let area = code.decode();
                assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, area.max_error_m());
            }
        }
    }

    #[test]
    fn antimeridian_and_pole_do_not_panic() {
        // lat 90 / lon 180 must clamp/wrap and round-trip without panicking.
        let code = PlusCode::encode(Coordinate::wgs84(90.0, 180.0), 10).unwrap();
        let area = code.decode();
        assert!(area.lat <= 90.0 && area.lon < 180.0);
    }

    #[test]
    fn shorter_lengths_pad() {
        assert_eq!(
            PlusCode::encode(Coordinate::wgs84(47.0000625, 8.0000625), 4)
                .unwrap()
                .as_str(),
            "8FVC0000+"
        );
    }

    #[test]
    fn validation_accepts_and_rejects() {
        assert!(PlusCode::try_from("87G7X2VV+2V").is_ok());
        assert!(PlusCode::try_from("87g7x2vv+2v").is_ok()); // lowercase normalized
        assert!(PlusCode::try_from("8FVC2222+").is_ok()); // length-8 full code
        assert!(PlusCode::try_from("INVALID").is_err()); // no separator
        assert!(PlusCode::try_from("8FVC2222").is_err()); // missing separator
        assert!(PlusCode::try_from("8FVC2222+2").is_err()); // one char after separator
        assert!(PlusCode::try_from("8FVC222+22").is_err()); // separator not at position 8
        assert!(PlusCode::try_from("8FVC222A+22").is_err()); // 'A' not in the alphabet
        assert!(PlusCode::try_from("").is_err()); // empty
    }

    #[test]
    fn try_from_round_trips_canonical_string() {
        let code = PlusCode::encode(Coordinate::wgs84(40.7128, -74.006), 11).unwrap();
        let reparsed = PlusCode::try_from(code.as_str()).unwrap();
        assert_eq!(code, reparsed);
    }

    #[test]
    fn latitude_precision_values() {
        assert_close(latitude_precision(2), 20.0, 1e-9);
        assert_close(latitude_precision(8), 0.0025, 1e-9);
        assert_close(latitude_precision(10), 0.000125, 1e-12);
        assert_close(latitude_precision(11), 0.000125 / 5.0, 1e-12);
        assert_close(latitude_precision(15), 0.000125 / 3125.0, 1e-15);
    }

    #[test]
    fn invalid_encoding_configuration_is_rejected() {
        let c = Coordinate::wgs84(0.0, 0.0);
        assert!(PlusCode::encode(c, 3).is_err());
        assert!(Geohash::encode(c, 0).is_err());
        assert!(Geohash::encode(c, 23).is_err());
        assert!(Maidenhead::encode(c, 0).is_err());
        assert!(PlusCode::encode(Coordinate::gcj02(0.0, 0.0), 10).is_err());
    }

    #[test]
    fn latitude_90_is_nudged_below_max() {
        // Encoding latitude 90 nudges it just below the maximum — identical to
        // encoding the nudged value directly.
        let at_pole = PlusCode::encode(Coordinate::wgs84(90.0, 0.0), 10).unwrap();
        let nudged = PlusCode::encode(Coordinate::wgs84(90.0 - 0.000125, 0.0), 10).unwrap();
        assert_eq!(at_pole, nudged);
    }

    #[test]
    fn decode_center_and_bound_precise() {
        let area = PlusCode::try_from("8FVC2222+22").unwrap().decode();
        assert_close(area.lat, 47.0000625, 1e-9);
        assert_close(area.lon, 8.0000625, 1e-9);
        // Cell is 0.000125° square; the bound is the half-diagonal in meters.
        let lat_m: f64 = 0.000125 * 111_320.0;
        let lon_m: f64 = 0.000125 * 111_320.0 * 47.0000625_f64.to_radians().cos();
        assert_close(area.max_error_m(), 0.5 * lat_m.hypot(lon_m), 1e-6);
    }

    #[test]
    fn decode_grid_refinement() {
        // One grid digit (X = row 4, col 3) refines the length-10 cell.
        let area = PlusCode::try_from("8FVC2222+22X").unwrap().decode();
        assert_close(area.lat, 47.0001 + 0.000_025 / 2.0, 1e-9);
        assert_close(area.lon, 8.00009375 + 0.000_031_25 / 2.0, 1e-9);
        // The grid cell is far smaller than the length-10 cell (~8.4 m bound).
        assert!(area.max_error_m() < 4.0);
    }

    #[test]
    fn decode_two_grid_digits() {
        // Two grid digits exercise the row/column grid place-value divisions.
        let area = PlusCode::try_from("8FVC2222+22XX").unwrap().decode();
        assert_close(area.lat, 47.0001225, 1e-7);
        assert_close(area.lon, 8.000121094, 1e-7);
    }

    #[test]
    fn decode_center_within_absolute_bound() {
        // Coordinates with non-zero digits in every pair, checked against a
        // fixed ~14 m cell bound — so a broken pair place value (which would
        // also inflate the reported bound) can't hide behind it.
        for c in [
            Coordinate::wgs84(40.7128, -74.006),
            Coordinate::wgs84(-33.8688, 151.2093),
        ] {
            let area = PlusCode::encode(c, 10).unwrap().decode();
            assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, 20.0);
        }
    }

    #[test]
    fn validation_padding_and_range_rules() {
        // A first longitude digit of 10 (longitude 20°E) is in range and valid.
        assert!(PlusCode::try_from("2G222222+").is_ok());
        // Padding must start on an even, non-zero index, be contiguous, and end
        // at the separator.
        assert!(PlusCode::try_from("8FVC0000+").is_ok()); // well-formed padded code
        assert!(PlusCode::try_from("00000000+").is_err()); // padding at index 0
        assert!(PlusCode::try_from("8FVCC000+").is_err()); // padding starts on an odd index
        assert!(PlusCode::try_from("8FVC0000+22").is_err()); // chars after a padded code
        // First latitude digit must be ≤ 8 and longitude ≤ 17 (range limits).
        assert!(PlusCode::try_from("F2222222+").is_err()); // first lat digit = 9
        assert!(PlusCode::try_from("2W222222+").is_err()); // first lon digit = 18
    }

    // --- Geohash ---

    #[test]
    fn geohash_reference_vectors() {
        assert_eq!(
            Geohash::encode(Coordinate::wgs84(42.6, -5.6), 5)
                .unwrap()
                .as_str(),
            "ezs42"
        );
        assert_eq!(
            Geohash::encode(Coordinate::wgs84(57.64911, 10.40744), 11)
                .unwrap()
                .as_str(),
            "u4pruydqqvj"
        );
    }

    #[test]
    fn geohash_round_trip_within_bound() {
        for c in [
            Coordinate::wgs84(40.7128, -74.006),
            Coordinate::wgs84(-33.8688, 151.2093),
            Coordinate::wgs84(0.0, 0.0),
            Coordinate::wgs84(89.9, 179.9),
        ] {
            for len in [6, 8, 10] {
                let area = Geohash::encode(c, len).unwrap().decode();
                assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, area.max_error_m());
            }
        }
    }

    #[test]
    fn geohash_decode_matches_known_cell() {
        let area = Geohash::try_from("ezs42").unwrap().decode();
        // Fixed ~5 km bound (the cell is ~5 km): a decode that fails to
        // alternate lon/lat would land thousands of km away.
        assert_within_meters(&Coordinate::wgs84(42.605, -5.603), &*area, 5000.0);
        assert!(area.max_error_m() > 0.0);
    }

    #[test]
    fn geohash_validation() {
        assert!(Geohash::try_from("dr5regy").is_ok());
        assert!(Geohash::try_from("DR5REGY").is_ok()); // case-insensitive
        assert!(Geohash::try_from("ezs42a").is_err()); // 'a' is excluded
        assert!(Geohash::try_from("ezs4i").is_err()); // 'i' is excluded
        assert!(Geohash::try_from("").is_err());
    }

    // --- Maidenhead ---

    #[test]
    fn maidenhead_reference_and_decode() {
        assert_eq!(
            Maidenhead::encode(Coordinate::wgs84(40.5, -75.0), 2)
                .unwrap()
                .as_str(),
            "FN20"
        );
        let area = Maidenhead::try_from("FN20").unwrap().decode();
        assert_close(area.lat, 40.5, 1e-9);
        assert_close(area.lon, -75.0, 1e-9);
        // The half-diagonal bound of the 2°×1° cell, pinned exactly.
        let lat_half: f64 = 0.5 * 111_320.0;
        let lon_half: f64 = 111_320.0 * 40.5_f64.to_radians().cos();
        assert_close(area.max_error_m(), lat_half.hypot(lon_half), 1e-6);
    }

    #[test]
    fn maidenhead_subsquare_decode() {
        // JN58td: field J/N, square 5/8, subsquare t(19)/d(3).
        // SW corner (48.125, 11.583…); subsquare cell (2/24°)×(1/24°).
        let area = Maidenhead::try_from("JN58td").unwrap().decode();
        assert_close(area.lat, 48.125 + (1.0 / 24.0) / 2.0, 1e-9);
        assert_close(area.lon, 10.0 + 19.0 / 12.0 + (2.0 / 24.0) / 2.0, 1e-9);
    }

    #[test]
    fn maidenhead_three_pairs_round_trip() {
        let c = Coordinate::wgs84(48.146, 11.605);
        let mh = Maidenhead::encode(c, 3).unwrap();
        assert_eq!(mh.as_str().len(), 6);
        let area = mh.decode();
        assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, area.max_error_m());
    }

    #[test]
    fn maidenhead_pole_does_not_panic() {
        let area = Maidenhead::encode(Coordinate::wgs84(90.0, 180.0), 3)
            .unwrap()
            .decode();
        assert!(area.lat <= 90.0 && area.lon < 180.0);
    }

    #[test]
    fn maidenhead_validation() {
        assert!(Maidenhead::try_from("FN").is_ok()); // 2-char field-only locator
        assert!(Maidenhead::try_from("FN20").is_ok());
        assert!(Maidenhead::try_from("JN58td").is_ok());
        assert!(Maidenhead::try_from("fn20").is_ok()); // case-insensitive field
        assert!(Maidenhead::try_from("FN2").is_err()); // odd length
        assert!(Maidenhead::try_from("").is_err()); // empty
        // 2-char: each field validated (second field invalid here).
        assert!(Maidenhead::try_from("F0").is_err());
        assert!(Maidenhead::try_from("SN20").is_err()); // 'S' is past R
        // 6-char: each component validated individually.
        assert!(Maidenhead::try_from("0N58td").is_err()); // field 1 invalid
        assert!(Maidenhead::try_from("J058td").is_err()); // field 2 invalid
        assert!(Maidenhead::try_from("JNX8td").is_err()); // square 1 non-digit
        assert!(Maidenhead::try_from("JN5Xtd").is_err()); // square 2 non-digit
        assert!(Maidenhead::try_from("JN580d").is_err()); // sub 1 past x ('0')
        assert!(Maidenhead::try_from("JN58t0").is_err()); // sub 2 past x ('0')
        assert!(Maidenhead::try_from("FN20zz").is_err()); // 'z' is past x
    }
}
