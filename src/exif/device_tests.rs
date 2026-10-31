//! GPS IFD values transcribed from real device files.
//!
//! Provenance: the public test corpus `drewnoakes/metadata-extractor-images`, commit
//! `7b56b649039c57903bcd5cbe9846ab81eb518ec4`, directory `jpg/`; each case names its
//! file. The values are the rationals as stored, read with Pillow's `Exif.get_ifd`.
//!
//! The corpus holds no Chinese-market file taken inside China: the Oppo R7 Plus was
//! used in Norway, so it pins the unflagged case, and `tests.rs` covers the
//! [`PossiblyGcj02`](crate::fix::DatumAmbiguity::PossiblyGcj02) flag with constructed
//! values.

use std::time::{Duration, SystemTime};

use super::{CaptureGeo, ExifGps, ExifRational, NorthReference, from_exif};
use crate::Height;
use crate::{Error, ParseErrorKind};

fn dms(values: [(u32, u32); 3]) -> Option<[ExifRational; 3]> {
    Some(values.map(ExifRational::from))
}

fn rational(numerator: u32, denominator: u32) -> Option<ExifRational> {
    Some(ExifRational::Unsigned(numerator, denominator))
}

fn text(value: &str) -> Option<String> {
    Some(value.to_string())
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} vs {expected}");
}

fn utc(seconds: u64, millis: u64) -> Option<SystemTime> {
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds) + Duration::from_millis(millis))
}

fn notes(capture: &CaptureGeo) -> &[String] {
    &capture.fix.source.as_ref().unwrap().notes
}

/// `jpg/Apple iPhone 6.jpg`. Its stored date is `2014:09:259`, which is not a date.
#[test]
fn iphone_6_with_a_malformed_date() {
    let gps = ExifGps {
        latitude_ref: text("N"),
        latitude: dms([(51, 1), (45, 1), (4467, 100)]),
        longitude_ref: text("W"),
        longitude: dms([(0, 1), (19, 1), (441, 100)]),
        altitude_ref: Some(0),
        altitude: rational(27704, 249),
        time_stamp: dms([(13, 1), (5, 1), (1205, 100)]),
        speed_ref: text("K"),
        speed: rational(87, 100),
        img_direction_ref: text("T"),
        img_direction: rational(40183, 1728),
        date_stamp: text("2014:09:259"),
        ..ExifGps::default()
    };
    let capture = from_exif(&gps).unwrap();
    close(capture.fix.coord.lat(), 51.0 + 45.0 / 60.0 + 44.67 / 3600.0);
    close(capture.fix.coord.lon(), -(19.0 / 60.0 + 4.41 / 3600.0));
    assert_eq!(
        capture.fix.coord.height(),
        Some(Height::Orthometric(27704.0 / 249.0))
    );
    assert_eq!(capture.fix.timestamp, None);
    assert!(notes(&capture).iter().any(|n| n.contains("2014:09:259")));
    close(capture.speed_mps.unwrap(), 0.87 / 3.6);
    let direction = capture.direction.unwrap();
    assert_eq!(direction.degrees(), 40183.0 / 1728.0);
    assert_eq!(direction.reference(), Some(NorthReference::True));
}

/// `jpg/Apple iPhone 6 plus.jpg`: below sea level, minutes as the fraction.
#[test]
fn iphone_6_plus_below_sea_level() {
    let gps = ExifGps {
        latitude_ref: text("N"),
        latitude: dms([(51, 1), (289_797, 10_000), (0, 1)]),
        longitude_ref: text("W"),
        longitude: dms([(3, 1), (111_202, 10_000), (0, 1)]),
        altitude_ref: Some(1),
        altitude: rational(38750, 233),
        time_stamp: dms([(13, 1), (32, 1), (37, 1)]),
        speed_ref: text("K"),
        speed: rational(0, 1),
        date_stamp: text("2014:10:31"),
        ..ExifGps::default()
    };
    let capture = from_exif(&gps).unwrap();
    close(capture.fix.coord.lat(), 51.0 + 28.9797 / 60.0);
    assert_eq!(
        capture.fix.coord.height(),
        Some(Height::Orthometric(-38750.0 / 233.0))
    );
    assert_eq!(capture.fix.timestamp, utc(1_414_762_357, 0));
    assert_eq!(capture.speed_mps, Some(0.0));
}

/// `jpg/Apple iPhone 5.jpg`: a time with no date gives no timestamp.
#[test]
fn iphone_5_time_without_date() {
    let gps = ExifGps {
        latitude_ref: text("N"),
        latitude: dms([(47, 1), (3763, 100), (0, 1)]),
        longitude_ref: text("W"),
        longitude: dms([(122, 1), (2024, 100), (0, 1)]),
        altitude_ref: Some(0),
        altitude: rational(13595, 3412),
        time_stamp: dms([(23, 1), (11, 1), (2563, 100)]),
        img_direction_ref: text("T"),
        img_direction: rational(46616, 499),
        ..ExifGps::default()
    };
    let capture = from_exif(&gps).unwrap();
    close(capture.fix.coord.lon(), -(122.0 + 20.24 / 60.0));
    assert_eq!(capture.fix.timestamp, None);
    assert_eq!(capture.direction.unwrap().degrees(), 46616.0 / 499.0);
}

/// `jpg/DJI Phantom 4 (1).jpg`: position and altitude only, no date or time.
#[test]
fn dji_phantom_4_has_no_stamp() {
    let gps = ExifGps {
        latitude_ref: text("N"),
        latitude: dms([(32, 1), (28, 1), (429_577, 10_000)]),
        longitude_ref: text("W"),
        longitude: dms([(90, 1), (15, 1), (360_022, 10_000)]),
        altitude_ref: Some(0),
        altitude: rational(109_686, 1000),
        ..ExifGps::default()
    };
    let capture = from_exif(&gps).unwrap();
    close(
        capture.fix.coord.lat(),
        32.0 + 28.0 / 60.0 + 42.9577 / 3600.0,
    );
    close(
        capture.fix.coord.lon(),
        -(90.0 + 15.0 / 60.0 + 36.0022 / 3600.0),
    );
    assert_eq!(capture.fix.timestamp, None);
    assert_eq!(capture.direction, None);
}

/// `jpg/Google Pixel 2.jpg`: ref 1 with a zero magnitude, and a DOP alone.
#[test]
fn pixel_2_dop_invents_no_accuracy() {
    let gps = ExifGps {
        latitude_ref: text("N"),
        latitude: dms([(43, 1), (4, 1), (2450, 100)]),
        longitude_ref: text("W"),
        longitude: dms([(89, 1), (23, 1), (161, 100)]),
        altitude_ref: Some(1),
        altitude: rational(0, 1000),
        time_stamp: dms([(19, 1), (12, 1), (51, 1)]),
        dop: rational(13054, 1000),
        date_stamp: text("2019:01:19"),
        ..ExifGps::default()
    };
    let capture = from_exif(&gps).unwrap();
    assert_eq!(capture.fix.accuracy, None);
    assert_eq!(capture.dop, Some(13.054));
    assert_eq!(capture.fix.timestamp, utc(1_547_925_171, 0));
    let Some(Height::Orthometric(meters)) = capture.fix.coord.height() else {
        panic!("expected an orthometric height");
    };
    assert!(meters == 0.0 && meters.is_sign_positive());
}

/// `jpg/Oppo R7 Plus.jpg`: a Chinese-market maker, but a position outside China.
#[test]
fn oppo_r7_plus_outside_china_is_not_flagged() {
    let gps = ExifGps {
        latitude_ref: text("N"),
        latitude: dms([(60, 1), (16, 1), (28692, 10_000)]),
        longitude_ref: text("E"),
        longitude: dms([(8, 1), (46, 1), (160_768, 10_000)]),
        altitude_ref: Some(0),
        altitude: rational(0, 1000),
        time_stamp: dms([(15, 1), (52, 1), (22, 1)]),
        date_stamp: text("2018:07:16"),
        ..ExifGps::default()
    };
    let capture = from_exif(&gps).unwrap();
    assert_eq!(capture.fix.source.as_ref().unwrap().datum_ambiguity, None);
    assert_eq!(capture.fix.timestamp, utc(1_531_756_342, 0));
}

/// `jpg/OnePlus8.jpg`: a GPS block with no fix, written as NUL refs and `0/0`.
#[test]
fn oneplus_8_placeholder_block_is_no_fix() {
    let gps = ExifGps {
        latitude_ref: text("\0"),
        latitude: dms([(0, 0), (0, 0), (0, 0)]),
        longitude_ref: text("\0"),
        longitude: dms([(0, 0), (0, 0), (0, 0)]),
        altitude_ref: Some(0),
        altitude: rational(0, 0),
        time_stamp: dms([(0, 0), (0, 0), (0, 0)]),
        date_stamp: text("\0\0\0\0\0\0\0\0\0\0"),
        ..ExifGps::default()
    };
    assert!(matches!(
        from_exif(&gps),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
}
