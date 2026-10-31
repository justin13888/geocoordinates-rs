//! Calls every EXIF / XMP FFI export with an asserted value, including the failures
//! each one documents, and pins that every field crosses the boundary.

use geocoordinates_ffi::*;

fn whole(numerator: u32) -> ExifRational {
    ExifRational::Unsigned {
        numerator,
        denominator: 1,
    }
}

fn dms(degrees: u32, minutes: u32, seconds: u32) -> Option<ExifDms> {
    Some(ExifDms {
        degrees: whole(degrees),
        minutes: whole(minutes),
        seconds: whole(seconds),
    })
}

fn text(value: &str) -> Option<String> {
    Some(value.to_string())
}

const STRICT: ExifOptions = ExifOptions {
    null_island: NullIslandPolicy::Reject,
};
const LENIENT: ExifOptions = ExifOptions {
    null_island: NullIslandPolicy::Accept,
};

fn full_ifd() -> ExifGps {
    ExifGps {
        latitude: dms(35, 30, 0),
        latitude_ref: text("S"),
        longitude: dms(139, 45, 0),
        longitude_ref: text("E"),
        altitude: Some(ExifRational::Signed {
            numerator: 25,
            denominator: 2,
        }),
        altitude_ref: Some(3),
        map_datum: text("TOKYO"),
        h_positioning_error: Some(whole(9)),
        dop: Some(whole(2)),
        date_stamp: text("2024:02:29"),
        time_stamp: Some(ExifTime {
            hours: whole(12),
            minutes: whole(34),
            seconds: whole(56),
        }),
        img_direction: Some(whole(90)),
        img_direction_ref: text("M"),
        speed: Some(whole(36)),
        speed_ref: text("K"),
        track: Some(whole(180)),
        track_ref: text("T"),
    }
}

fn full_xmp() -> XmpGps {
    XmpGps {
        latitude: text("35,30.0S"),
        longitude: text("139,45,0E"),
        altitude: text("25/2"),
        altitude_ref: text("3"),
        map_datum: text("TOKYO"),
        h_positioning_error: text("9"),
        dop: text("2"),
        time_stamp: text("2024-02-29T12:34:56Z"),
        img_direction: text("90/1"),
        img_direction_ref: text("M"),
        speed: text("36"),
        speed_ref: text("K"),
        track: text("180"),
        track_ref: text("T"),
    }
}

fn assert_full(capture: &CaptureGeo) {
    let coord = &capture.fix.coord;
    assert_eq!((coord.lat, coord.lon), (-35.5, 139.75));
    assert_eq!(coord.crs, Crs::Tokyo);
    assert_eq!(coord.height, Some(Height::Ellipsoidal { meters: -12.5 }));
    let accuracy = capture.fix.accuracy.as_ref().unwrap();
    assert_eq!(accuracy.kind, AccuracyKind::Bound);
    assert_eq!(accuracy.horizontal_m, Some(9.0));
    let expected = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_709_210_096);
    assert_eq!(capture.fix.timestamp, Some(expected));
    assert_eq!(
        capture.direction,
        Some(NorthBearing {
            bearing_deg: 90.0,
            reference: Some(NorthReference::Magnetic),
        })
    );
    assert_eq!(capture.speed_mps, Some(10.0));
    assert_eq!(
        capture.track,
        Some(NorthBearing {
            bearing_deg: 180.0,
            reference: Some(NorthReference::True),
        })
    );
    assert_eq!(capture.dop, Some(2.0));
}

#[test]
fn parse_exif_carries_every_tag() {
    assert_full(&parse_exif(full_ifd()).unwrap());
}

#[test]
fn parse_xmp_carries_every_property() {
    assert_full(&parse_xmp(full_xmp()).unwrap());
}

#[test]
fn parse_exif_with_applies_the_null_island_policy() {
    let null_island = ExifGps {
        latitude: dms(0, 0, 0),
        latitude_ref: text("N"),
        longitude: dms(0, 0, 0),
        longitude_ref: text("E"),
        ..ExifGps::default()
    };
    assert!(parse_exif_with(null_island.clone(), LENIENT).is_ok());
    assert!(matches!(
        parse_exif_with(null_island, STRICT),
        Err(GeoError::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
}

#[test]
fn parse_xmp_with_applies_the_null_island_policy() {
    let null_island = XmpGps {
        latitude: text("0,0.0N"),
        longitude: text("0,0.0E"),
        ..XmpGps::default()
    };
    assert!(parse_xmp_with(null_island.clone(), LENIENT).is_ok());
    assert!(matches!(
        parse_xmp_with(null_island, STRICT),
        Err(GeoError::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
}

#[test]
fn errors_cross_with_their_tag_and_kind() {
    let zero = ExifGps {
        altitude: Some(ExifRational::Unsigned {
            numerator: 1,
            denominator: 0,
        }),
        ..full_ifd()
    };
    assert!(matches!(
        parse_exif(zero),
        Err(GeoError::InvalidValue { field, .. }) if field == "altitude"
    ));
    assert!(matches!(
        parse_exif(ExifGps::default()),
        Err(GeoError::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
    let garbled = XmpGps {
        latitude: text("nowhere"),
        ..full_xmp()
    };
    assert!(matches!(
        parse_xmp(garbled),
        Err(GeoError::Parse {
            kind: ParseErrorKind::Syntax,
            ..
        })
    ));
}

#[test]
fn china_ambiguity_crosses_on_the_source() {
    let beijing = XmpGps {
        latitude: text("39,54.0N"),
        longitude: text("116,24.0E"),
        ..XmpGps::default()
    };
    let source = parse_xmp(beijing).unwrap().fix.source.unwrap();
    assert_eq!(source.datum_ambiguity, Some(DatumAmbiguity::PossiblyGcj02));
}
