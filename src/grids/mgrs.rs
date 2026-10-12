//! MGRS (Military Grid Reference System) strings.
//!
//! An MGRS string (e.g. `4QFJ12345678`) is a grid-zone designator, 100 km
//! square ID, and easting/northing digits. It is validated at construction
//! ([`TryFrom<&str>`](Mgrs::try_from) / [`FromStr`]), which can fail on invalid
//! grid letters. Because the reference is then valid by construction, decoding
//! is infallible; it yields a square with extent, so [`Mgrs::to_coordinate`]
//! returns [`Approx`].
//!
//! Both the UTM band (zones 1–60, latitude bands C–X) and the polar UPS caps
//! (zone letters A/B/Y/Z) are supported.

use core::str::FromStr;

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::grids::utm::{Hemisphere, K0 as UTM_K0, UPS_K0, Ups, Utm};

// --- 100 km square lettering tables (I and O are always excluded) ---
/// UTM column letters, selected by `(zone − 1) mod 3`.
const UTM_COLS: [&str; 3] = ["ABCDEFGH", "JKLMNPQR", "STUVWXYZ"];
/// UTM row letters (20, no I/O), offset by 5 rows on even zones.
const UTM_ROW: &str = "ABCDEFGHJKLMNPQRSTUV";
/// Latitude band letters, C (−80°) … X (72°–84°).
const BAND: &str = "CDEFGHJKLMNPQRSTUVWX";
/// UPS column letters per zone letter (Y/Z north, A/B south).
const UPS_COL_Y: &str = "RSTUXYZ";
const UPS_COL_Z: &str = "ABCFGHJ";
const UPS_COL_A: &str = "JKLPQRSTUXYZ";
const UPS_COL_B: &str = "ABCFGHJKLPQR";
/// UPS row letters (24, no I/O).
const UPS_ROW: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";

/// A validated MGRS reference. Construct via [`TryFrom<&str>`](Mgrs::try_from),
/// [`FromStr`], or [`Mgrs::try_from_coordinate`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mgrs {
    text: String,
    precision_m: u32,
}

impl Mgrs {
    /// The canonical MGRS string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Precision in meters (10 km, 1 km, … 1 m) implied by the digit count.
    #[must_use]
    pub fn precision_m(&self) -> u32 {
        self.precision_m
    }

    /// Decode to a coordinate at the square's center. Infallible — the
    /// reference was validated at construction.
    ///
    /// The error bound is the square's half diagonal converted from grid to
    /// ground meters: `precision_m / √2 / k_min`, where `k_min` is the
    /// projection's minimum point scale factor (0.9996 for UTM, 0.994 for
    /// UPS). Grid distance is ground distance times the point scale factor k,
    /// and k never falls below `k_min`, so the bound holds everywhere in the
    /// square. It overstates the true worst case by the local excess of k over
    /// `k_min`: about 0.14% at a standard UTM zone edge on the equator, and
    /// under 0.8% at the UPS cap's 80°S edge.
    #[must_use]
    pub fn to_coordinate(&self) -> Approx<Coordinate> {
        let (coord, _) = decode(&self.text).expect("MGRS was validated at construction");
        Approx::new(
            coord,
            f64::from(self.precision_m) * core::f64::consts::FRAC_1_SQRT_2
                / min_scale_factor(&self.text),
        )
    }

    /// Encode a WGS-84 coordinate to MGRS at a power-of-ten precision from
    /// 1 meter through 100 kilometers.
    pub fn try_from_coordinate(coord: Coordinate, precision_m: u32) -> Result<Self> {
        if coord.crs() != crate::Crs::Wgs84 {
            return Err(Error::CrsMismatch {
                expected: crate::Crs::Wgs84,
                found: coord.crs(),
            });
        }
        if !matches!(precision_m, 1 | 10 | 100 | 1_000 | 10_000 | 100_000) {
            return Err(Error::InvalidValue {
                field: "MGRS precision",
                detail: "must be 1, 10, 100, 1000, 10000, or 100000 meters".into(),
            });
        }
        let digits = 5 - precision_m.ilog10();
        let text = if (-80.0..84.0).contains(&coord.lat()) {
            encode_utm(coord, digits)
        } else {
            encode_ups(coord, digits)
        };
        Ok(Mgrs {
            text,
            precision_m: 10u32.pow(5 - digits),
        })
    }
}

impl TryFrom<&str> for Mgrs {
    type Error = crate::Error;

    /// Parse and validate an MGRS string.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] on a bad grid-zone designator
    /// or 100 km square id.
    fn try_from(s: &str) -> Result<Self> {
        let (_, precision_m) = decode(s)?;
        let text: String = s
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_uppercase();
        Ok(Mgrs { text, precision_m })
    }
}

impl FromStr for Mgrs {
    type Err = crate::Error;

    /// Equivalent to [`TryFrom<&str>`](Mgrs::try_from).
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

/// The latitude band letter for a UTM-band latitude.
fn band_letter(lat: f64) -> char {
    let idx = (((lat + 80.0) / 8.0).floor() as usize).min(19);
    BAND.as_bytes()[idx] as char
}

/// Scale factor (meters per least-significant digit) for `digits` digits.
fn digit_scale(digits: u32) -> f64 {
    10f64.powi(5 - digits as i32)
}

/// The zero-padded within-square easting/northing digit pair (empty at 100 km
/// precision, where `digits == 0`).
fn digit_pair(easting: f64, northing: f64, digits: u32) -> String {
    if digits == 0 {
        return String::new();
    }
    let scale = digit_scale(digits);
    let e = ((easting % 100_000.0) / scale) as u64;
    let n = ((northing % 100_000.0) / scale) as u64;
    let width = digits as usize;
    format!("{e:0width$}{n:0width$}")
}

fn encode_utm(coord: Coordinate, digits: u32) -> String {
    let utm = Utm::try_from_coordinate(coord).expect("latitude is in the UTM band");
    let set = ((utm.zone - 1) % 3) as usize;
    let col = UTM_COLS[set].as_bytes()[(utm.easting / 100_000.0) as usize - 1] as char;
    let row_off = if utm.zone % 2 == 0 { 5 } else { 0 };
    let row = UTM_ROW.as_bytes()[((utm.northing / 100_000.0) as usize + row_off) % 20] as char;
    format!(
        "{:02}{}{}{}{}",
        utm.zone,
        band_letter(coord.lat()),
        col,
        row,
        digit_pair(utm.easting, utm.northing, digits),
    )
}

fn encode_ups(coord: Coordinate, digits: u32) -> String {
    let ups = Ups::try_from_coordinate(coord).expect("latitude is in the polar band");
    let north = ups.hemisphere == Hemisphere::North;
    let east = ups.easting >= 2_000_000.0;
    let zl = match (north, east) {
        (true, false) => 'Y',
        (true, true) => 'Z',
        (false, false) => 'A',
        (false, true) => 'B',
    };
    let (col_tbl, col_off) = ups_column_table(zl);
    let col = col_tbl.as_bytes()[(ups.easting / 100_000.0) as usize - col_off] as char;
    let row_off = if north { 13 } else { 8 };
    let row = UPS_ROW.as_bytes()[(ups.northing / 100_000.0) as usize - row_off] as char;
    format!(
        "{}{}{}{}",
        zl,
        col,
        row,
        digit_pair(ups.easting, ups.northing, digits)
    )
}

/// The UPS column-letter table and its `floor(easting/100km)` offset.
fn ups_column_table(zl: char) -> (&'static str, usize) {
    match zl {
        'Y' => (UPS_COL_Y, 13),
        'Z' => (UPS_COL_Z, 20),
        'A' => (UPS_COL_A, 8),
        _ => (UPS_COL_B, 20),
    }
}

/// The minimum point scale factor of the projection a canonical MGRS string
/// lives in: UTM references start with the zone number, UPS ones with a letter.
fn min_scale_factor(canonical: &str) -> f64 {
    if canonical.starts_with(|c: char| c.is_ascii_digit()) {
        UTM_K0
    } else {
        UPS_K0
    }
}

/// Parse, validate, and decode an MGRS string to a coordinate at the cell
/// center plus the implied precision in meters.
fn decode(input: &str) -> Result<(Coordinate, u32)> {
    let s: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    let bad = || Error::InvalidGridRef(input.to_string());
    let first = s.chars().next().ok_or_else(bad)?;
    if first.is_ascii_digit() {
        decode_utm(&s, bad)
    } else {
        decode_ups(&s, bad)
    }
}

/// Split the trailing easting/northing digits into `(half_count, e, n)`.
fn split_digits(digits: &str, bad: impl Fn() -> Error) -> Result<(u32, f64, f64)> {
    if digits.len() % 2 != 0 || digits.len() > 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let k = (digits.len() / 2) as u32;
    if k == 0 {
        return Ok((0, 0.0, 0.0));
    }
    let half = digits.len() / 2;
    let e: f64 = digits[..half].parse().map_err(|_| bad())?;
    let n: f64 = digits[half..].parse().map_err(|_| bad())?;
    Ok((k, e, n))
}

fn decode_utm(s: &str, bad: impl Fn() -> Error + Copy) -> Result<(Coordinate, u32)> {
    let split = s.find(|c: char| !c.is_ascii_digit()).ok_or_else(bad)?;
    if split == 0 || split > 2 {
        return Err(bad());
    }
    let zone: u8 = s[..split].parse().map_err(|_| bad())?;
    if !(1..=60).contains(&zone) {
        return Err(bad());
    }
    let rest = &s[split..];
    if rest.len() < 3 {
        return Err(bad());
    }
    let mut chars = rest.chars();
    let band = chars.next().ok_or_else(bad)?;
    let col = chars.next().ok_or_else(bad)?;
    let row = chars.next().ok_or_else(bad)?;
    let (k, e_dig, n_dig) = split_digits(chars.as_str(), bad)?;

    let band_idx = BAND.find(band).ok_or_else(bad)?;
    let set = ((zone - 1) % 3) as usize;
    let col_num = UTM_COLS[set].find(col).ok_or_else(bad)? + 1;
    let row_off = if zone % 2 == 0 { 5 } else { 0 };
    let row_in = UTM_ROW.find(row).ok_or_else(bad)?;
    let row_val = (row_in + 20 - row_off) % 20;

    let scale = digit_scale(k);
    let easting = col_num as f64 * 100_000.0 + e_dig * scale + scale / 2.0;
    let n_base = n_dig * scale + scale / 2.0;
    let north = band >= 'N';
    let lo = -80.0 + 8.0 * band_idx as f64;
    let hi = if band == 'X' { 84.0 } else { lo + 8.0 };
    let hemisphere = if north {
        Hemisphere::North
    } else {
        Hemisphere::South
    };

    // The row letter fixes the northing only mod 2 000 km; the latitude band
    // selects which 20-row block it sits in. A candidate block outside the UTM
    // domain is just another miss: surfacing its `InvalidValue` would mask the
    // band/row mismatch, which is an invalid grid reference.
    for block in 0..11u32 {
        let northing = (row_val as f64 + 20.0 * f64::from(block)) * 100_000.0 + n_base;
        let Ok(coord) = Utm {
            zone,
            hemisphere,
            easting,
            northing,
        }
        .try_to_coordinate() else {
            continue;
        };
        if lo - 0.5 <= coord.lat() && coord.lat() <= hi + 0.5 {
            return Ok((coord, 10u32.pow(5 - k)));
        }
    }
    Err(bad())
}

fn decode_ups(s: &str, bad: impl Fn() -> Error + Copy) -> Result<(Coordinate, u32)> {
    let mut chars = s.chars();
    let zl = chars.next().ok_or_else(bad)?;
    if !matches!(zl, 'A' | 'B' | 'Y' | 'Z') {
        return Err(bad());
    }
    let col = chars.next().ok_or_else(bad)?;
    let row = chars.next().ok_or_else(bad)?;
    let (k, e_dig, n_dig) = split_digits(chars.as_str(), bad)?;

    let north = zl == 'Y' || zl == 'Z';
    let (col_tbl, col_off) = ups_column_table(zl);
    let col_idx = col_tbl.find(col).ok_or_else(bad)?;
    let row_idx = UPS_ROW.find(row).ok_or_else(bad)?;
    let row_off = if north { 13 } else { 8 };

    let scale = digit_scale(k);
    let easting = (col_idx + col_off) as f64 * 100_000.0 + e_dig * scale + scale / 2.0;
    let northing = (row_idx + row_off) as f64 * 100_000.0 + n_dig * scale + scale / 2.0;
    let coord = Ups {
        hemisphere: if north {
            Hemisphere::North
        } else {
            Hemisphere::South
        },
        easting,
        northing,
    }
    .try_to_coordinate()?;
    Ok((coord, 10u32.pow(5 - k)))
}

#[cfg(test)]
mod tests;
