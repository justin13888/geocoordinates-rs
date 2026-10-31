//! GPS date and time stamps, speed units, and bearings.

use std::time::{Duration, SystemTime};

use super::tests::{U, base, text, whole};
use super::{ExifGps, ExifRational, NorthBearing, NorthReference, from_exif};
use crate::Error;

fn stamped(date: &str, time: [ExifRational; 3]) -> Option<SystemTime> {
    let gps = ExifGps {
        date_stamp: text(date),
        time_stamp: Some(time),
        ..base()
    };
    from_exif(&gps).unwrap().fix.timestamp
}

#[test]
fn timestamps_are_utc_and_keep_fractional_seconds() {
    let at = |secs| Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs));
    let t = [whole(12), whole(34), whole(56)];
    assert_eq!(stamped("2024:02:29", t), at(1_709_210_096));
    assert_eq!(stamped("2024-02-29", t), at(1_709_210_096));
    assert_eq!(stamped(" 2024:02:29\0", t), at(1_709_210_096));
    assert_eq!(stamped("1970:01:01", [whole(0); 3]), at(0));
    assert_eq!(
        stamped("2000:03:01", [whole(0), whole(0), U(1205, 100)]),
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(951_868_812) + Duration::from_millis(50))
    );
    // A fractional hour and minute count too.
    assert_eq!(
        stamped("1970:01:01", [U(3, 2), U(1, 2), whole(0)]),
        at(5430)
    );
    assert_eq!(
        stamped("1969:12:31", [whole(23), whole(59), U(59_500, 1000)]),
        Some(SystemTime::UNIX_EPOCH - Duration::from_millis(500))
    );
    assert_eq!(
        stamped("1900:03:01", [whole(0); 3]),
        Some(SystemTime::UNIX_EPOCH - Duration::from_secs(2_203_891_200))
    );
}

#[test]
fn partial_or_unreadable_stamps_give_no_timestamp() {
    let t = [whole(1), whole(2), whole(3)];
    for date in [
        "2014:09:259",
        "2014:13:01",
        "2014:00:01",
        "2014:02:29",
        "1900:02:29",
        "2014:04:31",
        "2014:01:32",
        "2014:01:00",
        "14:01:01",
        "2014:01",
        "2014:01:01:01",
        "abcd:ef:gh",
    ] {
        assert_eq!(stamped(date, t), None, "{date}");
        let gps = ExifGps {
            date_stamp: text(date),
            time_stamp: Some(t),
            ..base()
        };
        let capture = from_exif(&gps).unwrap();
        let source = capture.fix.source.unwrap();
        assert!(
            source.notes.iter().any(|n| n.contains("unreadable")),
            "{date}"
        );
    }
    assert!(stamped("2024:12:31", t).is_some());
    assert!(stamped("2000:02:29", t).is_some());
    for date in ["", "\0\0", "    "] {
        let gps = ExifGps {
            date_stamp: text(date),
            time_stamp: Some(t),
            ..base()
        };
        let source = from_exif(&gps).unwrap().fix;
        assert_eq!(source.timestamp, None);
        assert_eq!(source.source.unwrap().notes.len(), 1, "only the datum note");
    }
    let date_only = ExifGps {
        date_stamp: text("2024:02:29"),
        ..base()
    };
    assert_eq!(from_exif(&date_only).unwrap().fix.timestamp, None);
    let time_only = ExifGps {
        time_stamp: Some(t),
        ..base()
    };
    assert_eq!(from_exif(&time_only).unwrap().fix.timestamp, None);
}

#[test]
fn speed_units_convert_to_meters_per_second() {
    for (unit, speed, expected) in [
        ("K", 36, 10.0),
        ("k", 36, 10.0),
        ("M", 100, 44.704),
        ("N", 10, 18520.0 / 3600.0),
        ("n\0", 10, 18520.0 / 3600.0),
    ] {
        let gps = ExifGps {
            speed: Some(whole(speed)),
            speed_ref: text(unit),
            ..base()
        };
        let actual = from_exif(&gps).unwrap().speed_mps.unwrap();
        assert!((actual - expected).abs() < 1e-12, "{unit}: {actual}");
    }
}

#[test]
fn speed_without_a_unit_is_dropped_with_a_note() {
    for unit in [None, text(""), text("\0")] {
        let gps = ExifGps {
            speed: Some(whole(5)),
            speed_ref: unit,
            ..base()
        };
        let capture = from_exif(&gps).unwrap();
        assert_eq!(capture.speed_mps, None);
        assert!(
            capture
                .fix
                .source
                .unwrap()
                .notes
                .iter()
                .any(|n| n.contains("GPSSpeedRef"))
        );
    }
    assert_eq!(from_exif(&base()).unwrap().speed_mps, None);
    let unit_alone = ExifGps {
        speed_ref: text("bogus"),
        ..base()
    };
    assert_eq!(from_exif(&unit_alone).unwrap().speed_mps, None);
}

#[test]
fn bearings_keep_their_reference() {
    for (letter, reference) in [
        (text("T"), Some(NorthReference::True)),
        (text("t"), Some(NorthReference::True)),
        (text("M"), Some(NorthReference::Magnetic)),
        (text("m\0"), Some(NorthReference::Magnetic)),
        (None, None),
        (text(""), None),
    ] {
        let gps = ExifGps {
            img_direction: Some(U(719, 2)),
            img_direction_ref: letter.clone(),
            track: Some(whole(0)),
            track_ref: letter,
            ..base()
        };
        let capture = from_exif(&gps).unwrap();
        assert_eq!(
            capture.direction,
            Some(NorthBearing::new(359.5, reference).unwrap())
        );
        assert_eq!(
            capture.track,
            Some(NorthBearing::new(0.0, reference).unwrap())
        );
    }
}

#[test]
fn north_bearing_new_rejects_outside_the_circle() {
    for bad in [-0.5, 360.0, f64::NAN, f64::INFINITY] {
        assert!(
            matches!(
                NorthBearing::new(bad, None),
                Err(Error::InvalidValue {
                    field: "degrees",
                    ..
                })
            ),
            "{bad}"
        );
    }
    assert_eq!(NorthBearing::new(0.0, None).unwrap().degrees(), 0.0);
}
