//! Rules of the IFD and XMP paths, table-driven. Real device values are in
//! `device_tests.rs`.

use super::{
    ExifGps, ExifOptions, ExifRational, NullIslandPolicy, XmpGps, from_exif, from_exif_with,
    from_xmp, from_xmp_with,
};
use crate::fix::{AccuracyKind, DatumAmbiguity};
use crate::{Crs, Error, Height, ParseErrorKind};

pub(super) const U: fn(u32, u32) -> ExifRational = ExifRational::Unsigned;

pub(super) fn whole(value: u32) -> ExifRational {
    U(value, 1)
}

fn dms(degrees: u32, minutes: u32, seconds: u32) -> Option<[ExifRational; 3]> {
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

fn field_of(gps: &ExifGps) -> &'static str {
    match from_exif(gps) {
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
        let coord = from_exif(&gps).unwrap().fix.coord;
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

#[test]
fn rejects_each_invalid_value_naming_its_tag() {
    let zero_denominator = U(1, 0);
    let negative = ExifRational::Signed(-1, 1);
    let cases: Vec<(&str, ExifGps)> = vec![
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
    ];
    for (field, gps) in &cases {
        assert_eq!(field_of(gps), *field, "{gps:?}");
    }
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
            matches!(from_exif(&gps), Err(Error::OutOfRange { .. })),
            "{gps:?}"
        );
    }
    let pole = ExifGps {
        latitude: dms(90, 0, 0),
        longitude: dms(180, 0, 0),
        ..base()
    };
    assert_eq!(from_exif(&pole).unwrap().fix.coord.lat(), 90.0);
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
                from_exif(&gps),
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
            from_xmp(&gps),
            Err(Error::Parse {
                kind: ParseErrorKind::NoFix,
                ..
            })
        ));
    }
}

#[test]
fn every_altitude_ref_maps_to_its_surface_and_sign() {
    for (reference, height) in [
        (None, Height::Orthometric(5.5)),
        (Some(0), Height::Orthometric(5.5)),
        (Some(1), Height::Orthometric(-5.5)),
        (Some(2), Height::Ellipsoidal(5.5)),
        (Some(3), Height::Ellipsoidal(-5.5)),
    ] {
        let gps = ExifGps {
            altitude: Some(U(11, 2)),
            altitude_ref: reference,
            ..base()
        };
        assert_eq!(
            from_exif(&gps).unwrap().fix.coord.height(),
            Some(height),
            "{reference:?}"
        );
    }
    let ref_alone = ExifGps {
        altitude_ref: Some(1),
        ..base()
    };
    assert_eq!(from_exif(&ref_alone).unwrap().fix.coord.height(), None);
}

#[test]
fn an_unrecognised_ref_is_an_error_without_its_value_tag() {
    for (field, gps) in [
        (
            "altitude_ref",
            ExifGps {
                altitude_ref: Some(9),
                ..base()
            },
        ),
        (
            "speed_ref",
            ExifGps {
                speed_ref: text("bogus"),
                ..base()
            },
        ),
        (
            "img_direction_ref",
            ExifGps {
                img_direction_ref: text("bogus"),
                ..base()
            },
        ),
        (
            "track_ref",
            ExifGps {
                track_ref: text("bogus"),
                ..base()
            },
        ),
    ] {
        assert_eq!(field_of(&gps), field);
    }
}

#[test]
fn zero_altitude_below_the_ellipsoid_is_positive_zero() {
    let gps = ExifGps {
        altitude: Some(U(0, 1)),
        altitude_ref: Some(3),
        ..base()
    };
    let Some(Height::Ellipsoidal(meters)) = from_exif(&gps).unwrap().fix.coord.height() else {
        panic!("expected an ellipsoidal height");
    };
    assert!(meters.is_sign_positive());
}

#[test]
fn map_datum_spellings_set_the_crs_at_full_confidence() {
    for (name, crs) in [
        ("WGS-84", Crs::Wgs84),
        ("WGS84", Crs::Wgs84),
        ("wgs 84", Crs::Wgs84),
        ("WGS_84\0", Crs::Wgs84),
        ("WGS 1984", Crs::Wgs84),
        ("World Geodetic System 1984", Crs::Wgs84),
        ("EPSG:4326", Crs::Wgs84),
        ("TOKYO", Crs::Tokyo),
        ("tokyo", Crs::Tokyo),
        ("GCJ-02", Crs::Gcj02),
        ("BD-09", Crs::Bd09),
    ] {
        let gps = ExifGps {
            map_datum: text(name),
            ..base()
        };
        let fix = from_exif(&gps).unwrap().fix;
        assert_eq!(fix.coord.crs(), crs, "{name}");
        let source = fix.source.unwrap();
        assert_eq!(source.confidence.value(), 1.0, "{name}");
        assert!(source.notes.is_empty(), "{name}");
    }
}

#[test]
fn china_ambiguity_needs_no_explicit_datum_and_a_position_in_the_box() {
    let beijing = |map_datum: Option<String>| ExifGps {
        latitude: dms(39, 54, 0),
        longitude: dms(116, 24, 0),
        map_datum,
        ..base()
    };
    for (datum, note) in [
        (None, "absent"),
        (text(""), "absent"),
        (text(" \0"), "absent"),
        (text("NAD27"), "not recognised"),
    ] {
        let fix = from_exif(&beijing(datum.clone())).unwrap().fix;
        assert_eq!(fix.coord.crs(), Crs::Wgs84, "{datum:?}");
        let source = fix.source.unwrap();
        assert_eq!(
            source.datum_ambiguity,
            Some(DatumAmbiguity::PossiblyGcj02),
            "{datum:?}"
        );
        assert_eq!(source.confidence.value(), 0.7, "{datum:?}");
        assert!(source.notes[0].contains(note), "{:?}", source.notes);
    }
    // Explicit datums are taken at their word, and WGS-84 stays unflagged.
    for name in ["WGS-84", "TOKYO", "GCJ-02"] {
        let source = from_exif(&beijing(text(name))).unwrap().fix.source.unwrap();
        assert_eq!(source.datum_ambiguity, None, "{name}");
    }
    // Outside the box an unstated datum is noted, not flagged.
    let source = from_exif(&base()).unwrap().fix.source.unwrap();
    assert_eq!(source.datum_ambiguity, None);
    assert_eq!(source.confidence.value(), 0.7);
    assert_eq!(source.notes.len(), 1);
}

#[test]
fn raw_records_the_position_tags_with_no_axis_order() {
    let gps = ExifGps {
        latitude_ref: text("n\0"),
        ..base()
    };
    let source = from_exif(&gps).unwrap().fix.source.unwrap();
    assert_eq!(
        source.raw,
        "GPSLatitude=10/1,0/1,0/1 n; GPSLongitude=20/1,0/1,0/1 E"
    );
    assert_eq!(source.axis_order, None);
    let signed = ExifGps {
        latitude: Some([ExifRational::Signed(10, 1), whole(0), whole(0)]),
        ..base()
    };
    assert!(
        from_exif(&signed)
            .unwrap()
            .fix
            .source
            .unwrap()
            .raw
            .starts_with("GPSLatitude=10/1,")
    );
    let xmp = from_xmp(&xmp_base()).unwrap().fix.source.unwrap();
    assert_eq!(xmp.raw, "GPSLatitude=10,0.0N; GPSLongitude=20,0.0E");
}

#[test]
fn accuracy_is_a_bound_only_from_the_positioning_error() {
    let gps = ExifGps {
        h_positioning_error: Some(U(9, 2)),
        dop: Some(U(3, 2)),
        ..base()
    };
    let capture = from_exif(&gps).unwrap();
    let accuracy = capture.fix.accuracy.unwrap();
    assert_eq!(accuracy.kind(), AccuracyKind::Bound);
    assert_eq!(accuracy.horizontal().map(|h| h.meters()), Some(4.5));
    assert_eq!(accuracy.vertical(), None);
    assert_eq!(capture.dop, Some(1.5));
}

#[test]
fn null_island_is_accepted_unless_rejected_by_policy() {
    let strict = ExifOptions::new().with_null_island(NullIslandPolicy::Reject);
    let ifd = ExifGps {
        latitude: dms(0, 0, 0),
        longitude: dms(0, 0, 0),
        ..base()
    };
    let xmp = XmpGps {
        latitude: text("0,0.0N"),
        longitude: text("0,0.0E"),
        ..XmpGps::default()
    };
    assert!(from_exif(&ifd).is_ok());
    assert!(from_xmp(&xmp).is_ok());
    assert!(matches!(
        from_exif_with(&ifd, strict),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
    assert!(matches!(
        from_xmp_with(&xmp, strict),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
    // Only both axes at zero count.
    for gps in [
        ExifGps {
            latitude: dms(0, 0, 0),
            ..base()
        },
        ExifGps {
            longitude: dms(0, 0, 0),
            ..base()
        },
    ] {
        assert!(from_exif_with(&gps, strict).is_ok());
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_validates() {
    use super::{NorthBearing, NorthReference};

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
    let capture = from_exif(&gps).unwrap();
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
