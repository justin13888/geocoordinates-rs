//! The GPX reader: waypoints, tracks and routes, time and HDOP metadata, and
//! indexed rejections.

use super::*;

const GPX_HEAD: &str = r#"<?xml version="1.0"?>
    <gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">"#;

#[test]
fn gpx_waypoint_and_track() {
    let doc = format!(
        r#"{GPX_HEAD}
          <wpt lat="48.2" lon="16.3"><ele>183.0</ele></wpt>
          <trk><trkseg>
            <trkpt lat="40.0" lon="-75.0"/>
            <trkpt lat="41.0" lon="-74.0"/>
          </trkseg></trk>
        </gpx>"#
    );
    let fixes = parse_gpx(&doc).unwrap();
    assert_eq!(fixes.len(), 3); // 1 waypoint + 2 track points
    assert_close(fixes[0].coord.lat(), 48.2, 1e-9);
    assert_close(fixes[0].coord.lon(), 16.3, 1e-9);
    // <ele> is mean-sea-level elevation.
    assert_eq!(fixes[0].coord.height(), Some(Height::Orthometric(183.0)));
    // GPX labels its axes with attributes, so no listing order is recorded.
    assert!(fixes.iter().all(|f| source(f).axis_order.is_none()));
    assert_eq!(
        source(&fixes[0]).raw,
        r#"<wpt lat="48.2" lon="16.3"><ele>183</ele></wpt>"#
    );
    assert_eq!(source(&fixes[2]).raw, r#"<trkpt lat="41" lon="-74"/>"#);
    assert_close(fixes[2].coord.lat(), 41.0, 1e-9);
    // A point without <time> or <hdop> carries neither.
    assert_eq!((fixes[1].timestamp, fixes[1].accuracy), (None, None));
}

#[test]
fn gpx_keeps_time_and_hdop() {
    use std::time::{Duration, SystemTime};

    let doc = format!(
        r#"{GPX_HEAD}<trk><trkseg>
          <trkpt lat="1" lon="2"><time>2024-03-01T12:00:05Z</time><hdop>1.5</hdop></trkpt>
          <trkpt lat="3" lon="4"><time>2024-03-01T14:00:05+02:00</time></trkpt>
        </trkseg></trk></gpx>"#
    );
    let fixes = parse_gpx(&doc).unwrap();
    let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_709_294_405);
    assert_eq!(fixes[0].timestamp, Some(expected));
    // An offset is honoured: 14:00:05+02:00 is the same instant.
    assert_eq!(fixes[1].timestamp, Some(expected));
    // HDOP × 5 m nominal UERE, the NMEA GGA scaling: a one-sigma estimate.
    let accuracy = fixes[0].accuracy.unwrap();
    assert_eq!(accuracy.kind(), crate::fix::AccuracyKind::OneSigma);
    assert_close(accuracy.horizontal().unwrap().meters(), 7.5, 1e-12);
    assert_eq!(accuracy.vertical(), None);
    assert_eq!(fixes[1].accuracy, None);
}

#[test]
fn gpx_pre_epoch_time_converts_without_panicking() {
    use std::time::{Duration, SystemTime};

    let doc = format!(
        r#"{GPX_HEAD}
          <wpt lat="1" lon="2"><time>1969-12-31T23:59:58.25Z</time></wpt>
          <wpt lat="3" lon="4"><time>1970-01-01T00:00:00Z</time></wpt>
        </gpx>"#
    );
    let fixes = parse_gpx(&doc).unwrap();
    // A native SystemTime is signed, so the pre-epoch instant is kept exactly
    // and no note is left; where SystemTime cannot hold it (wasm32), the
    // checked conversion leaves the timestamp `None` with a note instead of
    // panicking (pinned by the web bindings' test).
    let before = SystemTime::UNIX_EPOCH - Duration::from_millis(1_750);
    assert_eq!(fixes[0].timestamp, Some(before));
    assert!(source(&fixes[0]).notes.is_empty());
    assert_eq!(fixes[1].timestamp, Some(SystemTime::UNIX_EPOCH));
}

#[test]
fn gpx_emits_waypoints_then_tracks_then_routes() {
    let doc = format!(
        r#"{GPX_HEAD}
          <rte>
            <rtept lat="10.0" lon="20.0"/>
            <rtept lat="11.0" lon="21.0"><ele>5.5</ele></rtept>
          </rte>
          <trk><trkseg><trkpt lat="1.0" lon="2.0"/></trkseg>
               <trkseg><trkpt lat="3.0" lon="4.0"/></trkseg></trk>
          <wpt lat="-1.0" lon="-2.0"/>
        </gpx>"#
    );
    let fixes = parse_gpx(&doc).unwrap();
    assert_eq!(
        lat_lons(&fixes),
        [
            (-1.0, -2.0),
            (1.0, 2.0),
            (3.0, 4.0),
            (10.0, 20.0),
            (11.0, 21.0)
        ]
    );
    assert_eq!(fixes[4].coord.height(), Some(Height::Orthometric(5.5)));
    assert_eq!(fixes[3].coord.height(), None);
    let tags: Vec<_> = fixes
        .iter()
        .map(|f| source(f).raw.split(' ').next().unwrap().to_string())
        .collect();
    assert_eq!(tags, ["<wpt", "<trkpt", "<trkpt", "<rtept", "<rtept"]);
    assert_eq!(
        source(&fixes[4]).raw,
        r#"<rtept lat="11" lon="21"><ele>5.5</ele></rtept>"#
    );
    // A document with no points has no fixes.
    assert!(parse_gpx(&format!("{GPX_HEAD}</gpx>")).unwrap().is_empty());
}

#[test]
fn gpx_rejections_are_parse_errors() {
    assert_parse_err!(parse_gpx("<gpx>broken"), "gpx: ");
    let negative_hdop = format!(
        r#"{GPX_HEAD}<wpt lat="1" lon="2"/><wpt lat="1" lon="2"><hdop>-1</hdop></wpt></gpx>"#
    );
    let infinite_hdop = format!(r#"{GPX_HEAD}<wpt lat="1" lon="2"><hdop>inf</hdop></wpt></gpx>"#);
    // A negative or non-finite HDOP is named as such; a finite one whose
    // `× 5 m` accuracy overflows names the scaled range instead.
    let huge_hdop = format!(r#"{GPX_HEAD}<wpt lat="1" lon="2"><hdop>1e308</hdop></wpt></gpx>"#);
    let finite_msg = "hdop must be finite and nonnegative";
    for (doc, index, bad_hdop) in [
        (&negative_hdop, 1, true),
        (&infinite_hdop, 0, true),
        (&huge_hdop, 0, false),
    ] {
        let r = parse_gpx(doc);
        let prefix = format!("gpx: invalid position at index {index}: ");
        let ok = matches!(&r, Err(Error::Parse { kind: ParseErrorKind::Syntax, detail: m })
            if m.starts_with(&prefix)
            && m.contains(finite_msg) == bad_hdop && m.contains("out of range") != bad_hdop);
        assert!(ok, "{r:?}");
    }
    // A zero HDOP is valid, and scales to a zero radius.
    let zero_hdop = format!(r#"{GPX_HEAD}<wpt lat="1" lon="2"><hdop>0</hdop></wpt></gpx>"#);
    let accuracy = parse_gpx(&zero_hdop).unwrap()[0].accuracy.unwrap();
    assert_eq!(accuracy.horizontal(), Some(crate::Length::ZERO));
    // The gpx reader range-checks attributes itself, before any index exists.
    let off_globe = format!(r#"{GPX_HEAD}<wpt lat="1" lon="2"/><wpt lat="1" lon="200"/></gpx>"#);
    assert_parse_err!(parse_gpx(&off_globe), "gpx: ");
}
