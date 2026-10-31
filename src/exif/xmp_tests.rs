//! The XMP string forms: agreement with the IFD path and their own syntax.

use std::time::{Duration, SystemTime};

use super::tests::{U, text, whole, xmp_base};
use super::{ExifGps, XmpGps, from_exif, from_xmp};
use crate::{Error, ParseErrorKind};

fn xmp_error(gps: &XmpGps) -> Error {
    from_xmp(gps).unwrap_err()
}

/// The same values by either path give the same result, apart from `raw`.
#[test]
fn xmp_forms_match_the_ifd_path() {
    let ifd = ExifGps {
        latitude: Some([whole(39), U(461_234, 10_000), whole(0)]),
        latitude_ref: text("S"),
        longitude: Some([whole(116), whole(24), U(1205, 100)]),
        longitude_ref: text("W"),
        altitude: Some(U(27704, 249)),
        altitude_ref: Some(3),
        map_datum: text("TOKYO"),
        h_positioning_error: Some(U(9, 2)),
        dop: Some(U(13054, 1000)),
        date_stamp: text("2024:02:29"),
        time_stamp: Some([whole(12), whole(34), U(1205, 100)]),
        img_direction: Some(U(40183, 1728)),
        img_direction_ref: text("M"),
        speed: Some(U(87, 100)),
        speed_ref: text("N"),
        track: Some(U(719, 2)),
        track_ref: text("T"),
    };
    let xmp = XmpGps {
        latitude: text("39,46.1234S"),
        longitude: text("116,24,12.05W"),
        altitude: text("27704/249"),
        altitude_ref: text("3"),
        map_datum: text("TOKYO"),
        h_positioning_error: text("4.5"),
        dop: text("13054/1000"),
        time_stamp: text("2024-02-29T12:34:12.05Z"),
        img_direction: text("40183/1728"),
        img_direction_ref: text("M"),
        speed: text("87/100"),
        speed_ref: text("N"),
        track: text("719/2"),
        track_ref: text("T"),
    };
    let mut from_ifd = from_exif(&ifd).unwrap();
    let mut from_text = from_xmp(&xmp).unwrap();
    from_ifd.fix.source.as_mut().unwrap().raw.clear();
    from_text.fix.source.as_mut().unwrap().raw.clear();
    assert_eq!(from_ifd, from_text);
}

#[test]
fn xmp_rejects_what_the_ifd_path_rejects() {
    for (field, gps) in [
        (
            "latitude",
            XmpGps {
                latitude: text("10,60.0N"),
                ..xmp_base()
            },
        ),
        (
            "latitude",
            XmpGps {
                latitude: text("10,0,60N"),
                ..xmp_base()
            },
        ),
        (
            "latitude_ref",
            XmpGps {
                latitude: text("10,0.0E"),
                ..xmp_base()
            },
        ),
        (
            "longitude_ref",
            XmpGps {
                longitude: text("20,0.0N"),
                ..xmp_base()
            },
        ),
        (
            "altitude",
            XmpGps {
                altitude: text("1/0"),
                ..xmp_base()
            },
        ),
        (
            "dop",
            XmpGps {
                dop: text("0/0"),
                ..xmp_base()
            },
        ),
    ] {
        match xmp_error(&gps) {
            Error::InvalidValue { field: got, .. } => assert_eq!(got, field, "{gps:?}"),
            other => panic!("{gps:?}: {other:?}"),
        }
    }
    let bad_ref = XmpGps {
        altitude: text("1"),
        altitude_ref: text("up"),
        ..xmp_base()
    };
    assert!(matches!(
        xmp_error(&bad_ref),
        Error::InvalidValue {
            field: "altitude_ref",
            ..
        }
    ));
    let ref_four = XmpGps {
        altitude: text("1"),
        altitude_ref: text("4"),
        ..xmp_base()
    };
    assert!(matches!(
        xmp_error(&ref_four),
        Error::InvalidValue {
            field: "altitude_ref",
            ..
        }
    ));
    let far = XmpGps {
        latitude: text("91,0.0N"),
        ..xmp_base()
    };
    assert!(matches!(xmp_error(&far), Error::OutOfRange { .. }));
    let bad_time = XmpGps {
        time_stamp: text("2024-02-29T24:00:00Z"),
        ..xmp_base()
    };
    assert!(matches!(
        xmp_error(&bad_time),
        Error::InvalidValue {
            field: "time_stamp",
            ..
        }
    ));
}

#[test]
fn xmp_text_that_does_not_read_is_a_syntax_error() {
    let bad_coordinates = [
        "",
        "N",
        "10N",
        "10,0,0,0N",
        ",0.0N",
        "10,N",
        "NaN,0.0N",
        "inf,0.0N",
        "-10,0.0N",
        "+10,0.0N",
        "1e1,0.0N",
        "1.2.3,0.0N",
        "10,..N",
        "١٠,0.0N",
    ];
    for bad in bad_coordinates {
        let gps = XmpGps {
            latitude: text(bad),
            ..xmp_base()
        };
        assert!(
            matches!(
                xmp_error(&gps),
                Error::Parse {
                    kind: ParseErrorKind::Syntax,
                    ..
                }
            ),
            "{bad}"
        );
    }
    let huge = "9".repeat(400);
    for bad in [
        "NaN",
        "inf",
        "-1",
        "1e3",
        "x/1",
        "1/x",
        "1/2/3",
        "/2",
        "1/",
        huge.as_str(),
    ] {
        let gps = XmpGps {
            altitude: text(bad),
            ..xmp_base()
        };
        assert!(
            matches!(
                xmp_error(&gps),
                Error::Parse {
                    kind: ParseErrorKind::Syntax,
                    ..
                }
            ),
            "{bad}"
        );
    }
}

#[test]
fn xmp_blank_numbers_read_as_absent() {
    let gps = XmpGps {
        altitude: text(""),
        altitude_ref: text(" "),
        dop: text("\0"),
        time_stamp: text(""),
        ..xmp_base()
    };
    let capture = from_xmp(&gps).unwrap();
    assert_eq!(capture.fix.coord.height(), None);
    assert_eq!(capture.dop, None);
    assert_eq!(capture.fix.timestamp, None);
    assert_eq!(
        capture.fix.source.unwrap().notes.len(),
        1,
        "only the datum note"
    );
}

#[test]
fn xmp_time_stamps_honour_their_zone_and_need_every_part() {
    let at = |secs, millis| {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs) + Duration::from_millis(millis))
    };
    for (stamp, expected) in [
        ("2024-02-29T12:34:56Z", at(1_709_210_096, 0)),
        ("2024-02-29T12:34:56z", at(1_709_210_096, 0)),
        ("2024-02-29T12:34:56.25Z", at(1_709_210_096, 250)),
        ("2024-02-29T20:34:56+08:00", at(1_709_210_096, 0)),
        ("2024-02-29T07:04:56-05:30", at(1_709_210_096, 0)),
        ("2024-02-29T12:34:56+00:00", at(1_709_210_096, 0)),
    ] {
        let gps = XmpGps {
            time_stamp: text(stamp),
            ..xmp_base()
        };
        assert_eq!(from_xmp(&gps).unwrap().fix.timestamp, expected, "{stamp}");
    }
    for stamp in [
        "2024-02-29",
        "2024-02-29T12:34:56",
        "2024-02-29T12:34Z",
        "2024-02-29T12:34:56:00Z",
        "2024-02-29T12:34:xxZ",
        "2024-02-29T12:34:56+0800",
        "2024-02-29T12:34:56+8:00",
        "2024-02-29T12:34:56+08:0",
        "2024-02-29T12:34:56+aa:00",
        "2024-02-29T12:34:56+00:aa",
        "nonsense",
    ] {
        let gps = XmpGps {
            time_stamp: text(stamp),
            ..xmp_base()
        };
        let fix = from_xmp(&gps).unwrap().fix;
        assert_eq!(fix.timestamp, None, "{stamp}");
        assert!(
            fix.source
                .unwrap()
                .notes
                .iter()
                .any(|n| n.contains("partial")),
            "{stamp}"
        );
    }
    let bad_date = XmpGps {
        time_stamp: text("2024-13-01T00:00:00Z"),
        ..xmp_base()
    };
    let fix = from_xmp(&bad_date).unwrap().fix;
    assert_eq!(fix.timestamp, None);
    assert!(
        fix.source
            .unwrap()
            .notes
            .iter()
            .any(|n| n.contains("unreadable"))
    );
}
