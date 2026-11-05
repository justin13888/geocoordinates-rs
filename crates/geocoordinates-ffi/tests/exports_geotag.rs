//! Calls every geotag FFI export at least once with an asserted value, including
//! the failure each one documents.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use geocoordinates_ffi::*;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

fn at(second: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(second)
}

fn fix(lat: f64, lon: f64, second: Option<u64>) -> Fix {
    Fix {
        coord: wgs84(lat, lon),
        accuracy: None,
        timestamp: second.map(at),
        source: None,
    }
}

/// Along the equator: 0° at 1000 s, 0.01° E at 1100 s, and one untimed fix.
fn track() -> std::sync::Arc<Track> {
    Track::new(vec![
        fix(0.0, 0.01, Some(1_100)),
        fix(5.0, 5.0, None),
        fix(0.0, 0.0, Some(1_000)),
    ])
    .unwrap()
}

#[test]
fn default_options_mirror_the_core_defaults() {
    let options = geotag_options_default();
    assert_eq!(options.clock_offset_ms, 0);
    assert_eq!(options.max_gap, Duration::from_secs(1_800));
    assert_eq!(options.max_extrapolation, Duration::ZERO);
    assert_eq!(options.interpolation, Interpolation::Geodesic);
    assert_eq!(options.max_speed_mps, 50.0);
}

#[test]
fn a_track_is_built_sorted_and_reports_what_it_dropped() {
    let track = track();
    assert_eq!((track.len(), track.dropped_untimed()), (2, 1));
    assert!(!track.is_empty());
    assert_eq!(track.crs(), Some(Crs::Wgs84));
    assert_eq!(track.fixes()[0].timestamp, Some(at(1_000)));
    let empty = Track::new(vec![]).unwrap();
    assert!(empty.is_empty() && empty.crs().is_none());
}

#[test]
fn a_track_refuses_conflicts_mixed_datums_and_bad_records() {
    assert!(matches!(
        Track::new(vec![fix(0.0, 0.0, Some(1)), fix(0.0, 1.0, Some(1))]),
        Err(GeoError::InvalidValue { field, .. }) if field == "fixes"
    ));
    let mut gcj = fix(39.9, 116.4, Some(1));
    gcj.coord = coordinate_new(39.9, 116.4, Crs::Gcj02).unwrap();
    assert!(matches!(
        Track::new(vec![gcj]),
        Err(GeoError::CrsMismatch { .. })
    ));
    let mut bad = fix(0.0, 0.0, Some(1));
    bad.coord.lat = 91.0;
    assert!(Track::new(vec![bad]).is_err());
}

#[test]
fn locate_interpolates_applies_the_offset_and_marks_provenance() {
    let track = track();
    // A camera 10 s fast.
    let options = GeotagOptions {
        clock_offset_ms: -10_000,
        ..geotag_options_default()
    };
    let located = track.locate(at(1_035), options).unwrap().unwrap();
    assert!((located.coord.lon - 0.0025).abs() < 1e-9);
    assert_eq!(located.timestamp, Some(at(1_025)));
    let source = located.source.unwrap();
    assert_eq!(
        source.raw,
        "geotag:interpolated t0=1000.000 t1=1100.000 f=0.250"
    );
    assert_eq!(
        source.notes,
        ["clock offset of -10000 ms applied to the photo time"]
    );
    assert_eq!(located.accuracy.unwrap().kind, AccuracyKind::Unspecified);

    let nearest = GeotagOptions {
        interpolation: Interpolation::Nearest,
        ..geotag_options_default()
    };
    let held = track.locate(at(1_080), nearest).unwrap().unwrap();
    assert_eq!(held.coord.lon, 0.01);

    // Outside the track, and past it with extrapolation.
    assert_eq!(
        track.locate(at(1_200), geotag_options_default()).unwrap(),
        None
    );
    let reach = GeotagOptions {
        max_extrapolation: Duration::from_secs(100),
        ..geotag_options_default()
    };
    let past = track.locate(at(1_200), reach).unwrap().unwrap();
    assert_eq!(
        past.source.unwrap().raw,
        "geotag:extrapolated t=1100.000 dt=100.000"
    );

    let reckless = GeotagOptions {
        max_speed_mps: f64::NAN,
        ..geotag_options_default()
    };
    assert!(matches!(
        track.locate(at(1_050), reckless),
        Err(GeoError::InvalidValue { field, .. }) if field == "max_speed_mps"
    ));
}

#[test]
fn estimate_offset_ms_finds_the_camera_clock_error() {
    let track = track();
    // Photographed at the place the track passed at 1050 s; the camera read 1650 s.
    let reference = GeotagReference {
        photo_time: at(1_650),
        location: wgs84(0.0, 0.005),
    };
    assert_eq!(
        estimate_offset_ms(track.clone(), vec![reference]).unwrap(),
        -600_000
    );
    assert!(matches!(
        estimate_offset_ms(track.clone(), vec![]),
        Err(GeoError::InvalidValue { field, .. }) if field == "references"
    ));
    assert!(matches!(
        estimate_offset_ms(Track::new(vec![]).unwrap(), vec![reference]),
        Err(GeoError::InvalidValue { field, .. }) if field == "track"
    ));
    let elsewhere = GeotagReference {
        location: coordinate_new(0.0, 0.005, Crs::Tokyo).unwrap(),
        ..reference
    };
    assert!(matches!(
        estimate_offset_ms(track.clone(), vec![elsewhere]),
        Err(GeoError::CrsMismatch { .. })
    ));
    let bad = GeotagReference {
        location: Coordinate {
            lat: 91.0,
            ..wgs84(0.0, 0.0)
        },
        ..reference
    };
    assert!(estimate_offset_ms(track, vec![bad]).is_err());
}
