//! MGRS (Military Grid Reference System) strings.
//!
//! An MGRS string (e.g. `4QFJ12345678`) is a grid-zone designator, 100 km
//! square ID, and easting/northing digits. It is validated at construction
//! ([`TryFrom<&str>`](Mgrs::try_from) / [`FromStr`], and serde deserialization),
//! which can fail on invalid grid letters or digits. Because the reference is
//! then valid by construction, decoding is infallible; it yields a square with
//! extent, so [`Mgrs::to_coordinate`] returns [`Approx`]. The square size is
//! the closed set [`MgrsPrecision`].
//!
//! Both the UTM band (zones 1–60, latitude bands C–X) and the polar UPS caps
//! (zone letters A/B/Y/Z) are supported.

use core::str::FromStr;

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::grids::utm::{K0 as UTM_K0, UPS_K0, Ups, Utm, UtmHemisphere};

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

/// The side of an MGRS square: the closed set of precisions an MGRS reference
/// can express, one per easting/northing digit count (0 through 5 digits each).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MgrsPrecision {
    /// 100 km square: grid-zone designator and square id, no digits.
    Km100,
    /// 10 km square: one easting and one northing digit.
    Km10,
    /// 1 km square: two digits each.
    Km1,
    /// 100 m square: three digits each.
    M100,
    /// 10 m square: four digits each.
    M10,
    /// 1 m square: five digits each.
    M1,
}

impl MgrsPrecision {
    /// The square's side in meters: 100 000, 10 000, 1 000, 100, 10, or 1.
    #[must_use]
    pub fn meters(self) -> u32 {
        10u32.pow(5 - self.digits())
    }

    /// Easting (and northing) digit count, 0 through 5.
    fn digits(self) -> u32 {
        match self {
            MgrsPrecision::Km100 => 0,
            MgrsPrecision::Km10 => 1,
            MgrsPrecision::Km1 => 2,
            MgrsPrecision::M100 => 3,
            MgrsPrecision::M10 => 4,
            MgrsPrecision::M1 => 5,
        }
    }

    /// The precision of a reference with `digits` easting digits; `None`
    /// beyond five.
    fn from_digits(digits: u32) -> Option<Self> {
        Some(match digits {
            0 => MgrsPrecision::Km100,
            1 => MgrsPrecision::Km10,
            2 => MgrsPrecision::Km1,
            3 => MgrsPrecision::M100,
            4 => MgrsPrecision::M10,
            5 => MgrsPrecision::M1,
            _ => return None,
        })
    }
}

/// A validated MGRS reference. Construct via [`TryFrom<&str>`](Mgrs::try_from),
/// `TryFrom<String>`, [`FromStr`], or [`Mgrs::try_from_coordinate`].
///
/// [`Display`](core::fmt::Display) writes the canonical string, which parses
/// back to an equal value. Under the `serde` feature it (de)serializes as that
/// bare string, and deserialization validates it exactly as parsing does.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
pub struct Mgrs {
    text: String,
    precision: MgrsPrecision,
}

impl Mgrs {
    /// The canonical MGRS string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The square size implied by the digit count.
    #[must_use]
    pub fn precision(&self) -> MgrsPrecision {
        self.precision
    }

    /// Decode to a coordinate at the square's center. Infallible — the
    /// reference was validated at construction.
    ///
    /// The error bound is the square's half diagonal converted from grid to
    /// ground meters: `side / √2 / k_min`, where `side` is
    /// [`MgrsPrecision::meters`] and `k_min` is the
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
            f64::from(self.precision.meters()) * core::f64::consts::FRAC_1_SQRT_2
                / min_scale_factor(&self.text),
        )
    }

    /// Encode a WGS-84 coordinate to the MGRS square of the given size that
    /// contains it: UTM-based between 80°S and 84°N, UPS-based (zone letters
    /// A/B/Y/Z) in the polar caps.
    ///
    /// # Errors
    /// Returns [`crate::Error::CrsMismatch`] for a non-WGS-84 coordinate.
    pub fn try_from_coordinate(coord: Coordinate, precision: MgrsPrecision) -> Result<Self> {
        if coord.crs() != crate::Crs::Wgs84 {
            return Err(Error::CrsMismatch {
                expected: crate::Crs::Wgs84,
                found: coord.crs(),
            });
        }
        let digits = precision.digits();
        let text = if (-80.0..84.0).contains(&coord.lat()) {
            encode_utm(coord, digits)
        } else {
            encode_ups(coord, digits)
        };
        Ok(Mgrs { text, precision })
    }
}

impl TryFrom<&str> for Mgrs {
    type Error = crate::Error;

    /// Parse and validate an MGRS string. Whitespace is ignored, letters are
    /// uppercased, and a one-digit UTM zone is zero-padded (`4Q…` → `04Q…`),
    /// so the stored string is canonical: the same form
    /// [`Mgrs::try_from_coordinate`] emits.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] on a bad grid-zone designator,
    /// 100 km square id, or digit group.
    fn try_from(s: &str) -> Result<Self> {
        let (_, precision) = decode(s)?;
        let mut text: String = s
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_uppercase();
        // `decode` accepted it, so a leading digit followed by a non-digit is
        // a one-digit zone.
        let bytes = text.as_bytes();
        if bytes[0].is_ascii_digit() && !bytes[1].is_ascii_digit() {
            text.insert(0, '0');
        }
        Ok(Mgrs { text, precision })
    }
}

impl TryFrom<String> for Mgrs {
    type Error = crate::Error;

    /// Validate as [`TryFrom<&str>`](Mgrs::try_from) does; this is the serde
    /// route.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn try_from(s: String) -> Result<Self> {
        Self::try_from(s.as_str())
    }
}

impl FromStr for Mgrs {
    type Err = crate::Error;

    /// Equivalent to [`TryFrom<&str>`](Mgrs::try_from).
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

impl From<Mgrs> for String {
    /// The canonical string.
    fn from(mgrs: Mgrs) -> String {
        mgrs.text
    }
}

impl core::fmt::Display for Mgrs {
    /// Writes the canonical string, which parses back to an equal value.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.text)
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
    let set = ((utm.zone() - 1) % 3) as usize;
    let col = UTM_COLS[set].as_bytes()[(utm.easting() / 100_000.0) as usize - 1] as char;
    let row_off = if utm.zone() % 2 == 0 { 5 } else { 0 };
    let row = UTM_ROW.as_bytes()[((utm.northing() / 100_000.0) as usize + row_off) % 20] as char;
    format!(
        "{:02}{}{}{}{}",
        utm.zone(),
        band_letter(coord.lat()),
        col,
        row,
        digit_pair(utm.easting(), utm.northing(), digits),
    )
}

fn encode_ups(coord: Coordinate, digits: u32) -> String {
    let ups = Ups::try_from_coordinate(coord).expect("latitude is in the polar band");
    let north = ups.hemisphere() == UtmHemisphere::North;
    let east = ups.easting() >= 2_000_000.0;
    let zl = match (north, east) {
        (true, false) => 'Y',
        (true, true) => 'Z',
        (false, false) => 'A',
        (false, true) => 'B',
    };
    let (col_tbl, col_off) = ups_column_table(zl);
    let col = col_tbl.as_bytes()[(ups.easting() / 100_000.0) as usize - col_off] as char;
    let row_off = if north { 13 } else { 8 };
    let row = UPS_ROW.as_bytes()[(ups.northing() / 100_000.0) as usize - row_off] as char;
    format!(
        "{}{}{}{}",
        zl,
        col,
        row,
        digit_pair(ups.easting(), ups.northing(), digits)
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
/// center plus the precision its digit count implies.
fn decode(input: &str) -> Result<(Coordinate, MgrsPrecision)> {
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

/// Split the trailing easting/northing digits into `(precision, e, n)`.
fn split_digits(digits: &str, bad: impl Fn() -> Error) -> Result<(MgrsPrecision, f64, f64)> {
    if digits.len() % 2 != 0 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let half = digits.len() / 2;
    let precision = MgrsPrecision::from_digits(half as u32).ok_or_else(&bad)?;
    if half == 0 {
        return Ok((precision, 0.0, 0.0));
    }
    let e: f64 = digits[..half].parse().map_err(|_| bad())?;
    let n: f64 = digits[half..].parse().map_err(|_| bad())?;
    Ok((precision, e, n))
}

fn decode_utm(s: &str, bad: impl Fn() -> Error + Copy) -> Result<(Coordinate, MgrsPrecision)> {
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
    let (precision, e_dig, n_dig) = split_digits(chars.as_str(), bad)?;

    let band_idx = BAND.find(band).ok_or_else(bad)?;
    let set = ((zone - 1) % 3) as usize;
    let col_num = UTM_COLS[set].find(col).ok_or_else(bad)? + 1;
    let row_off = if zone % 2 == 0 { 5 } else { 0 };
    let row_in = UTM_ROW.find(row).ok_or_else(bad)?;
    let row_val = (row_in + 20 - row_off) % 20;

    let scale = digit_scale(precision.digits());
    let easting = col_num as f64 * 100_000.0 + e_dig * scale + scale / 2.0;
    let n_base = n_dig * scale + scale / 2.0;
    let north = band >= 'N';
    let lo = -80.0 + 8.0 * band_idx as f64;
    let hi = if band == 'X' { 84.0 } else { lo + 8.0 };
    let hemisphere = if north {
        UtmHemisphere::North
    } else {
        UtmHemisphere::South
    };

    // The row letter fixes the northing only mod 2 000 km; the latitude band
    // selects which 20-row block it sits in. A candidate block outside the UTM
    // domain is just another miss: surfacing its `InvalidValue` would mask the
    // band/row mismatch, which is an invalid grid reference.
    for block in 0..11u32 {
        let northing = (row_val as f64 + 20.0 * f64::from(block)) * 100_000.0 + n_base;
        let Ok(utm) = Utm::new(zone, hemisphere, easting, northing) else {
            continue;
        };
        let coord = utm.to_coordinate();
        if lo - 0.5 <= coord.lat() && coord.lat() <= hi + 0.5 {
            return Ok((coord, precision));
        }
    }
    Err(bad())
}

fn decode_ups(s: &str, bad: impl Fn() -> Error + Copy) -> Result<(Coordinate, MgrsPrecision)> {
    let mut chars = s.chars();
    let zl = chars.next().ok_or_else(bad)?;
    if !matches!(zl, 'A' | 'B' | 'Y' | 'Z') {
        return Err(bad());
    }
    let col = chars.next().ok_or_else(bad)?;
    let row = chars.next().ok_or_else(bad)?;
    let (precision, e_dig, n_dig) = split_digits(chars.as_str(), bad)?;

    let north = zl == 'Y' || zl == 'Z';
    let (col_tbl, col_off) = ups_column_table(zl);
    let col_idx = col_tbl.find(col).ok_or_else(bad)?;
    let row_idx = UPS_ROW.find(row).ok_or_else(bad)?;
    let row_off = if north { 13 } else { 8 };

    let scale = digit_scale(precision.digits());
    let easting = (col_idx + col_off) as f64 * 100_000.0 + e_dig * scale + scale / 2.0;
    let northing = (row_idx + row_off) as f64 * 100_000.0 + n_dig * scale + scale / 2.0;
    let hemisphere = if north {
        UtmHemisphere::North
    } else {
        UtmHemisphere::South
    };
    let coord = Ups::new(hemisphere, easting, northing)?.to_coordinate();
    Ok((coord, precision))
}

#[cfg(test)]
mod tests;
