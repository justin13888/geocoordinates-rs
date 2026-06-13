//! Encoded/discrete location systems.
//!
//! Currently: **Plus Codes** (Open Location Code). Geohash and Maidenhead are
//! scaffolded but deferred — see `ROADMAP.md`.
//!
//! Encoding a point into a cell is exact; [`decode`](PlusCode::decode) returns
//! the cell **center** wrapped in [`Approx`], carrying the
//! cell's half-diagonal (in meters) as the error bound. Strings are validated
//! at construction ([`TryFrom<&str>`](PlusCode::try_from) / [`FromStr`]).
//!
//! [`FromStr`]: std::str::FromStr

use core::str::FromStr;

use crate::angle::{clamp_latitude, wrap_longitude};
use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};

// Deferred (uncommented with the Geohash + Maidenhead milestone — see ROADMAP.md):
//
// /// A validated geohash string (base-32), e.g. `dr5regy`.
// #[derive(Debug, Clone, PartialEq, Eq)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
// pub struct Geohash(String);
//
// /// A validated Maidenhead locator (amateur radio grid square), e.g. `FN20`.
// #[derive(Debug, Clone, PartialEq, Eq)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
// pub struct Maidenhead(String);

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
    /// 2, 4, 6, 8, 10, and 11–15); it is clamped to `[2, 15]` and rounded up to
    /// an even length below 10. Longitude is wrapped and latitude clamped, so
    /// the antimeridian and poles are handled.
    #[must_use]
    pub fn encode(coord: Coordinate, length: usize) -> Self {
        let length = normalize_length(length);
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

        PlusCode(assemble(&digits, length))
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

/// Clamp the requested code length to `[2, 15]`, rounding an odd length below 10
/// up to the next even value (only even pair lengths are valid below 10).
fn normalize_length(length: usize) -> usize {
    match length.clamp(2, MAX_DIGIT_COUNT) {
        3 => 4,
        5 => 6,
        7 => 8,
        9 => 10,
        other => other,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{assert_close, assert_within_meters};

    #[test]
    fn encode_reference_vectors() {
        // Canonical Open Location Code test vectors.
        assert_eq!(
            PlusCode::encode(Coordinate::wgs84(20.375, 2.775), 6).as_str(),
            "7FG49Q00+"
        );
        assert_eq!(
            PlusCode::encode(Coordinate::wgs84(47.0000625, 8.0000625), 10).as_str(),
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
                let code = PlusCode::encode(c, length);
                let area = code.decode();
                assert_within_meters(&Coordinate::wgs84(c.lat, c.lon), &*area, area.max_error_m());
            }
        }
    }

    #[test]
    fn antimeridian_and_pole_do_not_panic() {
        // lat 90 / lon 180 must clamp/wrap and round-trip without panicking.
        let code = PlusCode::encode(Coordinate::wgs84(90.0, 180.0), 10);
        let area = code.decode();
        assert!(area.lat <= 90.0 && area.lon < 180.0);
    }

    #[test]
    fn shorter_lengths_pad() {
        assert_eq!(
            PlusCode::encode(Coordinate::wgs84(47.0000625, 8.0000625), 4).as_str(),
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
        let code = PlusCode::encode(Coordinate::wgs84(40.7128, -74.006), 11);
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
    fn normalize_length_maps_to_valid() {
        assert_eq!(normalize_length(0), 2); // clamp low
        assert_eq!(normalize_length(99), 15); // clamp high
        assert_eq!(normalize_length(3), 4); // odd below 10 rounds up
        assert_eq!(normalize_length(5), 6);
        assert_eq!(normalize_length(7), 8);
        assert_eq!(normalize_length(9), 10);
        assert_eq!(normalize_length(8), 8); // even stays
        assert_eq!(normalize_length(10), 10);
        assert_eq!(normalize_length(11), 11); // odd at/above 10 is valid
    }

    #[test]
    fn latitude_90_is_nudged_below_max() {
        // Encoding latitude 90 nudges it just below the maximum — identical to
        // encoding the nudged value directly.
        let at_pole = PlusCode::encode(Coordinate::wgs84(90.0, 0.0), 10);
        let nudged = PlusCode::encode(Coordinate::wgs84(90.0 - 0.000125, 0.0), 10);
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
            let area = PlusCode::encode(c, 10).decode();
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
}
