//! Rules of the metadata tags beside the position (altitude, datum, raw record,
//! accuracy) and the null-island policy, table-driven. The position tags' own
//! rules are in `tests.rs`.

use super::tests::{U, base, dms, field_of, text, whole, xmp_base};
use super::{CaptureGeo, ExifGps, ExifOptions, ExifRational, NullIslandPolicy, XmpGps};
use crate::fix::{AccuracyKind, DatumAmbiguity};
use crate::{Crs, Error, Height, ParseErrorKind};

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
            CaptureGeo::from_exif(&gps).unwrap().fix.coord.height(),
            Some(height),
            "{reference:?}"
        );
    }
    let ref_alone = ExifGps {
        altitude_ref: Some(1),
        ..base()
    };
    let coord = CaptureGeo::from_exif(&ref_alone).unwrap().fix.coord;
    assert_eq!(coord.height(), None);
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
    let Some(Height::Ellipsoidal(meters)) = CaptureGeo::from_exif(&gps).unwrap().fix.coord.height()
    else {
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
        let fix = CaptureGeo::from_exif(&gps).unwrap().fix;
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
        let fix = CaptureGeo::from_exif(&beijing(datum.clone())).unwrap().fix;
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
        let fix = CaptureGeo::from_exif(&beijing(text(name))).unwrap().fix;
        let source = fix.source.unwrap();
        assert_eq!(source.datum_ambiguity, None, "{name}");
    }
    // Outside the box an unstated datum is noted, not flagged.
    let source = CaptureGeo::from_exif(&base()).unwrap().fix.source.unwrap();
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
    let source = CaptureGeo::from_exif(&gps).unwrap().fix.source.unwrap();
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
        CaptureGeo::from_exif(&signed)
            .unwrap()
            .fix
            .source
            .unwrap()
            .raw
            .starts_with("GPSLatitude=10/1,")
    );
    let xmp = CaptureGeo::from_xmp(&xmp_base()).unwrap().fix;
    assert_eq!(
        xmp.source.unwrap().raw,
        "GPSLatitude=10,0.0N; GPSLongitude=20,0.0E"
    );
}

#[test]
fn accuracy_is_a_bound_only_from_the_positioning_error() {
    let gps = ExifGps {
        h_positioning_error: Some(U(9, 2)),
        dop: Some(U(3, 2)),
        ..base()
    };
    let capture = CaptureGeo::from_exif(&gps).unwrap();
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
    // Accepted as the origin itself, on WGS-84, not as some other point.
    for capture in [CaptureGeo::from_exif(&ifd), CaptureGeo::from_xmp(&xmp)] {
        let c = capture.unwrap().fix.coord;
        assert_eq!((c.lat(), c.lon(), c.crs()), (0.0, 0.0, Crs::Wgs84));
    }
    assert!(matches!(
        CaptureGeo::from_exif_with(&ifd, strict),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
    assert!(matches!(
        CaptureGeo::from_xmp_with(&xmp, strict),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
    // Only both axes at zero count.
    for (zero_lat, at) in [(true, (0.0, 20.0)), (false, (10.0, 0.0))] {
        let mut gps = base();
        *if zero_lat {
            &mut gps.latitude
        } else {
            &mut gps.longitude
        } = dms(0, 0, 0);
        let coord = CaptureGeo::from_exif_with(&gps, strict).unwrap().fix.coord;
        assert_eq!((coord.lat(), coord.lon()), at);
    }
}
