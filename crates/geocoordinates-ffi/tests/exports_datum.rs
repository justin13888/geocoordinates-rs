//! Calls every Helmert / datum-transform FFI export at least once with an
//! asserted value. The rest of geodesy is in `exports_geodesy.rs`.
//!
//! Values are pinned to what the core crate produces today (the NIMA
//! TR8350.2 catalogue shifts), so a change in either the mirror or the core
//! surfaces here as a failing assertion.

use geocoordinates_ffi::*;

fn close(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() <= tol,
        "expected {expected} ± {tol}, got {actual}"
    );
}

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

#[test]
fn helmert_identity_inverse_and_apply() {
    let identity = helmert_identity();
    assert_eq!(
        identity,
        Helmert {
            tx_m: 0.0,
            ty_m: 0.0,
            tz_m: 0.0,
            rx_arcsec: 0.0,
            ry_arcsec: 0.0,
            rz_arcsec: 0.0,
            scale_ppm: 0.0,
        }
    );
    let shift = Helmert {
        tx_m: 1.0,
        ty_m: 2.0,
        tz_m: 3.0,
        ..identity
    };
    let moved = helmert_apply_ecef(
        shift,
        Ecef {
            x_m: 10.0,
            y_m: 20.0,
            z_m: 30.0,
        },
    )
    .unwrap();
    assert_eq!(
        moved,
        Ecef {
            x_m: 11.0,
            y_m: 22.0,
            z_m: 33.0,
        }
    );
    let inverse = helmert_inverse_first_order(shift);
    assert_eq!(
        (inverse.tx_m, inverse.ty_m, inverse.tz_m),
        (-1.0, -2.0, -3.0)
    );
    assert_eq!(
        helmert_apply_ecef(inverse, moved),
        Ok(Ecef {
            x_m: 10.0,
            y_m: 20.0,
            z_m: 30.0,
        })
    );
    // A non-finite record, a non-finite parameter, and an overflowing result
    // are each an error naming what failed.
    let field_of = |h: Helmert, x: f64, y: f64| match helmert_apply_ecef(
        h,
        Ecef {
            x_m: x,
            y_m: y,
            z_m: 0.0,
        },
    ) {
        Err(GeoError::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    };
    let scaled = |scale_ppm| Helmert {
        scale_ppm,
        ..identity
    };
    assert_eq!(field_of(shift, 0.0, f64::NAN), "y_m");
    assert_eq!(field_of(scaled(f64::NAN), 0.0, 0.0), "helmert");
    assert_eq!(field_of(scaled(1.0), f64::MAX, 0.0), "ecef");
}

#[test]
fn datum_transform_catalogue_is_pinned() {
    assert_eq!(datum_transform_catalog_to_wgs84(Crs::Wgs84), None);
    assert_eq!(datum_transform_catalog_to_wgs84(Crs::Gcj02), None);
    assert_eq!(datum_transform_catalog_to_wgs84(Crs::Bd09), None);
    // NIMA TR8350.2 mean shifts and the root-sum-square of their published
    // per-axis accuracies (none is published for Pulkovo-1942).
    let cases = [
        (Crs::Nad27, (-8.0, 160.0, 176.0), Some(86f64.sqrt())),
        (Crs::Tokyo, (-148.0, 507.0, 685.0), Some(825f64.sqrt())),
        (Crs::Pulkovo42, (28.0, -130.0, -95.0), None),
    ];
    for (crs, (tx, ty, tz), accuracy_m) in cases {
        let dt = datum_transform_catalog_to_wgs84(crs).unwrap();
        assert_eq!((dt.from_crs, dt.to_crs), (crs, Crs::Wgs84));
        assert_eq!(dt.accuracy_m, accuracy_m);
        assert_eq!(
            dt.helmert,
            Helmert {
                tx_m: tx,
                ty_m: ty,
                tz_m: tz,
                ..helmert_identity()
            }
        );
    }
}

#[test]
fn datum_transform_apply_and_inverse_round_trip() {
    let dt = datum_transform_catalog_to_wgs84(Crs::Nad27).unwrap();
    let nad27 = Coordinate {
        lat: 40.0,
        lon: -100.0,
        height: None,
        crs: Crs::Nad27,
    };
    let wgs = datum_transform_apply(dt, nad27).unwrap();
    assert_eq!(wgs.crs, Crs::Wgs84);
    close(wgs.lat, 40.000_009_482_759, 1e-9);
    close(wgs.lon, -100.000_417_622_218_8, 1e-9);
    let inverse = datum_transform_inverse_first_order(dt).unwrap();
    assert_eq!((inverse.from_crs, inverse.to_crs), (Crs::Wgs84, Crs::Nad27));
    assert_eq!(inverse.helmert, helmert_inverse_first_order(dt.helmert));
    assert_eq!(inverse.accuracy_m, dt.accuracy_m);
    let back = datum_transform_apply(inverse, wgs).unwrap();
    assert_eq!(back.crs, Crs::Nad27);
    close(back.lat, 40.0, 1e-8);
    close(back.lon, -100.0, 1e-8);
    match datum_transform_apply(dt, wgs84(40.0, -100.0)) {
        Err(GeoError::CrsMismatch { expected, found }) => {
            assert_eq!((expected, found), (Crs::Nad27, Crs::Wgs84));
        }
        other => panic!("expected CrsMismatch, got {other:?}"),
    }
}

#[test]
fn datum_transform_new_validates_hand_built_records() {
    let shift = Helmert {
        tx_m: 1.0,
        ..helmert_identity()
    };
    let dt = datum_transform_new(Crs::Tokyo, Crs::Nad27, shift, Some(3.0)).unwrap();
    assert_eq!(
        dt,
        DatumTransform {
            from_crs: Crs::Tokyo,
            to_crs: Crs::Nad27,
            helmert: shift,
            accuracy_m: Some(3.0),
        }
    );
    let gcj = DatumTransform {
        from_crs: Crs::Gcj02,
        ..dt
    };
    let bad_helmert = DatumTransform {
        helmert: Helmert {
            scale_ppm: f64::NAN,
            ..shift
        },
        ..dt
    };
    let bad_accuracy = DatumTransform {
        accuracy_m: Some(-1.0),
        ..dt
    };
    match datum_transform_new(gcj.from_crs, gcj.to_crs, gcj.helmert, gcj.accuracy_m) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "from_crs"),
        other => panic!("expected InvalidValue for `from_crs`, got {other:?}"),
    }
    match datum_transform_new(Crs::Tokyo, Crs::Bd09, shift, None) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "to_crs"),
        other => panic!("expected InvalidValue for `to_crs`, got {other:?}"),
    }
    let tokyo = coordinate_new(35.0, 139.0, Crs::Tokyo).unwrap();
    for bad in [gcj, bad_helmert, bad_accuracy] {
        assert!(datum_transform_apply(bad, tokyo).is_err(), "{bad:?}");
        assert!(datum_transform_inverse_first_order(bad).is_err(), "{bad:?}");
    }
    for (bad, field) in [(bad_helmert, "helmert"), (bad_accuracy, "accuracy")] {
        match datum_transform_new(bad.from_crs, bad.to_crs, bad.helmert, bad.accuracy_m) {
            Err(GeoError::InvalidValue { field: f, .. }) => assert_eq!(f, field),
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }
    // `datum_transform_validate` applies the same rules to a hand-built record.
    assert_eq!(datum_transform_validate(dt), Ok(()));
    for (bad, field) in [
        (gcj, "from_crs"),
        (bad_helmert, "helmert"),
        (bad_accuracy, "accuracy"),
    ] {
        match datum_transform_validate(bad) {
            Err(GeoError::InvalidValue { field: f, .. }) => assert_eq!(f, field),
            other => panic!("expected InvalidValue for `{field}`, got {other:?}"),
        }
    }
}
