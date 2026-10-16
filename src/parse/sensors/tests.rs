//! NMEA 0183 sentence parsing: fields, checksums, timestamps and the
//! metadata each sentence maps onto its [`Fix`].

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
    // HDOP × 5 m is a one-sigma estimate.
    let accuracy = fix.accuracy.unwrap();
    assert_eq!(accuracy.kind(), crate::fix::AccuracyKind::OneSigma);
    assert_close(accuracy.horizontal().unwrap().meters(), 0.9 * 5.0, 1e-9);
    let src = fix.source.as_ref().unwrap();
    assert_eq!(src.axis_order, Some(AxisOrder::LatLon)); // NMEA lists lat first
    // The geoid separation is typed, not a note: h = H + N.
    assert_eq!(src.geoid_separation, Some(Length::from_meters(46.9)));
    assert!(src.notes.is_empty(), "{:?}", src.notes);
}

/// A geoid separation in a unit other than meters is rejected, as the
/// altitude is; an absent one leaves the field `None`, and a negative one
/// (geoid below the ellipsoid) is kept.
#[test]
fn gga_geoid_separation_is_typed_and_needs_meters() {
    let feet = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,F,,";
    let message = parse_error(feet);
    assert!(message.contains("geoid separation unit"), "{message}");
    let absent = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,,,,";
    let fix = from_nmea_sentence(absent).unwrap();
    assert_eq!(fix.source.unwrap().geoid_separation, None);
    let below = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,-34.2,M,,";
    let fix = from_nmea_sentence(below).unwrap();
    assert_eq!(
        fix.source.unwrap().geoid_separation,
        Some(Length::from_meters(-34.2))
    );
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
    let fix = from_nmea_sentence("$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10")
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
        from_nmea_sentence("$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,,A").is_ok()
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
    let bare = from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,,,,,,").unwrap();
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
    assert_eq!(fix.accuracy.unwrap().horizontal(), Some(Length::ZERO));
    let gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,-0.9,545.4,M,46.9,M,,*6A";
    assert!(matches!(from_nmea_sentence(gga), Err(Error::Parse(m)) if m.contains("HDOP")));
}

/// A finite HDOP whose `× 5 m` accuracy overflows is rejected for the
/// overflow, not misreported as a non-finite or negative HDOP.
#[test]
fn gga_hdop_overflowing_accuracy_names_the_scaled_range() {
    let data = "GPGGA,123519,4807.038,N,01131.000,E,1,08,1e308,545.4,M,46.9,M,,";
    let cs = data.bytes().fold(0u8, |acc, b| acc ^ b);
    let r = from_nmea_sentence(&format!("${data}*{cs:02X}"));
    assert!(
        matches!(&r, Err(Error::Parse(m)) if m.contains("out of range") && !m.contains("finite")),
        "{r:?}"
    );
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
