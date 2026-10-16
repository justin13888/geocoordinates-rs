//! Calls the China datum validation and `Coordinate` bridge exports, and pins
//! the domain edges the core crate enforces: the BD-09 wrap at the
//! antimeridian, the BD-09 pole guards, and Baidu Mercator's ±74° limit.

use geocoordinates_ffi::*;

fn close(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() <= tol,
        "expected {expected} ± {tol}, got {actual}"
    );
}

#[test]
fn datum_records_validate_like_their_core_constructors() {
    assert!(
        wgs84_validate(Wgs84 {
            lat: 39.9,
            lon: 116.4
        })
        .is_ok()
    );
    assert!(
        gcj02_validate(Gcj02 {
            lat: -90.0,
            lon: 180.0
        })
        .is_ok()
    );
    assert!(matches!(
        wgs84_validate(Wgs84 {
            lat: 91.0,
            lon: 0.0
        }),
        Err(GeoError::OutOfRange { .. })
    ));
    assert!(matches!(
        gcj02_validate(Gcj02 {
            lat: 0.0,
            lon: 180.5
        }),
        Err(GeoError::OutOfRange { .. })
    ));
    assert!(matches!(
        bd09_validate(Bd09 {
            lat: f64::NAN,
            lon: 0.0
        }),
        Err(GeoError::OutOfRange { .. })
    ));
    assert!(bd09_validate(Bd09 { lat: 0.0, lon: 0.0 }).is_ok());
    // Every conversion validates its record first.
    assert!(matches!(
        wgs84_to_gcj02(Wgs84 {
            lat: 91.0,
            lon: 0.0
        }),
        Err(GeoError::OutOfRange { .. })
    ));
}

#[test]
fn baidu_mercator_records_must_be_finite() {
    assert!(baidu_mercator_validate(BaiduMercator { x: 1.0, y: 2.0 }).is_ok());
    match baidu_mercator_validate(BaiduMercator {
        x: 0.0,
        y: f64::INFINITY,
    }) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "y"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
    match baidu_mercator_to_bd09(BaiduMercator {
        x: 0.0,
        y: 13_000_000.0,
    }) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "y"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn datum_records_bridge_to_and_from_coordinate() {
    let w = wgs84_to_coordinate(Wgs84 { lat: 1.0, lon: 2.0 }).unwrap();
    assert_eq!(
        (w.lat, w.lon, w.crs, w.height),
        (1.0, 2.0, Crs::Wgs84, None)
    );
    let g = gcj02_to_coordinate(Gcj02 { lat: 1.0, lon: 2.0 }).unwrap();
    assert_eq!(g.crs, Crs::Gcj02);
    let b = bd09_to_coordinate(Bd09 { lat: 1.0, lon: 2.0 }).unwrap();
    assert_eq!(b.crs, Crs::Bd09);

    assert_eq!(
        wgs84_from_coordinate(w).unwrap(),
        Wgs84 { lat: 1.0, lon: 2.0 }
    );
    assert_eq!(
        gcj02_from_coordinate(g).unwrap(),
        Gcj02 { lat: 1.0, lon: 2.0 }
    );
    assert_eq!(
        bd09_from_coordinate(b).unwrap(),
        Bd09 { lat: 1.0, lon: 2.0 }
    );

    // A datum is never laundered: the wrong CRS is a mismatch.
    match wgs84_from_coordinate(g) {
        Err(GeoError::CrsMismatch { expected, found }) => {
            assert_eq!((expected, found), (Crs::Wgs84, Crs::Gcj02));
        }
        other => panic!("expected CrsMismatch, got {other:?}"),
    }
    assert!(matches!(
        bd09_to_coordinate(Bd09 {
            lat: 0.0,
            lon: -181.0
        }),
        Err(GeoError::OutOfRange { .. })
    ));
}

#[test]
fn bd09_wraps_at_the_antimeridian_and_guards_the_poles() {
    let b = gcj02_to_bd09(Gcj02 {
        lat: 0.0,
        lon: 180.0,
    })
    .unwrap();
    close(b.lon, -179.9935, 1e-3);
    let back = bd09_to_gcj02_refined(Bd09 {
        lat: 0.0,
        lon: -180.0,
    })
    .unwrap();
    close(back.lon, 179.9935, 1e-3);
    assert_eq!(back.max_error_m, 0.5);
    let fast = bd09_to_gcj02_fast(Bd09 {
        lat: 0.0,
        lon: -180.0,
    })
    .unwrap();
    assert_eq!(fast.max_error_m, 125.0);

    assert!(matches!(
        gcj02_to_bd09(Gcj02 {
            lat: 90.0,
            lon: 0.0
        }),
        Err(GeoError::OutsideDomain { .. })
    ));
    assert!(matches!(
        bd09_to_wgs84_refined(Bd09 {
            lat: -90.0,
            lon: 0.0
        }),
        Err(GeoError::OutsideDomain { .. })
    ));
}

#[test]
fn refined_gcj02_inverse_reports_an_unconverged_residual() {
    let r = gcj02_to_wgs84_refined(Gcj02 {
        lat: 40.0,
        lon: 72.006,
    })
    .unwrap();
    assert!(r.max_error_m > 100.0, "{}", r.max_error_m);
}

#[test]
fn fast_gcj02_inverse_measures_its_residual_at_the_box_edge() {
    let edge = Gcj02 {
        lat: 40.0,
        lon: 72.006,
    };
    let r = gcj02_to_wgs84_fast(edge).unwrap();
    assert!(r.max_error_m > 300.0, "{}", r.max_error_m);
    let fast = bd09_to_wgs84_fast(gcj02_to_bd09(edge).unwrap()).unwrap();
    assert!(fast.max_error_m > 300.0, "{}", fast.max_error_m);
}

#[test]
fn bd09_fast_wgs84_inverse_sums_its_legs() {
    let r = bd09_to_wgs84_fast(Bd09 {
        lat: 39.922_700,
        lon: 116.416_627,
    })
    .unwrap();
    assert_eq!(r.max_error_m, 6.5);
    close(r.lat, 39.915, 1e-4);
    close(r.lon, 116.404, 1e-4);
    assert!(matches!(
        bd09_to_wgs84_fast(Bd09 {
            lat: -90.0,
            lon: 0.0
        }),
        Err(GeoError::OutsideDomain { .. })
    ));
    assert!(matches!(
        bd09_to_wgs84_fast(Bd09 {
            lat: 91.0,
            lon: 0.0
        }),
        Err(GeoError::OutOfRange { .. })
    ));
}

#[test]
fn baidu_mercator_is_defined_only_within_74_degrees() {
    assert!(matches!(
        baidu_mercator_from_bd09(Bd09 {
            lat: 80.0,
            lon: 0.0
        }),
        Err(GeoError::OutsideDomain { .. })
    ));
    assert!(
        baidu_mercator_from_bd09(Bd09 {
            lat: 74.0,
            lon: 0.0
        })
        .is_ok()
    );
}
