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
use crate::fix::{Accuracy, Confidence, Fix, RawSource};

/// Nominal GPS user-equivalent range error (meters): turns the dimensionless
/// HDOP into a rough horizontal-accuracy estimate (`HDOP × UERE`).
const NOMINAL_UERE_M: f64 = 5.0;

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
    match &kind[kind.len().saturating_sub(3)..] {
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
    if value.len() < deg_digits {
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
    Ddm {
        degrees,
        minutes,
        hemisphere,
    }
    .try_to_dd()
    .map(|value| value.0)
    .map_err(|error| Error::Parse(format!("bad NMEA coordinate: {error}")))
}

/// Latitude (2 degree digits) + longitude (3 degree digits) → coordinate.
fn coordinate(lat: &str, ns: &str, lon: &str, ew: &str, raw: &str) -> Result<Coordinate> {
    let lat = parse_ddm(lat, ns, 2, raw)?;
    let lon = parse_ddm(lon, ew, 3, raw)?;
    let coord = Coordinate::wgs84(lat, lon);
    coord
        .validate()
        .map_err(|error| Error::Parse(format!("bad NMEA coordinate: {error}")))?;
    Ok(coord)
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
            axis_order: None, // NMEA fixes the lat-first order
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
        coord = coord.with_height(Height::Orthometric(alt));
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
    coord
        .validate()
        .map_err(|error| Error::Parse(format!("bad NMEA coordinate: {error}")))?;
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
    use super::*;
    use crate::test_support::assert_close;

    #[test]
    fn gga_full_metadata() {
        let fix =
            from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")
                .unwrap();
        assert_close(fix.coord.lat, 48.0 + 7.038 / 60.0, 1e-9);
        assert_close(fix.coord.lon, 11.0 + 31.0 / 60.0, 1e-9);
        assert_eq!(fix.coord.height, Some(Height::Orthometric(545.4)));
        assert_close(
            fix.accuracy.unwrap().horizontal_m.unwrap(),
            0.9 * NOMINAL_UERE_M,
            1e-9,
        );
        let src = fix.source.as_ref().unwrap();
        assert_eq!(src.axis_order, None); // NMEA is lat-first
        assert!(src.notes.iter().any(|n| n.contains("geoid separation")));
    }

    #[test]
    fn rmc_southern_western_void() {
        let fix =
            from_nmea_sentence("$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07")
                .unwrap();
        assert_close(fix.coord.lat, -(33.0 + 40.0 / 60.0), 1e-9);
        assert_close(fix.coord.lon, -(70.0 + 30.0 / 60.0), 1e-9);
        assert!(fix.accuracy.is_none());
        assert!(fix.source.unwrap().notes.iter().any(|n| n.contains("void")));
    }

    #[test]
    fn gll_west() {
        let fix = from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1D").unwrap();
        assert_close(fix.coord.lat, 49.0 + 16.45 / 60.0, 1e-9);
        assert_close(fix.coord.lon, -(123.0 + 11.12 / 60.0), 1e-9);
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
}
