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

use crate::angle::{Ddm, Hemisphere};
use crate::coord::{Coordinate, Height};
use crate::error::{Error, Result};
use crate::fix::{Accuracy, AxisOrder, Confidence, Fix, RawSource};

use super::NOMINAL_UERE_M;

/// Parse a single NMEA 0183 sentence (GGA/RMC/GLL) into a [`Fix`].
///
/// NMEA uses degrees-decimal-minutes (DDM) and carries fix quality, HDOP,
/// altitude, and geoid separation — mapped onto [`Fix`] metadata. The optional
/// `*HH` checksum (XOR of the bytes between `$` and `*`) is verified when
/// present.
///
/// # Errors
/// Returns [`crate::Error::Parse`] on an unrecognized/invalid sentence or a
/// checksum mismatch.
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

/// Parse a DDM field (`ddmm.mmmm`, `deg_digits` degree digits) plus its
/// hemisphere letter into signed decimal degrees.
fn parse_ddm(value: &str, hemi: &str, deg_digits: usize, raw: &str) -> Result<f64> {
    let value = value.trim();
    if !value.is_char_boundary(deg_digits) {
        return Err(Error::Parse(format!("malformed NMEA coordinate: {raw}")));
    }
    let (deg_str, min_str) = value.split_at(deg_digits);
    let degrees: u16 = deg_str
        .parse()
        .map_err(|_| Error::Parse(format!("bad NMEA degrees: {raw}")))?;
    let minutes: f64 = min_str
        .parse()
        .map_err(|_| Error::Parse(format!("bad NMEA minutes: {raw}")))?;
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
fn base_fix(coord: Coordinate, accuracy: Option<Accuracy>, notes: Vec<String>, raw: &str) -> Fix {
    Fix {
        coord,
        accuracy,
        timestamp: None,
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
    let mut coord = coordinate(field(f, 2), field(f, 3), field(f, 4), field(f, 5), raw)?;
    let mut notes = Vec::new();
    if let Some(alt) = optional_number(field(f, 9), "altitude", raw)? {
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
                horizontal_m: Some(hdop * NOMINAL_UERE_M),
                vertical_m: None,
            })
        })
        .transpose()?;
    Ok(base_fix(coord, accuracy, notes, raw))
}

/// `RMC`: recommended minimum — position plus an A/V validity flag.
fn parse_rmc(f: &[&str], raw: &str) -> Result<Fix> {
    let coord = coordinate(field(f, 3), field(f, 4), field(f, 5), field(f, 6), raw)?;
    let mut notes = Vec::new();
    if field(f, 2).trim() == "V" {
        notes.push("RMC reports a void (non-valid) fix".to_string());
    }
    Ok(base_fix(coord, None, notes, raw))
}

/// `GLL`: geographic position, latitude/longitude.
fn parse_gll(f: &[&str], raw: &str) -> Result<Fix> {
    let coord = coordinate(field(f, 1), field(f, 2), field(f, 3), field(f, 4), raw)?;
    let mut notes = Vec::new();
    if field(f, 6).trim() == "V" {
        notes.push("GLL reports a void (non-valid) fix".to_string());
    }
    Ok(base_fix(coord, None, notes, raw))
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
            fix.accuracy.unwrap().horizontal_m.unwrap(),
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
        // Three minute digits, a signed degree, and an exponent are malformed too.
        for value in ["49160.45", "+916.45", "4916e1", "4916.4x"] {
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
    fn gll_west() {
        let fix = from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1D").unwrap();
        assert_close(fix.coord.lat(), 49.0 + 16.45 / 60.0, 1e-9);
        assert_close(fix.coord.lon(), -(123.0 + 11.12 / 60.0), 1e-9);
        // Status "A" is valid, so no void note is attached.
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
