//! Sensor/device ingestion: NMEA 0183 sentences.
//!
//! EXIF/XMP image GPS metadata is **out of scope** — it is handled by a
//! separate library that consumes this crate's primitives (the angle
//! conversions for GPS rationals, [`Fix`] with its
//! [`RawSource`], and
//! [`DatumAmbiguity::PossiblyGcj02`](crate::fix::DatumAmbiguity::PossiblyGcj02)
//! for China-EXIF datum ambiguity).
//!
//! The three position sentences (`GGA`, `RMC`, `GLL`) are parsed directly — the
//! grammar is small and parsing it here keeps the dependency surface (and the
//! wasm build) clean.

use std::time::{Duration, SystemTime};

use crate::angle::{Ddm, Hemisphere};
use crate::coord::{Coordinate, Height};
use crate::error::{Error, Result};
use crate::fix::{Accuracy, AxisOrder, Confidence, Fix, RawSource};
use crate::units::Length;

use super::NOMINAL_UERE_M;

/// Parse a single NMEA 0183 sentence (GGA/RMC/GLL) into a [`Fix`].
///
/// NMEA uses degrees-decimal-minutes (DDM) and carries fix quality, HDOP,
/// altitude, and geoid separation — mapped onto [`Fix`] metadata. The optional
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
/// Returns [`crate::Error::Parse`] when:
/// - the sentence type is not `GGA`, `RMC`, or `GLL`, or the `*HH` checksum is
///   malformed or does not match;
/// - the sentence reports no fix: `RMC`/`GLL` status `V` (void), `RMC`/`GLL`
///   mode indicator `N` (data not valid), or `GGA` fix quality `0`;
/// - a coordinate field is not `ddmm[.m…]` (latitude) or `dddmm[.m…]`
///   (longitude) — exactly two minute digits before the decimal point — or its
///   hemisphere letter is missing or wrong for the axis;
/// - the `GGA` altitude is present but its unit field is not `M` (meters);
/// - a numeric field (altitude, geoid separation, HDOP) is malformed or
///   non-finite, or HDOP is negative.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::parse::sensors::from_nmea_sentence;
///
/// let gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47";
/// let fix = from_nmea_sentence(gga)?;
/// assert!((fix.coord.lat() - 48.1173).abs() < 1e-9);
/// assert!((fix.coord.lon() - 11.516_666_666).abs() < 1e-9);
/// let horizontal = fix.accuracy.and_then(|a| a.horizontal).map(|h| h.meters());
/// assert_eq!(horizontal, Some(4.5)); // HDOP 0.9 × 5 m
///
/// // Status `V`: the receiver has no fix, so there is no position.
/// let void = "$GPRMC,123519,V,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W";
/// assert!(matches!(from_nmea_sentence(void), Err(Error::Parse(_))));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "nmea")]
pub fn from_nmea_sentence(sentence: &str) -> Result<Fix> {
    let body = sentence.trim().strip_prefix('$').unwrap_or(sentence.trim());
    let (data, checksum) = match body.split_once('*') {
        Some((d, c)) => (d, Some(c.trim())),
        None => (body, None),
    };
    if let Some(cs) = checksum {
        if cs.len() != 2 || !cs.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Parse(format!("bad NMEA checksum field: {sentence}")));
        }
        let expected = u8::from_str_radix(cs, 16)
            .map_err(|_| Error::Parse(format!("bad NMEA checksum field: {sentence}")))?;
        let actual = data.bytes().fold(0u8, |acc, b| acc ^ b);
        if actual != expected {
            return Err(Error::Parse(format!("NMEA checksum mismatch: {sentence}")));
        }
    }

    let fields: Vec<&str> = data.split(',').collect();
    let kind = *fields.first().unwrap_or(&"");
    // Drop the 2-letter talker id (GP/GN/GL/…); the type is the trailing 3.
    match kind.get(kind.len().saturating_sub(3)..).unwrap_or("") {
        "GGA" => parse_gga(&fields, sentence),
        "RMC" => parse_rmc(&fields, sentence),
        "GLL" => parse_gll(&fields, sentence),
        _ => Err(Error::Parse(format!(
            "unsupported NMEA sentence type: {kind}"
        ))),
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
    let malformed = || Error::Parse(format!("malformed NMEA coordinate: {raw}"));
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
            return Err(Error::Parse(format!(
                "bad NMEA axis/hemisphere '{other:?}': {raw}"
            )));
        }
    };
    Ddm::new(degrees, minutes, hemisphere)
        .map(Ddm::to_degrees)
        .map_err(|error| Error::Parse(format!("bad NMEA coordinate: {error}")))
}

/// Latitude (2 degree digits) + longitude (3 degree digits) → coordinate.
fn coordinate(lat: &str, ns: &str, lon: &str, ew: &str, raw: &str) -> Result<Coordinate> {
    let lat = parse_ddm(lat, ns, 2, raw)?;
    let lon = parse_ddm(lon, ew, 3, raw)?;
    Coordinate::wgs84(lat, lon)
        .map_err(|error| Error::Parse(format!("bad NMEA coordinate: {error}")))
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
        .map_err(|_| Error::Parse(format!("bad NMEA {name}: {raw}")))?;
    if !parsed.is_finite() {
        return Err(Error::Parse(format!("non-finite NMEA {name}: {raw}")));
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
    Fix {
        coord,
        accuracy,
        timestamp,
        source: Some(RawSource {
            raw: raw.to_string(),
            confidence: Confidence::new(1.0),
            // NMEA identifies each axis by its field position, latitude first;
            // the hemisphere letter only signs it.
            axis_order: Some(AxisOrder::LatLon),
            datum_ambiguity: None,
            notes,
        }),
    }
}

/// `GGA`: position fix with HDOP, altitude (MSL/orthometric), and geoid separation.
fn parse_gga(f: &[&str], raw: &str) -> Result<Fix> {
    if field(f, 6).trim() == "0" {
        return Err(Error::Parse(format!(
            "NMEA GGA reports no fix (quality 0): {raw}"
        )));
    }
    let mut coord = coordinate(field(f, 2), field(f, 3), field(f, 4), field(f, 5), raw)?;
    let mut notes = Vec::new();
    if let Some(alt) = optional_number(field(f, 9), "altitude", raw)? {
        if field(f, 10).trim() != "M" {
            return Err(Error::Parse(format!(
                "NMEA GGA altitude unit is not M (meters): {raw}"
            )));
        }
        coord = coord
            .with_height(Height::Orthometric(alt))
            .map_err(|error| Error::Parse(format!("bad NMEA coordinate: {error}")))?;
    }
    if let Some(sep) = optional_number(field(f, 11), "geoid separation", raw)? {
        notes.push(format!("geoid separation {sep} m"));
    }
    let accuracy = optional_number(field(f, 8), "HDOP", raw)?
        .map(|hdop| {
            if hdop < 0.0 {
                return Err(Error::Parse(format!("negative NMEA HDOP: {raw}")));
            }
            Ok(Accuracy {
                horizontal: Some(Length::from_meters(hdop * NOMINAL_UERE_M)),
                vertical: None,
            })
        })
        .transpose()?;
    Ok(base_fix(coord, accuracy, None, notes, raw))
}

/// Reject a sentence whose status field is `V` (void) or whose NMEA 2.3 mode
/// indicator is `N` (data not valid). Either field may be absent.
fn require_valid(kind: &str, status: &str, mode: &str, raw: &str) -> Result<()> {
    if status.trim() == "V" {
        return Err(Error::Parse(format!(
            "NMEA {kind} reports a void (non-valid) fix: {raw}"
        )));
    }
    if mode.trim() == "N" {
        return Err(Error::Parse(format!(
            "NMEA {kind} reports mode N (data not valid): {raw}"
        )));
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
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::test_support::assert_close;

    #[test]
    fn gga_full_metadata() {
        let fix =
            from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")
                .unwrap();
        assert_close(fix.coord.lat(), 48.0 + 7.038 / 60.0, 1e-9);
        assert_close(fix.coord.lon(), 11.0 + 31.0 / 60.0, 1e-9);
        assert_eq!(fix.coord.height(), Some(Height::Orthometric(545.4)));
        assert_close(
            fix.accuracy.unwrap().horizontal.unwrap().meters(),
            0.9 * NOMINAL_UERE_M,
            1e-9,
        );
        let src = fix.source.as_ref().unwrap();
        assert_eq!(src.axis_order, Some(AxisOrder::LatLon)); // NMEA lists lat first
        assert!(src.notes.iter().any(|n| n.contains("geoid separation")));
    }

    /// Seconds since the Unix epoch as a `SystemTime`.
    fn epoch(secs: u64, nanos: u32) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::new(secs, nanos)
    }

    /// The parse error's message, failing the test on success or another variant.
    fn parse_error(sentence: &str) -> String {
        match from_nmea_sentence(sentence) {
            Err(Error::Parse(message)) => message,
            other => panic!("{sentence:?}: expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn rmc_southern_western_with_timestamp() {
        let fix =
            from_nmea_sentence("$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10")
                .unwrap();
        assert_close(fix.coord.lat(), -(33.0 + 40.0 / 60.0), 1e-9);
        assert_close(fix.coord.lon(), -(70.0 + 30.0 / 60.0), 1e-9);
        assert!(fix.accuracy.is_none());
        // 1994-03-23T12:35:19Z: a two-digit year of 80 or more is 19xx.
        assert_eq!(fix.timestamp, Some(epoch(764_426_119, 0)));
        assert!(fix.source.unwrap().notes.is_empty());
    }

    #[test]
    fn void_and_no_fix_sentences_are_rejected() {
        // RMC status `V`: the receiver says the position is not valid.
        let rmc = parse_error("$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07");
        assert!(rmc.contains("void"), "{rmc}");
        // GLL status `V`.
        let gll = parse_error("$GPGLL,4916.45,N,12311.12,W,225444,V");
        assert!(gll.contains("void"), "{gll}");
        // GGA fix quality 0: no fix, typically with empty position fields.
        let gga = parse_error("$GPGGA,123519,4807.038,N,01131.000,E,0,08,0.9,545.4,M,46.9,M,,");
        assert!(gga.contains("quality 0"), "{gga}");
        let empty = parse_error("$GPGGA,123519,,,,,0,00,,,M,,M,,");
        assert!(empty.contains("quality 0"), "{empty}");
        // NMEA 2.3 mode indicator `N` (data not valid) on RMC and GLL.
        let rmc_mode = parse_error("$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,,N");
        assert!(rmc_mode.contains("mode N"), "{rmc_mode}");
        let gll_mode = parse_error("$GPGLL,4916.45,N,12311.12,W,225444,A,N");
        assert!(gll_mode.contains("mode N"), "{gll_mode}");
    }

    #[test]
    fn valid_status_and_mode_are_accepted() {
        // An autonomous (`A`) or differential (`D`) mode and other GGA qualities parse.
        assert!(
            from_nmea_sentence("$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,,A")
                .is_ok()
        );
        assert!(from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,D").is_ok());
        // Pre-2.0 GLL carries no status field at all.
        assert!(from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W").is_ok());
        for quality in ["1", "2", "6", ""] {
            let sentence =
                format!("$GPGGA,123519,4807.038,N,01131.000,E,{quality},08,0.9,545.4,M,46.9,M,,");
            assert!(from_nmea_sentence(&sentence).is_ok(), "{sentence}");
        }
    }

    #[test]
    fn minutes_need_two_integer_digits() {
        // `807.038` would split as 80° 7.038′; NMEA always writes `mm`.
        let lat = parse_error("$GPGLL,807.038,N,12311.12,W,225444,A");
        assert!(lat.contains("malformed NMEA coordinate"), "{lat}");
        let lon = parse_error("$GPGLL,4916.45,N,1231.12,W,225444,A");
        assert!(lon.contains("malformed NMEA coordinate"), "{lon}");
        // Three minute digits, a signed degree, a non-digit minute, an exponent,
        // a non-digit fraction, and an exponent inside the fraction (which an
        // `f64` parse alone would accept) are malformed too.
        for value in [
            "49160.45", "+916.45", "491x.45", "4916e1", "4916.4x", "4916.4e1",
        ] {
            let sentence = format!("$GPGLL,{value},N,12311.12,W,225444,A");
            let message = parse_error(&sentence);
            assert!(
                message.contains("malformed NMEA coordinate"),
                "{sentence}: {message}"
            );
        }
        // Whole minutes with no fraction, and a bare trailing point, still parse.
        let whole = from_nmea_sentence("$GPGLL,4916,N,12311.,W,225444,A").unwrap();
        assert_close(whole.coord.lat(), 49.0 + 16.0 / 60.0, 1e-9);
        assert_close(whole.coord.lon(), -(123.0 + 11.0 / 60.0), 1e-9);
    }

    #[test]
    fn gga_altitude_must_be_in_meters() {
        let feet = parse_error("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,F,46.9,M,,");
        assert!(feet.contains("altitude unit"), "{feet}");
        let missing = parse_error("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,,46.9,M,,");
        assert!(missing.contains("altitude unit"), "{missing}");
        // No altitude means no unit to check.
        let bare =
            from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,,,,,,").unwrap();
        assert_eq!(bare.coord.height(), None);
    }

    #[test]
    fn rmc_timestamp_edges() {
        let rmc = |time: &str, date: &str| {
            from_nmea_sentence(&format!(
                "$GPRMC,{time},A,3340.000,S,07030.000,W,000.0,000.0,{date},,"
            ))
            .unwrap()
        };
        // Fractional seconds; the 1980 and 2079 ends of the two-digit-year window.
        assert_eq!(
            rmc("000000.25", "010180").timestamp,
            Some(epoch(315_532_800, 250_000_000))
        );
        assert_eq!(
            rmc("235959", "311279").timestamp,
            Some(epoch(3_471_292_799, 0))
        );
        assert_eq!(
            rmc("000000.", "010180").timestamp,
            Some(epoch(315_532_800, 0))
        );
        // Nanosecond digits past the ninth are truncated.
        assert_eq!(
            rmc("000000.1234567899", "010180").timestamp,
            Some(epoch(315_532_800, 123_456_789))
        );
        // A leap day, and a leap second (UTC 23:59:60) are real times.
        assert_eq!(
            rmc("000000", "290224").timestamp,
            Some(epoch(1_709_164_800, 0))
        );
        assert_eq!(
            rmc("235960", "311279").timestamp,
            Some(epoch(3_471_292_800, 0))
        );
        // Either half missing leaves the timestamp empty without a note.
        for (time, date) in [("", "230394"), ("123519", "")] {
            let fix = rmc(time, date);
            assert_eq!(fix.timestamp, None);
            assert!(fix.source.unwrap().notes.is_empty());
        }
        // An unreadable date/time keeps the position but notes why no time is set.
        for (time, date) in [
            ("123519", "290223"), // not a leap year
            ("123519", "310494"), // April has 30 days
            ("123519", "000194"),
            ("123519", "011394"),
            ("123519", "010094"),
            ("123519", "32039"),
            ("123519", "2303945"),
            ("243519", "230394"),
            ("126019", "230394"),
            ("123561", "230394"),
            ("12351", "230394"),
            ("1235190", "230394"),
            ("123519.x", "230394"),
            ("12a519", "230394"),
            ("123519", "2303a4"),
        ] {
            let fix = rmc(time, date);
            assert_eq!(fix.timestamp, None, "{time} {date}");
            let notes = fix.source.unwrap().notes;
            assert!(
                notes.iter().any(|n| n.contains("timestamp")),
                "{time} {date}"
            );
        }
    }

    #[test]
    fn days_in_each_month() {
        let lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for (month, days) in (1..=12).zip(lengths) {
            assert_eq!(days_in_month(1994, month), days, "month {month}");
        }
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_since_epoch(1970, 1, 1), 0);
        assert_eq!(days_since_epoch(1973, 1, 1), 365 * 3 + 1);
    }

    #[test]
    fn gll_west() {
        let fix = from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1D").unwrap();
        assert_close(fix.coord.lat(), 49.0 + 16.45 / 60.0, 1e-9);
        assert_close(fix.coord.lon(), -(123.0 + 11.12 / 60.0), 1e-9);
        // Status "A" is valid, so the fix carries no note.
        assert!(fix.source.unwrap().notes.is_empty());
    }

    #[test]
    fn checksum_is_verified() {
        // Flip a digit so the stored checksum no longer matches.
        assert!(from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E").is_err());
        assert!(from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1").is_err());
        // …but a sentence with no `*HH` suffix is accepted.
        assert!(from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A").is_ok());
    }

    /// The checksum is exactly two hex digits, even where a shorter or signed
    /// field would parse to the right value (this sentence's XOR is 0x03).
    #[test]
    fn checksum_must_be_two_hex_digits() {
        let data = "$GNGLL,4916.45,N,12311.12,W,225444,A,";
        assert!(from_nmea_sentence(&format!("{data}*03")).is_ok());
        for cs in ["3", "+3"] {
            let r = from_nmea_sentence(&format!("{data}*{cs}"));
            assert!(matches!(&r, Err(Error::Parse(m)) if m.contains("checksum field")));
        }
    }

    /// An HDOP of zero is a (perfect) dilution, not a negative one.
    #[test]
    fn gga_hdop_zero_is_accepted_and_negative_rejected() {
        let gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.0,545.4,M,46.9,M,,*4E";
        let fix = from_nmea_sentence(gga).unwrap();
        assert_eq!(fix.accuracy.unwrap().horizontal, Some(Length::ZERO));
        let gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,-0.9,545.4,M,46.9,M,,*6A";
        assert!(matches!(from_nmea_sentence(gga), Err(Error::Parse(m)) if m.contains("HDOP")));
    }

    #[test]
    fn unsupported_and_malformed_are_errors() {
        assert!(from_nmea_sentence("$GPVTG,054.7,T,034.4,M,005.5,N,010.2,K*48").is_err());
        assert!(from_nmea_sentence("$GPGGA,,,N,,E,,,,,,,,,").is_err()); // empty coordinate
        assert!(from_nmea_sentence("$GPGLL,4916.45,X,12311.12,W,225444,A").is_err()); // bad hemi
        assert!(from_nmea_sentence("$GPGLL,4960.00,N,12311.12,W,225444,A").is_err());
        assert!(from_nmea_sentence("$GPGGA,0,4807.0,N,01131.0,E,1,8,NaN,0,M,0,M").is_err());
    }

    #[test]
    fn non_ascii_sentence_type_is_parse_error_not_panic() {
        assert!(matches!(from_nmea_sentence("$éAB"), Err(Error::Parse(_))));
    }

    #[test]
    fn non_ascii_coordinate_field_is_parse_error_not_panic() {
        assert!(matches!(
            from_nmea_sentence("$GPGLL,1é,N,12311.12,W,225444,A"),
            Err(Error::Parse(_))
        ));
        // Longitude has 3 degree digits, so the split point (byte 3) falls
        // inside 'é' here, unlike the latitude case above (2 degree digits).
        assert!(matches!(
            from_nmea_sentence("$GPGLL,4916.45,N,12é11.12,W,225444,A"),
            Err(Error::Parse(_))
        ));
    }

    #[test]
    fn non_ascii_and_truncated_inputs_never_panic() {
        let probes = [
            "$éAB",
            "$GPGLL,1é,N,12311.12,W,225444,A",
            "$GPGGA,é,N,é,E,,,,,,,,,",
            "$GPRMC,é",
            "$é",
            "é",
            "$",
            "",
            "$GPGLL,é,é,é,é,é,é",
            "$GPGGA,123519,48é7.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47",
            "$GPGLL,4916.45,N,12311.12,Wé,225444,A",
            "$GPGLL,4916.45,N,12é11.12,W,225444,A",
            "$GP\u{1F30D}A",
            "$GPGGA,,,é,,é,,,,,,,,,",
            "$G",
            "$éé",
            "$GPGLLé",
            "$GPGLL,é",
            "$GPGGAé,1,N,1,E",
            "€€€",
            "$*é",
            "$GPGLL,4916.45é,N,12311.12,W,225444,A",
        ];
        // Every probe is rejected as a parse error; a panic or any other
        // variant fails the test.
        for p in probes {
            let r = from_nmea_sentence(p);
            assert!(matches!(r, Err(Error::Parse(_))), "{p:?}: {r:?}");
        }
    }
}
