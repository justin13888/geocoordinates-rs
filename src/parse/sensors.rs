//! Sensor/device ingestion: NMEA 0183 sentences.
//!
//! Reading EXIF/XMP image GPS metadata out of a JPEG, TIFF or HEIF file is **out
//! of scope** — it is handled by a separate library that consumes this crate's
//! primitives (the angle conversions for GPS rationals, [`Fix`] with its
//! [`RawSource`], and
//! [`DatumAmbiguity::PossiblyGcj02`](crate::fix::DatumAmbiguity::PossiblyGcj02)
//! for China-EXIF datum ambiguity). Turning tag values already read into a [`Fix`]
//! is the planned `exif` feature, not yet shipped.
//!
//! The three position sentences (`GGA`, `RMC`, `GLL`) are parsed directly — the
//! grammar is small and parsing it here keeps the dependency surface (and the
//! wasm build) clean.

use std::time::{Duration, SystemTime};

use crate::angle::{Ddm, Hemisphere};
use crate::coord::{Coordinate, Height};
use crate::error::{Error, ParseErrorKind, Result};
use crate::fix::{Accuracy, AxisOrder, Confidence, Fix, RawSource};
use crate::units::Length;

use super::{hdop_accuracy, syntax};

/// Parse a single NMEA 0183 sentence (GGA/RMC/GLL) into a [`Fix`].
///
/// NMEA uses degrees-decimal-minutes (DDM) and carries fix quality, HDOP,
/// altitude, and geoid separation — mapped onto [`Fix`] metadata: `GGA`'s HDOP
/// becomes an [`AccuracyKind::OneSigma`](crate::fix::AccuracyKind::OneSigma)
/// horizontal accuracy of `HDOP × 5 m` (a nominal GPS user-equivalent range
/// error), and its geoid separation N becomes
/// [`RawSource::geoid_separation`], so the ellipsoidal height is the
/// orthometric altitude plus N. The optional
/// `*HH` checksum (XOR of the bytes between `$` and `*`) is verified when
/// present.
///
/// Only a sentence that reports a fix yields one: a void status, a "data not
/// valid" mode, or GGA fix quality 0 is an error, never a position.
///
/// `RMC` fills [`Fix::timestamp`] from its UTC time (`hhmmss[.s…]`) and date
/// (`ddmmyy`) fields. The two-digit year is read in the GPS-era window
/// 1980–2079 (`80`–`99` → 19xx, `00`–`79` → 20xx). A missing date or time
/// leaves the timestamp `None`; an unreadable one does too, with a note on
/// [`RawSource::notes`] saying so. `GGA` and `GLL` carry a time of day but no
/// date, so their timestamp is always `None`.
///
/// # Errors
/// Returns [`crate::Error::Parse`], whose [`ParseErrorKind`] is:
/// - [`Checksum`](ParseErrorKind::Checksum) when the `*HH` checksum is
///   malformed or does not match;
/// - [`Unsupported`](ParseErrorKind::Unsupported) when the sentence type is
///   not `GGA`, `RMC`, or `GLL`;
/// - [`NoFix`](ParseErrorKind::NoFix) when the sentence reports no fix:
///   `RMC`/`GLL` status `V` (void), `RMC`/`GLL` mode indicator `N` (data not
///   valid), or `GGA` fix quality `0`;
/// - [`Syntax`](ParseErrorKind::Syntax) when a coordinate field is not
///   `ddmm[.m…]` (latitude) or `dddmm[.m…]` (longitude) — exactly two minute
///   digits before the decimal point — or its hemisphere letter is missing or
///   wrong for the axis (or the position it reads is out of range); when the
///   `GGA` altitude or geoid separation is present but its unit field is not
///   `M` (meters); or when a numeric field (altitude, geoid separation, HDOP)
///   is malformed or non-finite, or HDOP is negative.
///
/// ```
/// use geocoordinates::{Error, ParseErrorKind};
/// use geocoordinates::parse::sensors::from_nmea_sentence;
///
/// let gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47";
/// let fix = from_nmea_sentence(gga)?;
/// assert!((fix.coord.lat() - 48.1173).abs() < 1e-9);
/// assert!((fix.coord.lon() - 11.516_666_666).abs() < 1e-9);
/// let horizontal = fix.accuracy.and_then(|a| a.horizontal()).map(|h| h.meters());
/// assert_eq!(horizontal, Some(4.5)); // HDOP 0.9 × 5 m
/// // h = H + N: the orthometric altitude plus the reported geoid separation.
/// let n = fix.source.and_then(|s| s.geoid_separation).map(|n| n.meters());
/// assert_eq!(n, Some(46.9));
///
/// // Status `V`: the receiver has no fix, so there is no position.
/// let void = "$GPRMC,123519,V,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W";
/// assert!(matches!(
///     from_nmea_sentence(void),
///     Err(Error::Parse { kind: ParseErrorKind::NoFix, .. })
/// ));
/// // A corrupted byte fails the checksum instead.
/// let corrupt = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*48";
/// assert!(matches!(
///     from_nmea_sentence(corrupt),
///     Err(Error::Parse { kind: ParseErrorKind::Checksum, .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "nmea")]
#[cfg_attr(docsrs, doc(cfg(feature = "nmea")))]
pub fn from_nmea_sentence(sentence: &str) -> Result<Fix> {
    let body = sentence.trim().strip_prefix('$').unwrap_or(sentence.trim());
    let (data, checksum) = match body.split_once('*') {
        Some((d, c)) => (d, Some(c.trim())),
        None => (body, None),
    };
    if let Some(cs) = checksum {
        let bad_field = || {
            Error::parse(
                ParseErrorKind::Checksum,
                format!("bad NMEA checksum field: {sentence}"),
            )
        };
        if cs.len() != 2 || !cs.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad_field());
        }
        let expected = u8::from_str_radix(cs, 16).map_err(|_| bad_field())?;
        let actual = data.bytes().fold(0u8, |acc, b| acc ^ b);
        if actual != expected {
            return Err(Error::parse(
                ParseErrorKind::Checksum,
                format!("NMEA checksum mismatch: {sentence}"),
            ));
        }
    }

    let fields: Vec<&str> = data.split(',').collect();
    let kind = *fields.first().unwrap_or(&"");
    // Drop the 2-letter talker id (GP/GN/GL/…); the type is the trailing 3.
    match kind.get(kind.len().saturating_sub(3)..).unwrap_or("") {
        "GGA" => parse_gga(&fields, sentence),
        "RMC" => parse_rmc(&fields, sentence),
        "GLL" => parse_gll(&fields, sentence),
        _ => Err(Error::parse(
            ParseErrorKind::Unsupported,
            format!("unsupported NMEA sentence type: {kind}"),
        )),
    }
}

/// Whether every byte of `s` is an ASCII digit (vacuously true when empty).
fn all_digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

/// Parse a DDM field (`ddmm.mmmm`, `deg_digits` degree digits) plus its
/// hemisphere letter into signed decimal degrees.
///
/// NMEA always writes the minutes with two integer digits, so `807.038` is
/// malformed rather than 80° 7.038′; signs and exponents are rejected too.
fn parse_ddm(value: &str, hemi: &str, deg_digits: usize, raw: &str) -> Result<f64> {
    let malformed = || syntax(format!("malformed NMEA coordinate: {raw}"));
    let value = value.trim();
    if !value.is_char_boundary(deg_digits) {
        return Err(malformed());
    }
    let (deg_str, min_str) = value.split_at(deg_digits);
    let (min_whole, min_fraction) = min_str.split_once('.').unwrap_or((min_str, ""));
    if !all_digits(deg_str)
        || min_whole.len() != 2
        || !all_digits(min_whole)
        || !all_digits(min_fraction)
    {
        return Err(malformed());
    }
    // The grammar check above guarantees both parses succeed; any failure is
    // still the same malformed-coordinate error rather than a panic.
    let degrees: u16 = deg_str.parse().map_err(|_| malformed())?;
    let minutes: f64 = min_str.parse().map_err(|_| malformed())?;
    let hemisphere = match (deg_digits, hemi.trim()) {
        (2, "N") => Hemisphere::North,
        (2, "S") => Hemisphere::South,
        (3, "E") => Hemisphere::East,
        (3, "W") => Hemisphere::West,
        other => {
            return Err(syntax(format!(
                "bad NMEA axis/hemisphere '{other:?}': {raw}"
            )));
        }
    };
    Ddm::new(degrees, minutes, hemisphere)
        .map(Ddm::to_degrees)
        .map_err(|error| syntax(format!("bad NMEA coordinate: {error}")))
}

/// Latitude (2 degree digits) + longitude (3 degree digits) → coordinate.
fn coordinate(lat: &str, ns: &str, lon: &str, ew: &str, raw: &str) -> Result<Coordinate> {
    let lat = parse_ddm(lat, ns, 2, raw)?;
    let lon = parse_ddm(lon, ew, 3, raw)?;
    Coordinate::wgs84(lat, lon).map_err(|error| syntax(format!("bad NMEA coordinate: {error}")))
}

/// The `i`-th comma-separated field, or `""` when absent.
fn field<'a>(fields: &[&'a str], i: usize) -> &'a str {
    fields.get(i).copied().unwrap_or("")
}

fn optional_number(value: &str, name: &str, raw: &str) -> Result<Option<f64>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let parsed = value
        .parse::<f64>()
        .map_err(|_| syntax(format!("bad NMEA {name}: {raw}")))?;
    if !parsed.is_finite() {
        return Err(syntax(format!("non-finite NMEA {name}: {raw}")));
    }
    Ok(Some(parsed))
}

/// Assemble a [`Fix`] with the shared NMEA provenance (lat-first, confident).
fn base_fix(
    coord: Coordinate,
    accuracy: Option<Accuracy>,
    timestamp: Option<SystemTime>,
    notes: Vec<String>,
    raw: &str,
) -> Fix {
    let mut fix = Fix::from_coord(coord);
    fix.accuracy = accuracy;
    fix.timestamp = timestamp;
    let mut source = RawSource::new(raw, Confidence::CERTAIN);
    // NMEA identifies each axis by its field position, latitude first; the
    // hemisphere letter only signs it.
    source.axis_order = Some(AxisOrder::LatLon);
    source.notes = notes;
    fix.source = Some(source);
    fix
}

/// `GGA`: position fix with HDOP, altitude (MSL/orthometric), and geoid separation.
fn parse_gga(f: &[&str], raw: &str) -> Result<Fix> {
    if field(f, 6).trim() == "0" {
        return Err(Error::parse(
            ParseErrorKind::NoFix,
            format!("NMEA GGA reports no fix (quality 0): {raw}"),
        ));
    }
    let mut coord = coordinate(field(f, 2), field(f, 3), field(f, 4), field(f, 5), raw)?;
    if let Some(alt) = optional_number(field(f, 9), "altitude", raw)? {
        if field(f, 10).trim() != "M" {
            return Err(syntax(format!(
                "NMEA GGA altitude unit is not M (meters): {raw}"
            )));
        }
        coord = coord
            .with_height(Height::Orthometric(alt))
            .map_err(|error| syntax(format!("bad NMEA coordinate: {error}")))?;
    }
    let geoid_separation = optional_number(field(f, 11), "geoid separation", raw)?
        .map(|n| {
            if field(f, 12).trim() != "M" {
                return Err(syntax(format!(
                    "NMEA GGA geoid separation unit is not M (meters): {raw}"
                )));
            }
            Ok(Length::from_meters(n))
        })
        .transpose()?;
    let accuracy = optional_number(field(f, 8), "HDOP", raw)?
        .map(|hdop| {
            if hdop < 0.0 {
                return Err(syntax(format!("negative NMEA HDOP: {raw}")));
            }
            hdop_accuracy(hdop)
        })
        .transpose()?;
    let mut fix = base_fix(coord, accuracy, None, Vec::new(), raw);
    if let Some(source) = fix.source.as_mut() {
        source.geoid_separation = geoid_separation;
    }
    Ok(fix)
}

/// Reject a sentence whose status field is `V` (void) or whose NMEA 2.3 mode
/// indicator is `N` (data not valid). Either field may be absent.
fn require_valid(kind: &str, status: &str, mode: &str, raw: &str) -> Result<()> {
    if status.trim() == "V" {
        return Err(Error::parse(
            ParseErrorKind::NoFix,
            format!("NMEA {kind} reports a void (non-valid) fix: {raw}"),
        ));
    }
    if mode.trim() == "N" {
        return Err(Error::parse(
            ParseErrorKind::NoFix,
            format!("NMEA {kind} reports mode N (data not valid): {raw}"),
        ));
    }
    Ok(())
}

/// `RMC`: recommended minimum — position, an A/V validity flag, and the UTC
/// date and time.
fn parse_rmc(f: &[&str], raw: &str) -> Result<Fix> {
    require_valid("RMC", field(f, 2), field(f, 12), raw)?;
    let coord = coordinate(field(f, 3), field(f, 4), field(f, 5), field(f, 6), raw)?;
    let mut notes = Vec::new();
    let (time, date) = (field(f, 1).trim(), field(f, 9).trim());
    let timestamp = if time.is_empty() || date.is_empty() {
        None
    } else {
        let parsed = utc_timestamp(time, date);
        if parsed.is_none() {
            notes.push(format!(
                "RMC date '{date}' / time '{time}' unreadable; timestamp left empty"
            ));
        }
        parsed
    };
    Ok(base_fix(coord, None, timestamp, notes, raw))
}

/// `GLL`: geographic position, latitude/longitude.
fn parse_gll(f: &[&str], raw: &str) -> Result<Fix> {
    require_valid("GLL", field(f, 6), field(f, 7), raw)?;
    let coord = coordinate(field(f, 1), field(f, 2), field(f, 3), field(f, 4), raw)?;
    Ok(base_fix(coord, None, None, Vec::new(), raw))
}

/// The two-digit decimal number at byte `at` of an all-ASCII-digit string.
fn two_digits(s: &str, at: usize) -> u32 {
    s.as_bytes()[at..at + 2]
        .iter()
        .fold(0, |acc, b| acc * 10 + u32::from(b - b'0'))
}

/// Days in `month` (1–12) of `year`. Every year in the 1980–2079 window that
/// is divisible by 4 is a leap year (2000 included), so no century rule is
/// needed.
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to `year-month-day` (proleptic Gregorian, `year ≥ 1970`).
fn days_since_epoch(year: u32, month: u32, day: u32) -> u64 {
    let leap_days = (1970..year).filter(|y| y % 4 == 0).count();
    let year_days = u64::from(year - 1970) * 365 + leap_days as u64;
    let month_days: u32 = (1..month).map(|m| days_in_month(year, m)).sum();
    year_days + u64::from(month_days) + u64::from(day - 1)
}

/// An RMC UTC time (`hhmmss` with an optional `.fraction`) and date (`ddmmyy`)
/// as a [`SystemTime`], or `None` when either is malformed or out of range.
/// A leap second (`ss = 60`) is accepted and lands on the next second.
fn utc_timestamp(time: &str, date: &str) -> Option<SystemTime> {
    let (hms, fraction) = time.split_once('.').unwrap_or((time, ""));
    if hms.len() != 6 || date.len() != 6 || !all_digits(hms) || !all_digits(date) {
        return None;
    }
    if !all_digits(fraction) {
        return None;
    }
    let (hour, minute, second) = (two_digits(hms, 0), two_digits(hms, 2), two_digits(hms, 4));
    let (day, month, yy) = (
        two_digits(date, 0),
        two_digits(date, 2),
        two_digits(date, 4),
    );
    let year = if yy >= 80 { 1900 + yy } else { 2000 + yy };
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    // Nanoseconds from the first nine fraction digits, right-padded with zeros.
    let nanos = fraction
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(9)
        .fold(0, |acc, b| acc * 10 + u32::from(b - b'0'));
    let seconds =
        days_since_epoch(year, month, day) * 86_400 + u64::from(hour * 3600 + minute * 60 + second);
    Some(SystemTime::UNIX_EPOCH + Duration::new(seconds, nanos))
}

#[cfg(test)]
mod tests;
