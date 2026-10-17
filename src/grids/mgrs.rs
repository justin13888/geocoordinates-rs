//! MGRS (Military Grid Reference System) strings.
//!
//! An MGRS string (e.g. `4QFJ12345678`) is a grid-zone designator, 100 km
//! square ID, and easting/northing digits. It is validated at construction
//! ([`Mgrs::parse`], its `TryFrom<&str>` / [`FromStr`] sugar, and serde deserialization),
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
use crate::grids::require_wgs84;
use crate::grids::utm::{FALSE_NORTHING_SOUTH, Ups, Utm, UtmHemisphere, zone_span};
use crate::units::Length;

mod clip;
use clip::Gzd;

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
///
/// ```
/// use geocoordinates::grids::{Mgrs, MgrsPrecision};
///
/// // Two easting and two northing digits: a 1 km square.
/// assert_eq!(Mgrs::try_from("18TWL8307")?.precision(), MgrsPrecision::Km1);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
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
    /// The square's side: 100 km, 10 km, 1 km, 100 m, 10 m, or 1 m.
    ///
    /// ```
    /// use geocoordinates::grids::MgrsPrecision;
    /// use geocoordinates::{Length, LengthUnit};
    ///
    /// assert_eq!(MgrsPrecision::Km100.side().to_unit(LengthUnit::Kilometer), 100.0);
    /// assert_eq!(MgrsPrecision::M1.side(), Length::from_meters(1.0));
    /// ```
    #[must_use]
    pub fn side(self) -> Length {
        Length::from_meters(f64::from(10u32.pow(5 - self.digits())))
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

/// A validated MGRS reference. Construct via [`Mgrs::parse`] (or its sugar
/// `TryFrom<&str>`, `TryFrom<String>`, [`FromStr`]) or
/// [`Mgrs::try_from_coordinate`].
///
/// [`Display`](core::fmt::Display) writes the canonical string, which parses
/// back to an equal value. Under the `serde` feature it (de)serializes as that
/// bare string, and deserialization validates it exactly as parsing does.
///
/// ```
/// use geocoordinates::grids::Mgrs;
///
/// let mgrs: Mgrs = "4q fj 1234 5678".parse()?;
/// assert_eq!(mgrs.to_string(), "04QFJ12345678");
/// assert!("4QFJ123".parse::<Mgrs>().is_err()); // odd digit count
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
pub struct Mgrs {
    text: String,
    precision: MgrsPrecision,
    /// The point [`Mgrs::to_coordinate`] returns, found once when the string
    /// was validated, so decoding never re-parses: the square's center, or a
    /// point of the part its grid-zone designator owns where that truncates
    /// the square.
    center: Center,
    /// The error bound of `center`, in ground meters.
    max_error_m: f64,
}

/// A point of an MGRS square in the projection its grid-zone designator
/// names.
#[derive(Debug, Clone, Copy)]
enum Center {
    Utm(Utm),
    Ups(Ups),
}

impl PartialEq for Mgrs {
    /// Equal when the canonical strings are: the precision and the center are
    /// both fixed by the string.
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Mgrs {}

impl core::hash::Hash for Mgrs {
    /// Hashes the canonical string, consistent with [`PartialEq`].
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl Mgrs {
    /// The canonical MGRS string: uppercase, no whitespace, two-digit zone.
    ///
    /// ```
    /// use geocoordinates::grids::Mgrs;
    ///
    /// assert_eq!(Mgrs::try_from("4qfj12345678")?.as_str(), "04QFJ12345678");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The square size implied by the digit count.
    ///
    /// ```
    /// use geocoordinates::grids::{Mgrs, MgrsPrecision};
    ///
    /// assert_eq!(Mgrs::try_from("04QFJ12345678")?.precision(), MgrsPrecision::M10);
    /// assert_eq!(Mgrs::try_from("18TWL")?.precision(), MgrsPrecision::Km100);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn precision(&self) -> MgrsPrecision {
        self.precision
    }

    /// Decode to a coordinate at the square's center, or, where the grid-zone
    /// designator truncates the square, at a point of the part it owns.
    /// Infallible — the reference was validated at construction.
    ///
    /// The error bound is the square's half diagonal converted from grid to
    /// ground meters: `side / √2 / k_min`, where `side` is
    /// [`MgrsPrecision::side`] and `k_min` is the
    /// projection's minimum point scale factor (0.9996 for UTM, 0.994 for
    /// UPS). Grid distance is ground distance times the point scale factor k,
    /// and k never falls below `k_min`, so the bound holds everywhere in the
    /// square. It overstates the true worst case by the local excess of k over
    /// `k_min`: about 0.14% at a standard UTM zone edge on the equator, and
    /// under 0.8% at the UPS cap's 80°S edge.
    ///
    /// **Truncated squares.** A zone meridian, a latitude-band edge, the
    /// antimeridian, the Norway (32V) and Svalbard (31X–37X) exceptions, or
    /// the UTM / UPS seam at 84°N and 80°S can cut the square, and its center
    /// can then lie outside the designator, where re-encoding it would name a
    /// different reference. There the point returned is instead one deep
    /// inside the owned part: the center of the largest circle inside both
    /// the square and the designator, with the designator's edges taken as
    /// straight lines in grid space. Re-encoding it at the same precision
    /// gives this reference back. The bound is then the grid
    /// distance from that point to the square's farthest corner over
    /// `k_min`, at most twice the half-diagonal bound, and it covers every
    /// point of the owned part. A reference the parser accepts but whose
    /// square holds no point of its designator (e.g. a zone-32X square, which
    /// the Svalbard exception drops) keeps the center and half-diagonal bound.
    ///
    /// ```
    /// use geocoordinates::grids::{Mgrs, MgrsPrecision};
    ///
    /// // A 1 m square in lower Manhattan.
    /// let center = Mgrs::try_from("18TWL8395907350")?.to_coordinate();
    /// assert!((center.value().lat() - 40.7128).abs() < 1e-5);
    /// assert!((center.value().lon() + 74.006).abs() < 1e-5);
    /// assert!(center.max_error().meters() < 0.71); // 1 m / √2 / 0.9996
    ///
    /// // Zone 1's westernmost 100 km square at the equator is cut by the
    /// // antimeridian: its nominal center lies in zone 60, but the decoded
    /// // point stays in zone 1 and encodes back to the same reference.
    /// let cut = Mgrs::try_from("01NAA")?.to_coordinate();
    /// assert!(cut.value().lon() > -180.0 && cut.value().lon() < -179.0);
    /// let again = Mgrs::try_from_coordinate(*cut.value(), MgrsPrecision::Km100)?;
    /// assert_eq!(again.as_str(), "01NAA");
    /// assert!(cut.max_error().meters() < 2.0 * 70_740.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_coordinate(&self) -> Approx<Coordinate> {
        Approx::from_valid(self.center.coordinate(), self.max_error_m)
    }

    /// Encode a WGS-84 coordinate to the MGRS square of the given size that
    /// contains it: UTM-based between 80°S and 84°N, UPS-based (zone letters
    /// A/B/Y/Z) in the polar caps.
    ///
    /// ```
    /// use geocoordinates::grids::{Mgrs, MgrsPrecision};
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let nyc = Coordinate::wgs84(40.7128, -74.006)?;
    /// let mgrs = Mgrs::try_from_coordinate(nyc, MgrsPrecision::Km1)?;
    /// assert_eq!(mgrs.as_str(), "18TWL8307");
    /// let pole_cap = Coordinate::wgs84(-85.0, 30.0)?;
    /// let mgrs = Mgrs::try_from_coordinate(pole_cap, MgrsPrecision::Km1)?;
    /// assert_eq!(mgrs.as_str(), "BCS7781"); // UPS south, east of 0°
    ///
    /// let gcj = Coordinate::gcj02(39.9, 116.4)?;
    /// let err = Mgrs::try_from_coordinate(gcj, MgrsPrecision::Km1);
    /// assert!(matches!(err, Err(Error::CrsMismatch { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns [`crate::Error::CrsMismatch`] for a non-WGS-84 coordinate.
    pub fn try_from_coordinate(coord: Coordinate, precision: MgrsPrecision) -> Result<Self> {
        require_wgs84(&coord)?;
        let text = encode_text(coord, precision.digits())?;
        // The value is built by parsing the emitted string, so the encoder can
        // never hand out a reference the parser would reject: the parser is
        // the single definition of a valid `Mgrs`. The encoder always emits a
        // parseable reference (a property test checks every precision across
        // the globe), so this `?` does not fire in practice.
        Mgrs::parse(&text)
    }
}

impl Mgrs {
    /// Parse and validate an MGRS string. Whitespace is ignored, letters are
    /// uppercased, and a one-digit UTM zone is zero-padded (`4Q…` → `04Q…`),
    /// so the stored string is canonical: the same form
    /// [`Mgrs::try_from_coordinate`] emits.
    ///
    /// `TryFrom<&str>`, `TryFrom<String>`, [`FromStr`], and serde all route
    /// through this.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::grids::Mgrs;
    ///
    /// assert_eq!(Mgrs::parse("4q fj 1234 5678")?.as_str(), "04QFJ12345678");
    /// assert!(matches!(Mgrs::parse("4QFJ123"), Err(Error::InvalidGridRef(_))));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] on a bad grid-zone designator,
    /// 100 km square id, or digit group.
    pub fn parse(s: &str) -> Result<Self> {
        let Decoded {
            center: nominal,
            precision,
            gzd,
        } = decode(s)?;
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
        let (center, max_error_m) = clip::settle(
            &text,
            precision.digits(),
            nominal,
            precision.side().meters(),
            gzd,
        );
        Ok(Mgrs {
            text,
            precision,
            center,
            max_error_m,
        })
    }
}

impl TryFrom<&str> for Mgrs {
    type Error = crate::Error;

    /// Validate as [`Mgrs::parse`] does.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn try_from(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl TryFrom<String> for Mgrs {
    type Error = crate::Error;

    /// Validate as [`Mgrs::parse`] does; this is the serde route.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn try_from(s: String) -> Result<Self> {
        Self::parse(&s)
    }
}

impl FromStr for Mgrs {
    type Err = crate::Error;

    /// Equivalent to [`Mgrs::parse`].
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
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

/// The latitude band letter for a UTM-band latitude whose northing is measured
/// in the northern (`north`) or southern hemisphere.
///
/// `lat + 80.0` rounds, and can round up onto a band edge: a latitude a few
/// ulps below 72°N would land in band X while the zone rules, which compare
/// against 72 itself, still put it south of the Svalbard exception, emitting
/// a designator (`36X`) that does not exist. The index is therefore checked
/// against the edge itself, so the band and the zone agree on which side of
/// every edge the latitude lies. (Rounding never goes the other way: the sum
/// of a latitude at or above an edge is at or above the edge's exact sum.)
///
/// A latitude a few ulps south of the equator whose southern northing rounds
/// to exactly 10 000 000 m has been moved to the north by the caller, so a
/// northern northing is clamped to bands N–X: the band letter and the
/// northing's hemisphere never disagree, and the decoder relies on that to
/// pick the false northing.
fn band_letter(lat: f64, north: bool) -> char {
    // The caller passes a UTM-band latitude, at or above -80, so the index is
    // never negative and band C's own edge never steps below it.
    let mut idx = ((lat + 80.0) / 8.0).floor() as usize;
    if lat < -80.0 + 8.0 * idx as f64 {
        idx -= 1;
    }
    let idx = if north {
        // Band X runs to 84°, so 80°–84° (index 20) folds into it.
        idx.clamp(BAND_EQUATOR, BAND.len() - 1)
    } else {
        idx
    };
    BAND.as_bytes()[idx] as char
}

/// Index of band N, the first band north of the equator.
const BAND_EQUATOR: usize = 10;

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

/// The canonical reference of the square of `digits` digits that contains a
/// WGS-84 coordinate: UTM-based between 80°S and 84°N, UPS-based beyond.
fn encode_text(coord: Coordinate, digits: u32) -> Result<String> {
    if (-80.0..84.0).contains(&coord.lat()) {
        encode_utm(coord, digits)
    } else {
        encode_ups(coord, digits)
    }
}

fn encode_utm(coord: Coordinate, digits: u32) -> Result<String> {
    let utm = Utm::try_from_coordinate(coord)?;
    // One decision fixes both the hemisphere the northing is measured in and
    // the band letter. A southern northing a few ulps below the false
    // northing rounds to exactly 10 000 000 m: that is the equator, which
    // MGRS writes as northing 0 in band N. Left in the south, it would emit
    // row A with zero digits in band M, a square no southern northing reaches.
    let (north, northing) = match utm.hemisphere() {
        UtmHemisphere::North => (true, utm.northing_m()),
        // A southern latitude never projects above the false northing, so
        // this is exactly it, and the equator's northern northing is 0.
        UtmHemisphere::South if utm.northing_m() >= FALSE_NORTHING_SOUTH => (true, 0.0),
        UtmHemisphere::South => (false, utm.northing_m()),
    };
    // On the central meridian's side the point lies on (#198).
    let easting = clip::encoder_easting(coord, &utm);
    let set = ((utm.zone() - 1) % 3) as usize;
    let col = UTM_COLS[set].as_bytes()[(easting / 100_000.0) as usize - 1] as char;
    let row_off = if utm.zone() % 2 == 0 { 5 } else { 0 };
    let row = UTM_ROW.as_bytes()[((northing / 100_000.0) as usize + row_off) % 20] as char;
    Ok(format!(
        "{:02}{}{}{}{}",
        utm.zone(),
        band_letter(coord.lat(), north),
        col,
        row,
        digit_pair(easting, northing, digits),
    ))
}

fn encode_ups(coord: Coordinate, digits: u32) -> Result<String> {
    let ups = Ups::try_from_coordinate(coord)?;
    let north = ups.hemisphere() == UtmHemisphere::North;
    let east = ups.easting_m() >= 2_000_000.0;
    let zl = match (north, east) {
        (true, false) => 'Y',
        (true, true) => 'Z',
        (false, false) => 'A',
        (false, true) => 'B',
    };
    let (col_tbl, col_off) = ups_column_table(zl);
    let col = col_tbl.as_bytes()[(ups.easting_m() / 100_000.0) as usize - col_off] as char;
    let row_off = if north { 13 } else { 8 };
    let row = UPS_ROW.as_bytes()[(ups.northing_m() / 100_000.0) as usize - row_off] as char;
    Ok(format!(
        "{}{}{}{}",
        zl,
        col,
        row,
        digit_pair(ups.easting_m(), ups.northing_m(), digits)
    ))
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

/// What a reference names, before [`clip::settle`] moves a truncated square's
/// center into its grid-zone designator.
struct Decoded {
    /// The nominal square's center.
    center: Center,
    /// The precision the digit count implies.
    precision: MgrsPrecision,
    /// The extent of the grid-zone designator.
    gzd: Gzd,
}

/// Parse, validate, and decode an MGRS string to its nominal square.
fn decode(input: &str) -> Result<Decoded> {
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

fn decode_utm(s: &str, bad: impl Fn() -> Error + Copy) -> Result<Decoded> {
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
            return Ok(Decoded {
                center: Center::Utm(utm),
                precision,
                gzd: Gzd::Utm {
                    zone,
                    south: lo,
                    north: hi,
                    span: zone_span(zone, (lo + hi) / 2.0),
                },
            });
        }
    }
    Err(bad())
}

fn decode_ups(s: &str, bad: impl Fn() -> Error + Copy) -> Result<Decoded> {
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
    Ok(Decoded {
        center: Center::Ups(Ups::new(hemisphere, easting, northing)?),
        precision,
        gzd: Gzd::Ups { north },
    })
}

#[cfg(test)]
mod tests;
