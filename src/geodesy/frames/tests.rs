//! Behaviour of the local frames: directions, round trips, datums, and serde.

use super::*;
use crate::Crs;
use crate::test_support::assert_close;

fn enu(east_m: f64, north_m: f64, up_m: f64) -> Enu {
    Enu::new(east_m, north_m, up_m).unwrap()
}

fn aer(azimuth_deg: f64, elevation_deg: f64, range_m: f64) -> Aer {
    Aer::new(azimuth_deg, elevation_deg, Length::from_meters(range_m)).unwrap()
}

#[test]
fn enu_directions() {
    let origin = Coordinate::wgs84(40.0, -75.0).unwrap();
    // A point due north of the origin has +north, ~0 east.
    let north = Enu::try_from_coordinate(Coordinate::wgs84(40.1, -75.0).unwrap(), origin).unwrap();
    assert!(north.north_m() > 10_000.0);
    assert_close(north.east_m(), 0.0, 1e-6);
    // A point due east is predominantly east; the small north component is
    // the parallel's poleward curvature (~5 m over ~8.5 km).
    let east = Enu::try_from_coordinate(Coordinate::wgs84(40.0, -74.9).unwrap(), origin).unwrap();
    assert!(east.east_m() > 8_000.0);
    assert!(east.north_m().abs() < 50.0);
}

#[test]
fn enu_round_trip() {
    let origin = Coordinate::wgs84(40.7128, -74.006).unwrap();
    let target = Coordinate::wgs84(40.73, -74.0)
        .unwrap()
        .with_height(Height::Ellipsoidal(120.0))
        .unwrap();
    let back = Enu::try_from_coordinate(target, origin)
        .unwrap()
        .try_to_coordinate(origin)
        .unwrap();
    assert_close(back.lat(), 40.73, 1e-9);
    assert_close(back.lon(), -74.0, 1e-9);
    match back.height() {
        Some(Height::Ellipsoidal(h)) => assert_close(h, 120.0, 1e-4),
        other => panic!("expected ellipsoidal height, got {other:?}"),
    }
}

/// A NAD27 pair round-trips through every frame on the Clarke 1866
/// ellipsoid, and comes back tagged NAD27.
#[test]
fn nad27_round_trips_through_enu_ned_and_aer() {
    let origin = Coordinate::new(39.0, -98.0, Crs::Nad27).unwrap();
    let target = Coordinate::new(39.05, -97.93, Crs::Nad27)
        .unwrap()
        .with_height(Height::Ellipsoidal(350.0))
        .unwrap();
    let e = Enu::try_from_coordinate(target, origin).unwrap();
    let n = Ned::try_from_coordinate(target, origin).unwrap();
    let a = Aer::try_from_coordinate(target, origin).unwrap();
    assert_eq!(n, e.to_ned());
    assert_eq!(a, e.to_aer());
    for back in [
        e.try_to_coordinate(origin).unwrap(),
        n.try_to_coordinate(origin).unwrap(),
        a.try_to_coordinate(origin).unwrap(),
    ] {
        assert_eq!(back.crs(), Crs::Nad27);
        assert_close(back.lat(), 39.05, 1e-9);
        assert_close(back.lon(), -97.93, 1e-9);
        match back.height() {
            Some(Height::Ellipsoidal(h)) => assert_close(h, 350.0, 1e-4),
            other => panic!("expected ellipsoidal height, got {other:?}"),
        }
    }
    // The offset is measured on Clarke 1866, not WGS-84: the same numbers on
    // WGS-84 give a different offset.
    let wgs = Enu::try_from_coordinate(
        Coordinate::wgs84(39.05, -97.93)
            .unwrap()
            .with_height(Height::Ellipsoidal(350.0))
            .unwrap(),
        Coordinate::wgs84(39.0, -98.0).unwrap(),
    )
    .unwrap();
    assert!((wgs.north_m() - e.north_m()).abs() > 1e-3);
}

#[test]
fn a_datum_mismatch_names_the_origin_datum() {
    let nad27 = Coordinate::new(39.0, -98.0, Crs::Nad27).unwrap();
    let tokyo = Coordinate::new(39.0, -98.0, Crs::Tokyo).unwrap();
    let mismatch = Err(Error::CrsMismatch {
        expected: Crs::Tokyo,
        found: Crs::Nad27,
    });
    assert_eq!(Enu::try_from_coordinate(nad27, tokyo), mismatch);
    assert_eq!(
        Ned::try_from_coordinate(nad27, tokyo),
        mismatch.clone().map(|e: Enu| e.to_ned())
    );
    assert_eq!(
        Aer::try_from_coordinate(nad27, tokyo),
        mismatch.map(|e: Enu| e.to_aer())
    );
}

#[test]
fn enu_to_aer_reference() {
    let north = enu(0.0, 100.0, 0.0).to_aer();
    assert_close(north.azimuth_deg(), 0.0, 1e-9);
    assert_close(north.elevation_deg(), 0.0, 1e-9);
    assert_close(north.range().meters(), 100.0, 1e-9);

    assert_close(enu(100.0, 0.0, 0.0).to_aer().azimuth_deg(), 90.0, 1e-9);

    let up = enu(0.0, 0.0, 100.0).to_aer();
    assert_close(up.elevation_deg(), 90.0, 1e-9);
    assert_close(up.range().meters(), 100.0, 1e-9);
}

#[test]
fn aer_to_enu_reference() {
    let e = aer(45.0, 0.0, 100.0).to_enu();
    let component = 100.0 * 45.0_f64.to_radians().sin();
    assert_close(e.east_m(), component, 1e-9);
    assert_close(e.north_m(), component, 1e-9);
    assert_close(e.up_m(), 0.0, 1e-9);
}

#[test]
fn aer_enu_round_trip() {
    let back = aer(120.0, 30.0, 5000.0).to_enu().to_aer();
    assert_close(back.azimuth_deg(), 120.0, 1e-9);
    assert_close(back.elevation_deg(), 30.0, 1e-9);
    assert_close(back.range().meters(), 5000.0, 1e-9);
}

#[test]
fn enu_ned_repackaging() {
    let ned = enu(1.0, 2.0, 3.0).to_ned();
    assert_eq!(
        (ned.north_m(), ned.east_m(), ned.down_m()),
        (2.0, 1.0, -3.0)
    );
    assert_eq!(ned.to_enu(), enu(1.0, 2.0, 3.0));
    assert_eq!(
        aer(30.0, 10.0, 7.0).to_ned(),
        aer(30.0, 10.0, 7.0).to_enu().to_ned()
    );
}

#[test]
fn zero_enu_aer_is_well_defined() {
    // A zero offset has zero range and a defined (not NaN) elevation.
    let a = enu(0.0, 0.0, 0.0).to_aer();
    assert_close(a.range().meters(), 0.0, 0.0);
    assert_close(a.elevation_deg(), 0.0, 0.0);
}

/// N4: elevation and range hold where squaring the components would
/// underflow or overflow, and elevation is never NaN.
#[test]
fn aer_survives_extreme_magnitudes() {
    for up in [1e-200, 1e200, -1e-200, -1e200] {
        let a = enu(0.0, 0.0, up).to_aer();
        assert_close(a.elevation_deg(), 90.0_f64.copysign(up), 0.0);
        assert_close(a.range().meters(), up.abs(), 0.0);
    }
    let flat = enu(3e200, 4e200, 0.0).to_aer();
    assert_close(flat.range().meters(), 5e200, 1e186);
    assert_close(flat.elevation_deg(), 0.0, 0.0);
    // A sweep of ordinary offsets never produces a NaN elevation, and
    // matches asin(up / range) wherever that is defined.
    for i in 1..2000 {
        let x = f64::from(i) * 0.123_456_789;
        let e = enu(x.sin() * 1e-3, x.cos() * 1e-3, x);
        let a = e.to_aer();
        assert!(a.elevation_deg().is_finite(), "{e:?}");
        let r = a.range().meters();
        assert_close(
            a.elevation_deg(),
            (e.up_m() / r).min(1.0).asin().to_degrees(),
            1e-6,
        );
    }
}

#[test]
fn coordinate_frames_reject_china_systems_and_orthometric_heights() {
    let origin = Coordinate::wgs84(40.0, -75.0).unwrap();
    let gcj = Coordinate::gcj02(40.0, -75.0).unwrap();
    let mismatch = Err(Error::CrsMismatch {
        expected: Crs::Wgs84,
        found: Crs::Gcj02,
    });
    // GCJ-02 is rejected even when both points share it, as `Ecef` does.
    assert_eq!(Enu::try_from_coordinate(gcj, gcj), mismatch);
    assert_eq!(Enu::try_from_coordinate(gcj, origin), mismatch);
    let bd09 = Coordinate::bd09(40.0, -75.0).unwrap();
    assert_eq!(
        enu(0.0, 0.0, 0.0).try_to_coordinate(bd09),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Bd09,
        })
    );
    let orthometric = origin.with_height(Height::Orthometric(10.0)).unwrap();
    let height = Err(Error::InvalidValue {
        field: "height",
        detail: "local frames require ellipsoidal height".into(),
    });
    assert_eq!(Enu::try_from_coordinate(origin, orthometric), height);
    assert_eq!(Enu::try_from_coordinate(orthometric, origin), height);
    assert_eq!(
        enu(0.0, 0.0, 0.0).try_to_coordinate(orthometric),
        height.map(|_: Enu| origin)
    );
}

/// Heights so large that the ECEF difference overflows are reported against
/// the height, not as an invalid ENU component.
#[test]
fn an_overflowing_offset_is_a_height_error() {
    let up = Coordinate::wgs84(0.0, 0.0)
        .unwrap()
        .with_height(Height::Ellipsoidal(f64::MAX))
        .unwrap();
    let down = Coordinate::wgs84(0.0, 180.0)
        .unwrap()
        .with_height(Height::Ellipsoidal(f64::MAX))
        .unwrap();
    assert!(matches!(
        Enu::try_from_coordinate(up, down),
        Err(Error::InvalidValue {
            field: "height",
            ..
        })
    ));
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_rejects_invalid_values() {
    let e = enu(1.0, -2.0, 3.5);
    let json = serde_json::to_string(&e).unwrap();
    assert_eq!(json, r#"{"east_m":1.0,"north_m":-2.0,"up_m":3.5}"#);
    assert_eq!(serde_json::from_str::<Enu>(&json).unwrap(), e);
    let n = e.to_ned();
    let json = serde_json::to_string(&n).unwrap();
    assert_eq!(serde_json::from_str::<Ned>(&json).unwrap(), n);
    let a = aer(30.0, 10.0, 7.0);
    let json = serde_json::to_string(&a).unwrap();
    assert_eq!(serde_json::from_str::<Aer>(&json).unwrap(), a);

    // An out-of-range literal never becomes a value.
    assert!(serde_json::from_str::<Enu>(r#"{"east_m":1e999,"north_m":0.0,"up_m":0.0}"#).is_err());
    assert!(
        serde_json::from_str::<Ned>(r#"{"north_m":0.0,"east_m":0.0,"down_m":-1e999}"#).is_err()
    );
    let bad_elevation = json.replace("10.0", "91.0");
    assert!(serde_json::from_str::<Aer>(&bad_elevation).is_err());
    let bad_range = json.replace("7.0", "-7.0");
    assert!(serde_json::from_str::<Aer>(&bad_range).is_err());
    // An azimuth out of [0, 360) is normalized, as `Aer::new` does.
    let wrapped: Aer = serde_json::from_str(&json.replace("30.0", "390.0")).unwrap();
    assert_close(wrapped.azimuth_deg(), 30.0, 1e-12);
}
