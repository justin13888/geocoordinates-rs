//! Track building, lookup across gaps and ends, accuracy and provenance, and
//! clock-offset estimation.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{GeotagOptions, Interpolation, Track, estimate_offset};
use crate::fix::{Accuracy, AccuracyKind, Confidence, DatumAmbiguity, Fix, RawSource};
use crate::geodesy::{geodesic_distance, intermediate};
use crate::{Coordinate, Crs, Error, Height, Length};

fn at(secs: i64) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::from_secs(secs.unsigned_abs())
    } else {
        UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs())
    }
}

fn fix(secs: i64, lat: f64, lon: f64) -> Fix {
    let mut fix = Fix::from_coord(Coordinate::wgs84(lat, lon).unwrap());
    fix.timestamp = Some(at(secs));
    fix
}

fn with_accuracy(mut fix: Fix, kind: AccuracyKind, meters: f64) -> Fix {
    fix.accuracy = Some(Accuracy::new(kind, Some(Length::from_meters(meters)), None).unwrap());
    fix
}

fn options() -> GeotagOptions {
    GeotagOptions::default()
}

fn raw(fix: &Fix) -> &str {
    &fix.source.as_ref().unwrap().raw
}

fn horizontal(fix: &Fix) -> f64 {
    fix.accuracy.unwrap().horizontal().unwrap().meters()
}

/// Three fixes along the equator: 0° at 0 s, 0.01° E at 100 s, 0.02° E at 200 s.
fn equator() -> Track {
    Track::new(vec![
        fix(200, 0.0, 0.02),
        fix(0, 0.0, 0.0),
        fix(100, 0.0, 0.01),
    ])
    .unwrap()
}

#[test]
fn new_sorts_drops_untimed_and_keeps_one_of_each_duplicate() {
    let mut untimed = fix(0, 5.0, 5.0);
    untimed.timestamp = None;
    let mut first = fix(50, 1.0, 1.0);
    first.source = Some(RawSource::new("first", Confidence::new(1.0).unwrap()));
    let mut second = fix(50, 1.0, 1.0);
    second.source = Some(RawSource::new("second", Confidence::new(1.0).unwrap()));
    let track = Track::new(vec![
        fix(90, 2.0, 2.0),
        untimed.clone(),
        first,
        second,
        untimed,
        fix(10, 0.0, 0.0),
    ])
    .unwrap();
    assert_eq!((track.len(), track.dropped_untimed()), (3, 2));
    let times: Vec<_> = track.fixes().iter().map(|f| f.timestamp.unwrap()).collect();
    assert_eq!(times, [at(10), at(50), at(90)]);
    assert_eq!(track.fixes()[1].source.as_ref().unwrap().raw, "first");
    assert!(!track.is_empty());
    assert_eq!(track.crs(), Some(Crs::Wgs84));
}

#[test]
fn new_rejects_duplicates_that_disagree() {
    // Another place, or the same place at another height, at the same instant.
    let elsewhere = fix(10, 0.0, 0.001);
    let mut higher = fix(10, 0.0, 0.0);
    higher.coord = higher.coord.with_height(Height::Ellipsoidal(3.0)).unwrap();
    for other in [elsewhere, higher] {
        assert!(matches!(
            Track::new(vec![fix(10, 0.0, 0.0), other]),
            Err(Error::InvalidValue { field: "fixes", .. })
        ));
    }
}

#[test]
fn new_rejects_mixed_datums_and_datums_with_no_ellipsoid() {
    let mut tokyo = fix(20, 35.0, 139.0);
    tokyo.coord = Coordinate::new(35.0, 139.0, Crs::Tokyo).unwrap();
    assert_eq!(
        Track::new(vec![fix(10, 35.0, 139.0), tokyo.clone()]),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Tokyo
        })
    );
    // One datum throughout is fine, whichever it is.
    assert_eq!(Track::new(vec![tokyo]).unwrap().crs(), Some(Crs::Tokyo));
    let mut gcj = fix(10, 39.9, 116.4);
    gcj.coord = Coordinate::gcj02(39.9, 116.4).unwrap();
    assert_eq!(
        Track::new(vec![gcj]),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02
        })
    );
}

#[test]
fn an_empty_track_locates_nothing() {
    let mut untimed = fix(0, 0.0, 0.0);
    untimed.timestamp = None;
    for track in [
        Track::new(vec![]).unwrap(),
        Track::new(vec![untimed]).unwrap(),
    ] {
        assert!(track.is_empty() && track.crs().is_none());
        let reach = options().with_max_extrapolation(Duration::MAX);
        assert_eq!(track.locate(at(0), &reach).unwrap(), None);
    }
}

#[test]
fn interpolates_along_the_geodesic_in_time() {
    let track = equator();
    let fix = track.locate(at(125), &options()).unwrap().unwrap();
    let expected = intermediate(
        Coordinate::wgs84(0.0, 0.01).unwrap(),
        Coordinate::wgs84(0.0, 0.02).unwrap(),
        0.25,
    )
    .unwrap();
    assert_eq!(fix.coord, expected);
    assert!((fix.coord.lon() - 0.0125).abs() < 1e-12);
    assert_eq!(fix.timestamp, Some(at(125)));
    assert_eq!(
        raw(&fix),
        "geotag:interpolated t0=100.000 t1=200.000 f=0.250"
    );
    let source = fix.source.unwrap();
    assert_eq!(source.confidence, Confidence::new(1.0).unwrap());
    assert!(source.notes.is_empty() && source.axis_order.is_none());
}

#[test]
fn a_fix_time_is_matched_exactly() {
    let track = equator();
    for (secs, lon) in [(0, 0.0), (100, 0.01), (200, 0.02)] {
        let fix = track.locate(at(secs), &options()).unwrap().unwrap();
        assert_eq!(fix.coord.lon(), lon);
        assert_eq!(raw(&fix), format!("geotag:matched t={secs}.000"));
        // No time has passed, so the only uncertainty is the fix's own: none.
        assert_eq!(horizontal(&fix), 0.0);
    }
}

#[test]
fn nearest_takes_the_nearer_fix_in_time_and_the_earlier_on_a_tie() {
    let track = equator();
    let nearest = options().with_interpolation(Interpolation::Nearest);
    for (secs, lon) in [(149, 0.01), (150, 0.01), (151, 0.02), (10, 0.0)] {
        let fix = track.locate(at(secs), &nearest).unwrap().unwrap();
        assert_eq!(fix.coord.lon(), lon, "{secs}");
        assert!(raw(&fix).starts_with("geotag:nearest"), "{}", raw(&fix));
    }
    let fix = track.locate(at(151), &nearest).unwrap().unwrap();
    assert_eq!(raw(&fix), "geotag:nearest t=200.000 t0=100.000 t1=200.000");
    // 49 s from the fix it took, at 50 m/s.
    assert!((horizontal(&fix) - 49.0 * 50.0).abs() < 1e-9);
}

#[test]
fn track_ends_extrapolate_only_as_far_as_allowed() {
    let track = equator();
    assert_eq!(track.locate(at(-1), &options()).unwrap(), None);
    assert_eq!(track.locate(at(201), &options()).unwrap(), None);
    let reach = options().with_max_extrapolation(Duration::from_secs(30));
    // The reach is inclusive.
    let before = track.locate(at(-30), &reach).unwrap().unwrap();
    assert_eq!(before.coord.lon(), 0.0);
    assert_eq!(raw(&before), "geotag:extrapolated t=0.000 dt=-30.000");
    let after = track.locate(at(230), &reach).unwrap().unwrap();
    assert_eq!(after.coord.lon(), 0.02);
    assert_eq!(raw(&after), "geotag:extrapolated t=200.000 dt=30.000");
    assert!((horizontal(&after) - 30.0 * 50.0).abs() < 1e-9);
    assert_eq!(track.locate(at(231), &reach).unwrap(), None);
}

#[test]
fn a_gap_longer_than_max_gap_is_outside_the_track() {
    let track = Track::new(vec![fix(0, 0.0, 0.0), fix(1_000, 0.0, 0.01)]).unwrap();
    let short = options().with_max_gap(Duration::from_secs(999));
    assert_eq!(track.locate(at(500), &short).unwrap(), None);
    // A gap exactly max_gap long is still interpolated.
    let exact = options().with_max_gap(Duration::from_secs(1_000));
    assert!(raw(&track.locate(at(500), &exact).unwrap().unwrap()).starts_with("geotag:interp"));
    // Inside the long gap, extrapolation holds whichever end is nearer.
    let reach = short.with_max_extrapolation(Duration::from_secs(100));
    let early = track.locate(at(100), &reach).unwrap().unwrap();
    assert_eq!(early.coord.lon(), 0.0);
    let late = track.locate(at(950), &reach).unwrap().unwrap();
    assert_eq!(late.coord.lon(), 0.01);
    assert_eq!(raw(&late), "geotag:extrapolated t=1000.000 dt=-50.000");
    assert_eq!(track.locate(at(500), &reach).unwrap(), None);
    // Equidistant from both ends: the earlier one.
    let tie = Track::new(vec![fix(0, 0.0, 0.0), fix(200, 0.0, 0.01)]).unwrap();
    let both = options()
        .with_max_gap(Duration::from_secs(10))
        .with_max_extrapolation(Duration::from_secs(100));
    assert_eq!(
        tie.locate(at(100), &both).unwrap().unwrap().coord.lon(),
        0.0
    );
}

#[test]
fn interpolation_crosses_the_antimeridian_the_short_way() {
    let track = Track::new(vec![fix(0, -16.8, 179.99), fix(60, -16.8, -179.99)]).unwrap();
    let fix = track.locate(at(30), &options()).unwrap().unwrap();
    assert!(fix.coord.lon().abs() > 179.999, "{}", fix.coord.lon());
    assert!((fix.coord.lat() + 16.8).abs() < 1e-6);
    // A quarter of the way along is still east of the line.
    let quarter = track.locate(at(15), &options()).unwrap().unwrap();
    assert!(
        (quarter.coord.lon() - 179.995).abs() < 1e-6,
        "{}",
        quarter.coord.lon()
    );
}

#[test]
fn interpolation_reaches_over_a_pole() {
    let track = Track::new(vec![fix(0, 89.9, 0.0), fix(100, 89.9, 180.0)]).unwrap();
    let fix = track.locate(at(50), &options()).unwrap().unwrap();
    assert!((fix.coord.lat() - 90.0).abs() < 1e-9, "{}", fix.coord.lat());
}

#[test]
fn heights_interpolate_only_on_one_surface() {
    let with = |mut f: Fix, h: Height| {
        f.coord = f.coord.with_height(h).unwrap();
        f
    };
    let track = Track::new(vec![
        with(fix(0, 0.0, 0.0), Height::Ellipsoidal(10.0)),
        with(fix(100, 0.0, 0.01), Height::Ellipsoidal(20.0)),
        with(fix(200, 0.0, 0.02), Height::Orthometric(30.0)),
        fix(300, 0.0, 0.03),
    ])
    .unwrap();
    let height = |secs| {
        track
            .locate(at(secs), &options())
            .unwrap()
            .unwrap()
            .coord
            .height()
    };
    assert_eq!(height(25), Some(Height::Ellipsoidal(12.5)));
    assert_eq!(height(150), None);
    assert_eq!(height(250), None);
    // A matched or held fix keeps its own height.
    assert_eq!(height(200), Some(Height::Orthometric(30.0)));
    let nearest = options().with_interpolation(Interpolation::Nearest);
    let held = track.locate(at(190), &nearest).unwrap().unwrap();
    assert_eq!(held.coord.height(), Some(Height::Orthometric(30.0)));
}

#[test]
fn accuracy_grows_with_time_and_stays_bound_only_between_bounds() {
    let a = with_accuracy(fix(0, 0.0, 0.0), AccuracyKind::Bound, 4.0);
    let b = with_accuracy(fix(100, 0.0, 0.01), AccuracyKind::Bound, 10.0);
    let slow = options().with_max_speed_mps(1.0);
    let track = Track::new(vec![a.clone(), b]).unwrap();
    let segment = geodesic_distance(a.coord, track.fixes()[1].coord)
        .unwrap()
        .meters();

    let near_a = track.locate(at(20), &slow).unwrap().unwrap();
    let accuracy = near_a.accuracy.unwrap();
    assert_eq!(accuracy.kind(), AccuracyKind::Bound);
    assert_eq!(accuracy.vertical(), None);
    // From a: 4 m + 20 s at 1 m/s + a fifth of the segment; from b it is larger.
    let expected = 4.0 + 20.0 + 0.2 * segment;
    assert!(
        (horizontal(&near_a) - expected).abs() < 1e-6,
        "{}",
        horizontal(&near_a)
    );
    // Further from both fixes, the bound is wider.
    let middle = track.locate(at(50), &slow).unwrap().unwrap();
    assert!(horizontal(&middle) > horizontal(&near_a));
    let near_b = track.locate(at(95), &slow).unwrap().unwrap();
    let expected = 10.0 + 5.0 + 0.05 * segment;
    assert!(
        (horizontal(&near_b) - expected).abs() < 1e-6,
        "{}",
        horizontal(&near_b)
    );

    // One end that is not a hard bound makes the result unspecified.
    for other in [
        with_accuracy(fix(100, 0.0, 0.01), AccuracyKind::OneSigma, 10.0),
        fix(100, 0.0, 0.01),
    ] {
        let track = Track::new(vec![a.clone(), other]).unwrap();
        let fix = track.locate(at(20), &slow).unwrap().unwrap();
        assert_eq!(fix.accuracy.unwrap().kind(), AccuracyKind::Unspecified);
    }
    // A held end alone decides.
    let reach = slow.with_max_extrapolation(Duration::from_secs(10));
    let held = Track::new(vec![a])
        .unwrap()
        .locate(at(-10), &reach)
        .unwrap()
        .unwrap();
    assert_eq!(held.accuracy.unwrap().kind(), AccuracyKind::Bound);
    assert!((horizontal(&held) - 14.0).abs() < 1e-12);
}

#[test]
fn a_vertical_only_accuracy_counts_as_no_horizontal_magnitude() {
    let mut a = fix(0, 0.0, 0.0);
    a.accuracy =
        Some(Accuracy::new(AccuracyKind::Bound, None, Some(Length::from_meters(3.0))).unwrap());
    let track = Track::new(vec![a]).unwrap();
    let fix = track.locate(at(0), &options()).unwrap().unwrap();
    assert_eq!(fix.accuracy.unwrap().kind(), AccuracyKind::Unspecified);
    assert_eq!(horizontal(&fix), 0.0);
}

#[test]
fn an_absurd_speed_leaves_no_magnitude() {
    let track = Track::new(vec![fix(0, 0.0, 0.0)]).unwrap();
    let options = options()
        .with_max_speed_mps(f64::MAX)
        .with_max_extrapolation(Duration::from_secs(100));
    let fix = track.locate(at(50), &options).unwrap().unwrap();
    assert_eq!(fix.accuracy, None);
}

#[test]
fn an_invalid_speed_is_refused() {
    let track = equator();
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            track.locate(at(50), &options().with_max_speed_mps(bad)),
            Err(Error::InvalidValue {
                field: "max_speed_mps",
                ..
            })
        ));
    }
    assert!(
        track
            .locate(at(50), &options().with_max_speed_mps(0.0))
            .is_ok()
    );
}

#[test]
fn the_clock_offset_moves_the_photo_time() {
    let track = equator();
    // A camera 25 s slow: its 100 s is 125 s on the track.
    let slow = options().with_clock_offset_ms(25_000);
    let fix = track.locate(at(100), &slow).unwrap().unwrap();
    assert_eq!(fix.timestamp, Some(at(125)));
    assert!((fix.coord.lon() - 0.0125).abs() < 1e-12);
    assert_eq!(
        fix.source.unwrap().notes,
        ["clock offset of +25000 ms applied to the photo time"]
    );
    // A camera an hour fast.
    let fast = options().with_clock_offset_ms(-3_600_000);
    let fix = track.locate(at(3_650), &fast).unwrap().unwrap();
    assert_eq!(fix.timestamp, Some(at(50)));
    // Sub-second offsets are kept.
    let fix = track
        .locate(at(50), &options().with_clock_offset_ms(500))
        .unwrap()
        .unwrap();
    assert_eq!(fix.timestamp, Some(at(50) + Duration::from_millis(500)));
    // An offset that leaves the clock's range has no position.
    let far = options().with_clock_offset_ms(i64::MAX);
    let latest = (0..64).rev().fold(UNIX_EPOCH, |t, bit| {
        t.checked_add(Duration::from_secs(1 << bit)).unwrap_or(t)
    });
    assert_eq!(track.locate(latest, &far).unwrap(), None);
}

#[test]
fn times_before_the_unix_epoch_work() {
    let track = Track::new(vec![fix(-200, 0.0, 0.0), fix(-100, 0.0, 0.01)]).unwrap();
    let fix = track.locate(at(-150), &options()).unwrap().unwrap();
    assert!((fix.coord.lon() - 0.005).abs() < 1e-12);
    assert_eq!(
        raw(&fix),
        "geotag:interpolated t0=-200.000 t1=-100.000 f=0.500"
    );
}

#[test]
fn a_datum_ambiguity_is_carried_over() {
    let mut flagged = fix(100, 39.9, 116.4);
    let mut source = RawSource::new("exif", Confidence::new(0.8).unwrap());
    source.datum_ambiguity = Some(DatumAmbiguity::PossiblyGcj02);
    flagged.source = Some(source);
    let track = Track::new(vec![fix(0, 39.9, 116.39), flagged]).unwrap();
    let fix = track.locate(at(50), &options()).unwrap().unwrap();
    assert_eq!(
        fix.source.unwrap().datum_ambiguity,
        Some(DatumAmbiguity::PossiblyGcj02)
    );
    let clean = track.locate(at(0), &options()).unwrap().unwrap();
    assert_eq!(clean.source.unwrap().datum_ambiguity, None);
}

#[test]
fn a_classic_datum_track_interpolates_on_its_own_ellipsoid() {
    let tokyo = |secs, lon| {
        let mut f = fix(secs, 35.0, lon);
        f.coord = Coordinate::new(35.0, lon, Crs::Tokyo).unwrap();
        f
    };
    let track = Track::new(vec![tokyo(0, 139.0), tokyo(100, 139.01)]).unwrap();
    let fix = track.locate(at(50), &options()).unwrap().unwrap();
    assert_eq!(fix.coord.crs(), Crs::Tokyo);
    let expected = intermediate(track.fixes()[0].coord, track.fixes()[1].coord, 0.5).unwrap();
    assert_eq!(fix.coord, expected);
}

#[test]
fn estimate_offset_finds_the_pass_and_takes_the_median() {
    let track = equator();
    // Each reference's camera time runs 600 s ahead of the track.
    let reference =
        |track_secs: i64, lon: f64| (at(track_secs + 600), Coordinate::wgs84(0.0, lon).unwrap());
    assert_eq!(
        estimate_offset(&track, &[reference(150, 0.015)]).unwrap(),
        -600_000
    );
    // On a fix, and off to the side of the track.
    assert_eq!(
        estimate_offset(&track, &[reference(100, 0.01)]).unwrap(),
        -600_000
    );
    let beside = (at(650), Coordinate::wgs84(0.001, 0.005).unwrap());
    assert_eq!(estimate_offset(&track, &[beside]).unwrap(), -600_000);
    // One wrong reference among three does not move the median.
    let wrong = (at(10_000), Coordinate::wgs84(0.0, 0.01).unwrap());
    let refs = [reference(20, 0.002), wrong, reference(180, 0.018)];
    assert_eq!(estimate_offset(&track, &refs).unwrap(), -600_000);
    // An even count averages the middle two.
    let refs = [
        reference(20, 0.002),
        (at(1_120), Coordinate::wgs84(0.0, 0.002).unwrap()),
    ];
    assert_eq!(estimate_offset(&track, &refs).unwrap(), -850_000);
    // Beyond the ends, the nearest end's time.
    let past = (at(5_000), Coordinate::wgs84(0.0, 0.05).unwrap());
    assert_eq!(estimate_offset(&track, &[past]).unwrap(), -4_800_000);
}

#[test]
fn estimate_offset_prefers_the_pass_nearest_the_photo_time() {
    // Out and back: the track passes 0.01° E at 100 s and again at 300 s.
    let track = Track::new(vec![
        fix(0, 0.0, 0.0),
        fix(100, 0.0, 0.01),
        fix(200, 0.0, 0.02),
        fix(300, 0.0, 0.01),
    ])
    .unwrap();
    let place = Coordinate::wgs84(0.0, 0.01).unwrap();
    assert_eq!(
        estimate_offset(&track, &[(at(110), place)]).unwrap(),
        -10_000
    );
    assert_eq!(
        estimate_offset(&track, &[(at(290), place)]).unwrap(),
        10_000
    );
    // Midway between the two passes, the earlier; a farther fix at the photo
    // time itself never wins on time alone.
    assert_eq!(
        estimate_offset(&track, &[(at(200), place)]).unwrap(),
        -100_000
    );
}

#[test]
fn estimate_offset_handles_a_stationary_segment_and_a_pole() {
    let track = Track::new(vec![
        fix(0, 0.0, 0.0),
        fix(100, 0.0, 0.0),
        fix(200, 0.0, 10.0),
    ])
    .unwrap();
    // The pole has no foot on the equator; the nearest fix decides.
    let pole = Coordinate::wgs84(90.0, 0.0).unwrap();
    assert_eq!(estimate_offset(&track, &[(at(50), pole)]).unwrap(), -50_000);
}

#[test]
fn estimate_offset_finds_the_foot_of_a_sub_meter_segment() {
    // Two fixes 0.45 m apart, and a place 1.1 m off the middle of them: the
    // foot is nearer than either end, however short the segment.
    let track = Track::new(vec![fix(0, 0.0, 0.0), fix(100, 0.0, 0.000_004)]).unwrap();
    let beside = Coordinate::wgs84(0.000_01, 0.000_002).unwrap();
    assert_eq!(estimate_offset(&track, &[(at(50), beside)]).unwrap(), 0);
    assert_eq!(
        estimate_offset(&track, &[(at(1_050), beside)]).unwrap(),
        -1_000_000
    );
}

#[test]
fn estimate_offset_refuses_what_it_cannot_match() {
    let track = equator();
    assert!(matches!(
        estimate_offset(&track, &[]),
        Err(Error::InvalidValue {
            field: "references",
            ..
        })
    ));
    let place = Coordinate::wgs84(0.0, 0.0).unwrap();
    assert!(matches!(
        estimate_offset(&Track::new(vec![]).unwrap(), &[(at(0), place)]),
        Err(Error::InvalidValue { field: "track", .. })
    ));
    let gcj = Coordinate::gcj02(0.0, 0.0).unwrap();
    assert_eq!(
        estimate_offset(&track, &[(at(0), gcj)]),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02
        })
    );
}

#[cfg(feature = "serde")]
#[test]
fn options_round_trip_through_serde_and_fill_defaults() {
    let options = GeotagOptions::default()
        .with_clock_offset_ms(-1_500)
        .with_max_gap(Duration::from_secs(60))
        .with_interpolation(Interpolation::Nearest);
    let json = serde_json::to_string(&options).unwrap();
    assert_eq!(
        serde_json::from_str::<GeotagOptions>(&json).unwrap(),
        options
    );
    let partial: GeotagOptions = serde_json::from_str(r#"{"clock_offset_ms":7}"#).unwrap();
    assert_eq!(partial, GeotagOptions::default().with_clock_offset_ms(7));
    assert!(serde_json::from_str::<Interpolation>(r#""Linear""#).is_err());
}
