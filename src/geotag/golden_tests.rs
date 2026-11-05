//! A GPX track read by `from_gpx`, located against positions computed outside
//! this crate with GeographicLib 2.0 (Python: `Geodesic.WGS84.InverseLine(a, b)
//! .Position(f * s13)`), the reference implementation of the geodesics
//! `geodesy::intermediate` runs.

#![cfg(feature = "gpx")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{GeotagOptions, Track};
use crate::Height;
use crate::fix::AccuracyKind;
use crate::parse::interchange::from_gpx;

const GOLDEN: &str = include_str!("testdata/golden.gpx");

/// 2024-06-01T10:00:00Z.
const ZURICH_START: u64 = 1_717_236_000;
/// 2024-06-02T00:00:00Z.
const TAVEUNI_START: u64 = 1_717_286_400;

/// The camera ran on UTC+2 with no zone, so its clock reads two hours ahead.
const CAMERA_AHEAD_MS: i64 = 2 * 3_600_000;

fn track() -> Track {
    Track::new(from_gpx(GOLDEN).unwrap()).unwrap()
}

/// The camera's reading `secs` seconds after `start` UTC.
fn camera(start: u64, secs: u64) -> SystemTime {
    UNIX_EPOCH
        + Duration::from_secs(start + secs)
        + Duration::from_millis(CAMERA_AHEAD_MS.unsigned_abs())
}

fn options() -> GeotagOptions {
    GeotagOptions::default().with_clock_offset_ms(-CAMERA_AHEAD_MS)
}

#[test]
fn every_track_point_is_kept_in_order() {
    let track = track();
    assert_eq!((track.len(), track.dropped_untimed()), (7, 0));
    assert_eq!(track.fixes()[0].coord.lat(), 47.3769);
    assert_eq!(track.fixes()[6].coord.lon(), -179.9985);
}

#[test]
fn interpolated_positions_match_geographiclib() {
    let track = track();
    // (seconds after the start, expected lat, lon, height)
    let cases = [
        (
            ZURICH_START,
            5,
            47.377_200_001_851,
            8.542_349_996_313,
            409.0,
        ),
        (
            ZURICH_START,
            31,
            47.378_440_000_598,
            8.544_339_997_459,
            411.6,
        ),
        (
            TAVEUNI_START,
            30,
            -16.800_500_003_808,
            -179.999_750_003_273,
            6.0,
        ),
    ];
    for (start, secs, lat, lon, height) in cases {
        let fix = track
            .locate(camera(start, secs), &options())
            .unwrap()
            .unwrap_or_else(|| panic!("{start} + {secs} s is inside the track"));
        assert!(
            (fix.coord.lat() - lat).abs() < 1e-9,
            "{secs}: {}",
            fix.coord.lat()
        );
        assert!(
            (fix.coord.lon() - lon).abs() < 1e-9,
            "{secs}: {}",
            fix.coord.lon()
        );
        let Some(Height::Orthometric(h)) = fix.coord.height() else {
            panic!("{secs}: GPX <ele> heights interpolate as orthometric");
        };
        assert!((h - height).abs() < 1e-9, "{secs}: {h}");
        assert_eq!(
            fix.timestamp,
            Some(UNIX_EPOCH + Duration::from_secs(start + secs))
        );
        let source = fix.source.unwrap();
        assert!(
            source.raw.starts_with("geotag:interpolated"),
            "{}",
            source.raw
        );
        assert_eq!(
            source.notes,
            ["clock offset of -7200000 ms applied to the photo time"]
        );
    }
}

#[test]
fn a_gpx_hdop_is_not_a_bound_so_neither_is_the_result() {
    let fix = track()
        .locate(camera(ZURICH_START, 5), &options())
        .unwrap()
        .unwrap();
    let accuracy = fix.accuracy.unwrap();
    assert_eq!(accuracy.kind(), AccuracyKind::Unspecified);
    // From the first fix: HDOP 1 × 5 m, 5 s at 50 m/s, and half the 118.693 m
    // segment GeographicLib measures.
    let expected = 5.0 + 250.0 + 118.693_321_298_178_49 / 2.0;
    let horizontal = accuracy.horizontal().unwrap().meters();
    assert!((horizontal - expected).abs() < 1e-6, "{horizontal}");
}

#[test]
fn a_photo_at_a_track_point_takes_it_exactly() {
    let fix = track()
        .locate(camera(ZURICH_START, 25), &options())
        .unwrap()
        .unwrap();
    assert_eq!((fix.coord.lat(), fix.coord.lon()), (47.3782, 8.5441));
    assert_eq!(fix.coord.height(), Some(Height::Orthometric(411.0)));
    assert_eq!(fix.source.unwrap().raw, "geotag:matched t=1717236025.000");
}

#[test]
fn photos_outside_the_coverage_have_no_position() {
    let track = track();
    // Before the walk, inside its 45-minute gap, and with no clock correction.
    for at in [
        camera(ZURICH_START, 0) - Duration::from_secs(60),
        camera(ZURICH_START, 20 * 60),
    ] {
        assert_eq!(track.locate(at, &options()).unwrap(), None);
    }
    assert_eq!(
        track
            .locate(camera(ZURICH_START, 5), &GeotagOptions::default())
            .unwrap(),
        None
    );
    // Two minutes of extrapolation reaches 45 s past the walk's fourth point.
    let reach = options().with_max_extrapolation(Duration::from_secs(120));
    let held = track
        .locate(camera(ZURICH_START, 90), &reach)
        .unwrap()
        .unwrap();
    assert_eq!((held.coord.lat(), held.coord.lon()), (47.379, 8.5449));
    assert_eq!(
        held.source.unwrap().raw,
        "geotag:extrapolated t=1717236045.000 dt=45.000"
    );
}
