//! Rules of the IFD and XMP paths' position tags, table-driven, and the helpers
//! the sibling test files share. The other tags' rules are in `tag_tests.rs`, and
//! real device values in `device_tests.rs`.

use super::{CaptureGeo, ExifGps, ExifRational, XmpGps};
use crate::{Error, ParseErrorKind};

pub(super) const U: fn(u32, u32) -> ExifRational = ExifRational::Unsigned;

pub(super) fn whole(value: u32) -> ExifRational {
    U(value, 1)
}

pub(super) fn dms(degrees: u32, minutes: u32, seconds: u32) -> Option<[ExifRational; 3]> {
    Some([whole(degrees), whole(minutes), whole(seconds)])
}

pub(super) fn text(value: &str) -> Option<String> {
    Some(value.to_string())
}

/// 10°N 20°E: outside China's box, every other tag absent.
pub(super) fn base() -> ExifGps {
    ExifGps {
        latitude: dms(10, 0, 0),
        latitude_ref: text("N"),
        longitude: dms(20, 0, 0),
        longitude_ref: text("E"),
        ..ExifGps::default()
    }
}

pub(super) fn xmp_base() -> XmpGps {
    XmpGps {
        latitude: text("10,0.0N"),
        longitude: text("20,0.0E"),
        ..XmpGps::default()
    }
}

pub(super) fn field_of(gps: &ExifGps) -> &'static str {
    match CaptureGeo::from_exif(gps) {
        Err(Error::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn ref_letters_set_the_sign_ignoring_case_and_padding() {
    for (lat_ref, lon_ref, lat, lon) in [
        ("N", "E", 10.0, 20.0),
        ("S", "W", -10.0, -20.0),
        ("n", "e", 10.0, 20.0),
        (" s\0", "w\0\0", -10.0, -20.0),
    ] {
        let gps = ExifGps {
            latitude_ref: text(lat_ref),
            longitude_ref: text(lon_ref),
            ..base()
        };
        let coord = CaptureGeo::from_exif(&gps).unwrap().fix.coord;
        assert_eq!(
            (coord.lat(), coord.lon()),
            (lat, lon),
            "{lat_ref} {lon_ref}"
        );
    }
}

#[test]
fn every_invalid_or_absent_position_ref_is_an_error() {
    for bad in [
        None,
        text(""),
        text("\0"),
        text("E"),
        text("X"),
        text("North"),
    ] {
        let gps = ExifGps {
            latitude_ref: bad.clone(),
            ..base()
        };
        assert_eq!(field_of(&gps), "latitude_ref", "{bad:?}");
    }
    for bad in [None, text(""), text("N"), text("West")] {
        let gps = ExifGps {
            longitude_ref: bad.clone(),
            ..base()
        };
        assert_eq!(field_of(&gps), "longitude_ref", "{bad:?}");
    }
}

/// One invalid value per row, with the field its error names; `policy_tests.rs`
/// runs it under each [`AuxiliaryTagPolicy`](super::AuxiliaryTagPolicy).
pub(super) fn rejections() -> Vec<(&'static str, ExifGps)> {
    let zero_denominator = U(1, 0);
    let negative = ExifRational::Signed(-1, 1);
    vec![
        (
            "latitude",
            ExifGps {
                latitude: Some([zero_denominator, whole(0), whole(0)]),
                ..base()
            },
        ),
        (
            "latitude",
            ExifGps {
                latitude: Some([whole(1), zero_denominator, whole(0)]),
                ..base()
            },
        ),
        (
            "latitude",
            ExifGps {
                latitude: Some([whole(1), whole(0), zero_denominator]),
                ..base()
            },
        ),
        (
            "latitude",
            ExifGps {
                latitude: Some([negative, whole(0), whole(0)]),
                ..base()
            },
        ),
        (
            "latitude",
            ExifGps {
                latitude: dms(10, 60, 0),
                ..base()
            },
        ),
        (
            "latitude",
            ExifGps {
                latitude: dms(10, 0, 60),
                ..base()
            },
        ),
        (
            "longitude",
            ExifGps {
                longitude: dms(20, 60, 0),
                ..base()
            },
        ),
        (
            "longitude",
            ExifGps {
                longitude: dms(20, 0, 60),
                ..base()
            },
        ),
        (
            "longitude",
            ExifGps {
                longitude: Some([zero_denominator, whole(0), whole(0)]),
                ..base()
            },
        ),
        (
            "altitude",
            ExifGps {
                altitude: Some(zero_denominator),
                ..base()
            },
        ),
        (
            "altitude",
            ExifGps {
                altitude: Some(negative),
                ..base()
            },
        ),
        (
            "altitude_ref",
            ExifGps {
                altitude: Some(whole(1)),
                altitude_ref: Some(4),
                ..base()
            },
        ),
        (
            "h_positioning_error",
            ExifGps {
                h_positioning_error: Some(zero_denominator),
                ..base()
            },
        ),
        (
            "dop",
            ExifGps {
                dop: Some(zero_denominator),
                ..base()
            },
        ),
        (
            "time_stamp",
            ExifGps {
                time_stamp: Some([zero_denominator, whole(0), whole(0)]),
                ..base()
            },
        ),
        (
            "time_stamp",
            ExifGps {
                time_stamp: dms(24, 0, 0),
                ..base()
            },
        ),
        (
            "time_stamp",
            ExifGps {
                time_stamp: dms(0, 60, 0),
                ..base()
            },
        ),
        (
            "time_stamp",
            ExifGps {
                time_stamp: dms(0, 0, 60),
                ..base()
            },
        ),
        (
            "img_direction",
            ExifGps {
                img_direction: Some(whole(360)),
                ..base()
            },
        ),
        (
            "img_direction",
            ExifGps {
                img_direction: Some(zero_denominator),
                ..base()
            },
        ),
        (
            "img_direction_ref",
            ExifGps {
                img_direction: Some(whole(1)),
                img_direction_ref: text("X"),
                ..base()
            },
        ),
        (
            "speed",
            ExifGps {
                speed: Some(zero_denominator),
                ..base()
            },
        ),
        (
            "speed_ref",
            ExifGps {
                speed: Some(whole(1)),
                speed_ref: text("X"),
                ..base()
            },
        ),
        (
            "track",
            ExifGps {
                track: Some(whole(361)),
                ..base()
            },
        ),
        (
            "track_ref",
            ExifGps {
                track: Some(whole(1)),
                track_ref: text("X"),
                ..base()
            },
        ),
    ]
}

#[test]
fn out_of_range_positions_are_the_pair_error() {
    for gps in [
        ExifGps {
            latitude: dms(90, 0, 1),
            ..base()
        },
        ExifGps {
            longitude: dms(180, 1, 0),
            ..base()
        },
    ] {
        assert!(
            matches!(CaptureGeo::from_exif(&gps), Err(Error::OutOfRange { .. })),
            "{gps:?}"
        );
    }
    let pole = ExifGps {
        latitude: dms(90, 0, 0),
        longitude: dms(180, 0, 0),
        ..base()
    };
    assert_eq!(CaptureGeo::from_exif(&pole).unwrap().fix.coord.lat(), 90.0);
}

#[test]
fn missing_position_or_placeholder_block_is_no_fix() {
    let zeros = Some([U(0, 0); 3]);
    for gps in [
        ExifGps::default(),
        ExifGps {
            latitude: None,
            ..base()
        },
        ExifGps {
            longitude: None,
            ..base()
        },
        ExifGps {
            latitude: zeros,
            longitude: zeros,
            ..base()
        },
        ExifGps {
            latitude: Some([ExifRational::Signed(0, 0); 3]),
            longitude: Some([ExifRational::Signed(0, 0); 3]),
            ..base()
        },
    ] {
        assert!(
            matches!(
                CaptureGeo::from_exif(&gps),
                Err(Error::Parse {
                    kind: ParseErrorKind::NoFix,
                    ..
                })
            ),
            "{gps:?}"
        );
    }
    // A 0/0 on one axis only is a bad value, not a blank block.
    assert_eq!(
        field_of(&ExifGps {
            latitude: zeros,
            ..base()
        }),
        "latitude"
    );
    let partial = Some([U(0, 0), U(0, 0), U(1, 1)]);
    assert_eq!(
        field_of(&ExifGps {
            latitude: partial,
            longitude: zeros,
            ..base()
        }),
        "latitude"
    );
    for gps in [
        XmpGps::default(),
        XmpGps {
            latitude: None,
            ..xmp_base()
        },
        XmpGps {
            longitude: None,
            ..xmp_base()
        },
    ] {
        assert!(matches!(
            CaptureGeo::from_xmp(&gps),
            Err(Error::Parse {
                kind: ParseErrorKind::NoFix,
                ..
            })
        ));
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_validates() {
    use super::{ExifOptions, NorthBearing, NorthReference, NullIslandPolicy};

    let gps = ExifGps {
        altitude: Some(U(1, 2)),
        altitude_ref: Some(1),
        ..base()
    };
    let json = serde_json::to_string(&gps).unwrap();
    assert_eq!(serde_json::from_str::<ExifGps>(&json).unwrap(), gps);
    assert_eq!(
        serde_json::from_str::<ExifGps>("{}").unwrap(),
        ExifGps::default()
    );
    assert_eq!(
        serde_json::from_str::<XmpGps>("{}").unwrap(),
        XmpGps::default()
    );
    let capture = CaptureGeo::from_exif(&gps).unwrap();
    let json = serde_json::to_string(&capture).unwrap();
    assert_eq!(
        serde_json::from_str::<super::CaptureGeo>(&json).unwrap(),
        capture
    );
    let bearing: NorthBearing =
        serde_json::from_str(r#"{"degrees":1.0,"reference":"True"}"#).unwrap();
    assert_eq!(bearing.reference(), Some(NorthReference::True));
    assert!(serde_json::from_str::<NorthBearing>(r#"{"degrees":360.0,"reference":null}"#).is_err());
    let options: ExifOptions = serde_json::from_str(r#"{"null_island":"Reject"}"#).unwrap();
    assert_eq!(options.null_island, NullIslandPolicy::Reject);
    assert_eq!(
        serde_json::from_str::<ExifOptions>("{}").unwrap(),
        ExifOptions::default()
    );
}
