//! The rejection tables under each [`AuxiliaryTagPolicy`]: `Reject` fails on every
//! row, and `Drop` keeps the position past every auxiliary row.

use super::tests::{U, base, field_of, rejections, text, whole, xmp_base};
use super::xmp_tests::xmp_rejections;
use super::{
    AuxiliaryTagPolicy, CaptureGeo, ExifGps, ExifOptions, ExifRational, NullIslandPolicy, XmpGps,
};
use crate::{Error, ParseErrorKind};
use proptest::prelude::*;

const DROP: ExifOptions = ExifOptions::new().with_invalid_auxiliary(AuxiliaryTagPolicy::Drop);

/// The tag a field's drop note names; `None` for a position field.
fn auxiliary_tag(field: &str) -> Option<&'static str> {
    Some(match field {
        "altitude" | "altitude_ref" => "GPSAltitude",
        "h_positioning_error" => "GPSHPositioningError",
        "dop" => "GPSDOP",
        "time_stamp" => "GPSTimeStamp",
        "img_direction" | "img_direction_ref" => "GPSImgDirection",
        "speed" | "speed_ref" => "GPSSpeed",
        "track" | "track_ref" => "GPSTrack",
        _ => return None,
    })
}

/// The position is kept, the field's output is absent, and one note says why.
fn assert_dropped(capture: &CaptureGeo, field: &str, tag: &str) {
    let coord = capture.fix.coord;
    assert_eq!((coord.lat(), coord.lon()), (10.0, 20.0), "{field}");
    let absent = match field {
        "altitude" | "altitude_ref" => coord.height().is_none(),
        "h_positioning_error" => capture.fix.accuracy.is_none(),
        "dop" => capture.dop.is_none(),
        "time_stamp" => capture.fix.timestamp.is_none(),
        "img_direction" | "img_direction_ref" => capture.direction.is_none(),
        "speed" | "speed_ref" => capture.speed_mps.is_none(),
        "track" | "track_ref" => capture.track.is_none(),
        other => panic!("{other} is not auxiliary"),
    };
    assert!(absent, "{field} kept: {capture:?}");
    let notes = &capture.fix.source.as_ref().unwrap().notes;
    let dropped: Vec<_> = notes.iter().filter(|n| n.contains("dropped:")).collect();
    assert_eq!(dropped.len(), 1, "{field}: {notes:?}");
    assert!(dropped[0].starts_with(tag), "{field}: {dropped:?}");
    assert!(dropped[0].contains(field), "{field}: {dropped:?}");
}

#[test]
fn rejects_each_invalid_value_naming_its_tag() {
    for (field, gps) in &rejections() {
        assert_eq!(field_of(gps), *field, "{gps:?}");
    }
}

#[test]
fn drop_keeps_the_position_past_each_invalid_auxiliary_tag() {
    let mut auxiliary_rows = 0;
    for (field, gps) in &rejections() {
        let result = CaptureGeo::from_exif_with(gps, DROP);
        match auxiliary_tag(field) {
            Some(tag) => {
                assert_dropped(&result.unwrap(), field, tag);
                auxiliary_rows += 1;
            }
            None => assert!(
                matches!(result, Err(Error::InvalidValue { field: got, .. }) if got == *field),
                "{gps:?}"
            ),
        }
    }
    assert!(auxiliary_rows >= 15, "{auxiliary_rows}");
}

#[test]
fn xmp_drop_keeps_the_position_past_each_invalid_auxiliary_property() {
    let unreadable = [
        ("altitude", "x/1"),
        ("h_positioning_error", "NaN"),
        ("dop", "-1"),
        ("img_direction", "1e3"),
    ];
    let unreadable = unreadable.map(|(field, value)| {
        let mut gps = xmp_base();
        *match field {
            "altitude" => &mut gps.altitude,
            "h_positioning_error" => &mut gps.h_positioning_error,
            "dop" => &mut gps.dop,
            _ => &mut gps.img_direction,
        } = text(value);
        (field, gps)
    });
    let zone = XmpGps {
        time_stamp: text("2024-02-29T12:00:00+24:00"),
        ..xmp_base()
    };
    let mut auxiliary_rows = 0;
    let rows = xmp_rejections().into_iter().chain(unreadable);
    for (field, gps) in rows.chain([("time_stamp", zone)]) {
        let result = CaptureGeo::from_xmp_with(&gps, DROP);
        match auxiliary_tag(field) {
            Some(tag) => {
                assert!(CaptureGeo::from_xmp(&gps).is_err(), "{gps:?}");
                assert_dropped(&result.unwrap(), field, tag);
                auxiliary_rows += 1;
            }
            None => assert!(result.is_err(), "{gps:?}"),
        }
    }
    assert!(auxiliary_rows >= 13, "{auxiliary_rows}");
}

#[test]
fn drop_takes_a_value_and_its_ref_together() {
    let gps = ExifGps {
        altitude: Some(whole(100)),
        altitude_ref: Some(7),
        img_direction: Some(whole(90)),
        img_direction_ref: text("X"),
        speed: Some(whole(10)),
        speed_ref: text("Q"),
        track: Some(U(1, 0)),
        track_ref: text("T"),
        dop: Some(whole(2)),
        ..base()
    };
    let capture = CaptureGeo::from_exif_with(&gps, DROP).unwrap();
    assert_eq!(capture.fix.coord.height(), None);
    assert_eq!(capture.direction, None);
    assert_eq!(capture.speed_mps, None);
    assert_eq!(capture.track, None);
    assert_eq!(capture.dop, Some(2.0), "a valid tag is kept");
    let notes = &capture.fix.source.unwrap().notes;
    assert_eq!(notes.iter().filter(|n| n.contains("dropped:")).count(), 4);
    assert!(
        !notes.iter().any(|n| n.contains("unit is unknown")),
        "a dropped speed is not also reported as unitless: {notes:?}"
    );
}

#[test]
fn drop_changes_nothing_when_every_tag_is_valid() {
    let gps = ExifGps {
        altitude: Some(whole(12)),
        dop: Some(whole(3)),
        speed: Some(whole(36)),
        speed_ref: text("K"),
        ..base()
    };
    assert_eq!(
        CaptureGeo::from_exif_with(&gps, DROP).unwrap(),
        CaptureGeo::from_exif(&gps).unwrap()
    );
}

#[test]
fn position_errors_fail_under_drop() {
    let null_island = ExifGps {
        latitude: Some([whole(0); 3]),
        longitude: Some([whole(0); 3]),
        altitude: Some(U(0, 0)),
        ..base()
    };
    let strict = DROP.with_null_island(NullIslandPolicy::Reject);
    assert!(matches!(
        CaptureGeo::from_exif_with(&null_island, strict),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
    let far = ExifGps {
        latitude: Some([whole(91), whole(0), whole(0)]),
        altitude: Some(U(0, 0)),
        ..base()
    };
    assert!(matches!(
        CaptureGeo::from_exif_with(&far, DROP),
        Err(Error::OutOfRange { .. })
    ));
    for gps in [
        ExifGps::default(),
        ExifGps {
            latitude: Some([U(0, 0); 3]),
            longitude: Some([U(0, 0); 3]),
            altitude: Some(U(0, 0)),
            ..base()
        },
    ] {
        assert!(matches!(
            CaptureGeo::from_exif_with(&gps, DROP),
            Err(Error::Parse {
                kind: ParseErrorKind::NoFix,
                ..
            })
        ));
    }
    let blank = XmpGps {
        latitude: text("10N"),
        altitude: text("1/0"),
        ..xmp_base()
    };
    assert!(matches!(
        CaptureGeo::from_xmp_with(&blank, DROP),
        Err(Error::Parse {
            kind: ParseErrorKind::Syntax,
            ..
        })
    ));
}

#[test]
fn a_position_error_wins_over_an_auxiliary_one() {
    let gps = ExifGps {
        latitude_ref: text("X"),
        altitude: Some(U(1, 0)),
        ..base()
    };
    assert_eq!(field_of(&gps), "latitude_ref");
}

#[cfg(feature = "serde")]
#[test]
fn invalid_auxiliary_round_trips_and_defaults_to_reject() {
    let options: ExifOptions = serde_json::from_str(r#"{"invalid_auxiliary":"Drop"}"#).unwrap();
    assert_eq!(options, DROP);
    let json = serde_json::to_string(&options).unwrap();
    assert_eq!(serde_json::from_str::<ExifOptions>(&json).unwrap(), DROP);
    let options: ExifOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(options.invalid_auxiliary, AuxiliaryTagPolicy::Reject);
}

/// Every option combination: both auxiliary-tag and both null-island policies.
const ALL_OPTIONS: [ExifOptions; 4] = [
    ExifOptions::new(),
    DROP,
    ExifOptions::new().with_null_island(NullIslandPolicy::Reject),
    DROP.with_null_island(NullIslandPolicy::Reject),
];

/// The result is `Ok`, or one of the typed errors a reader of tag values
/// produces; under `Drop` an input `Reject` accepts reads the same.
fn assert_typed(results: [crate::Result<CaptureGeo>; 4], input: &dyn std::fmt::Debug) {
    for result in &results {
        assert!(
            matches!(
                result,
                Ok(_)
                    | Err(Error::InvalidValue { .. }
                        | Error::Parse { .. }
                        | Error::OutOfRange { .. })
            ),
            "{result:?} from {input:?}"
        );
    }
    for (reject, drop) in [(0, 1), (2, 3)] {
        if let Ok(accepted) = &results[reject] {
            assert_eq!(results[drop].as_ref().ok(), Some(accepted), "{input:?}");
        }
    }
}

/// Any rational: the full `u32` / `i32` range, zero denominators included,
/// biased toward the small values a real GPS IFD holds.
fn rational() -> impl Strategy<Value = ExifRational> {
    prop_oneof![
        (0u32..120, 0u32..4).prop_map(|(n, d)| U(n, d)),
        any::<(u32, u32)>().prop_map(|(n, d)| U(n, d)),
        any::<(i32, i32)>().prop_map(|(n, d)| ExifRational::Signed(n, d)),
    ]
}

/// A hemisphere, reference or datum string: a meaningful letter or any text.
fn letter() -> impl Strategy<Value = Option<String>> {
    proptest::option::of(prop_oneof![
        proptest::sample::select(vec!["N", "S", "E", "W", "T", "M", "K", "", "WGS-84"])
            .prop_map(String::from),
        "\\PC{0,6}",
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn exif_rationals_never_panic(
        position in (
            proptest::option::of([rational(), rational(), rational()]),
            letter(),
            proptest::option::of([rational(), rational(), rational()]),
            letter(),
        ),
        altitude in (proptest::option::of(rational()), proptest::option::of(any::<u8>())),
        quality in (
            letter(),
            proptest::option::of(rational()),
            proptest::option::of(rational()),
            proptest::option::of("[0-9: ]{0,12}|\\PC{0,12}"),
            proptest::option::of([rational(), rational(), rational()]),
        ),
        motion in (
            proptest::option::of(rational()),
            letter(),
            proptest::option::of(rational()),
            letter(),
            proptest::option::of(rational()),
            letter(),
        ),
    ) {
        let gps = ExifGps {
            latitude: position.0,
            latitude_ref: position.1,
            longitude: position.2,
            longitude_ref: position.3,
            altitude: altitude.0,
            altitude_ref: altitude.1,
            map_datum: quality.0,
            h_positioning_error: quality.1,
            dop: quality.2,
            date_stamp: quality.3,
            time_stamp: quality.4,
            img_direction: motion.0,
            img_direction_ref: motion.1,
            speed: motion.2,
            speed_ref: motion.3,
            track: motion.4,
            track_ref: motion.5,
        };
        assert_typed(ALL_OPTIONS.map(|o| CaptureGeo::from_exif_with(&gps, o)), &gps);
    }

    #[test]
    fn xmp_property_strings_never_panic(
        position in (
            proptest::option::of(
                "[0-9]{1,3},[0-9]{1,2}(\\.[0-9]{0,4})?[NSEW]|[0-9]{1,3},[0-9]{1,2},[0-9]{1,2}[NSEW]|\\PC{0,16}"
            ),
            proptest::option::of(
                "[0-9]{1,3},[0-9]{1,2}(\\.[0-9]{0,4})?[NSEW]|[0-9]{1,3},[0-9]{1,2},[0-9]{1,2}[NSEW]|\\PC{0,16}"
            ),
        ),
        values in proptest::collection::vec(
            proptest::option::of("-?[0-9]{1,6}(/[0-9]{0,4}|\\.[0-9]{0,3})?|\\PC{0,12}"),
            7,
        ),
        stamp in proptest::option::of(
            "[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}(:[0-9]{2}(\\.[0-9]{1,3})?)?(Z|[+-][0-9]{2}:[0-9]{2})?|\\PC{0,24}"
        ),
        letters in (letter(), letter(), letter(), letter()),
    ) {
        let gps = XmpGps {
            latitude: position.0,
            longitude: position.1,
            altitude: values[0].clone(),
            altitude_ref: values[1].clone(),
            map_datum: letters.0,
            h_positioning_error: values[2].clone(),
            dop: values[3].clone(),
            time_stamp: stamp,
            img_direction: values[4].clone(),
            img_direction_ref: letters.1,
            speed: values[5].clone(),
            speed_ref: letters.2,
            track: values[6].clone(),
            track_ref: letters.3,
        };
        assert_typed(ALL_OPTIONS.map(|o| CaptureGeo::from_xmp_with(&gps, o)), &gps);
    }
}
