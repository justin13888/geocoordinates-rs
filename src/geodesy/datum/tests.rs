use super::*;
use crate::coord::Height;
use crate::test_support::assert_close;
use crate::units::Length;

fn ecef(x: f64, y: f64, z: f64) -> Ecef {
    Ecef::new(x, y, z)
}

/// A synthetic Helmert with every parameter non-zero, for exercising all
/// seven terms (rotations deliberately not proportional to any test vector).
const FULL: Helmert = Helmert {
    tx_m: 10.0,
    ty_m: -20.0,
    tz_m: 30.0,
    rx_arcsec: 2.0,
    ry_arcsec: -3.0,
    rz_arcsec: 4.0,
    scale_ppm: 5.0,
};

#[test]
fn identity_is_a_no_op() {
    let e = ecef(4_000_000.0, -2_000_000.0, 4_500_000.0);
    let r = Helmert::IDENTITY.apply_ecef(e);
    assert_close(r.x_m, e.x_m, 1e-9);
    assert_close(r.y_m, e.y_m, 1e-9);
    assert_close(r.z_m, e.z_m, 1e-9);
}

#[test]
fn apply_ecef_translation_only() {
    let h = Helmert {
        tx_m: 10.0,
        ty_m: -20.0,
        tz_m: 30.0,
        ..Helmert::IDENTITY
    };
    let r = h.apply_ecef(ecef(1_000.0, 2_000.0, 3_000.0));
    assert_close(r.x_m, 1_010.0, 1e-9);
    assert_close(r.y_m, 1_980.0, 1e-9);
    assert_close(r.z_m, 3_030.0, 1e-9);
}

#[test]
fn apply_ecef_full_seven_parameter() {
    // Independent reference (geodetic/ECEF Python): every term contributes.
    let r = FULL.apply_ecef(ecef(4_000_000.0, -2_000_000.0, 4_500_000.0));
    assert_close(r.x_m, 4_000_003.335_114_215_5, 1e-4);
    assert_close(r.y_m, -1_999_996.062_872_637_8, 1e-4);
    assert_close(r.z_m, 4_500_091.285_288_414, 1e-4);
}

#[test]
fn inverse_negates_every_parameter() {
    let i = FULL.inverse_first_order();
    assert_close(i.tx_m, -10.0, 1e-12);
    assert_close(i.ty_m, 20.0, 1e-12);
    assert_close(i.tz_m, -30.0, 1e-12);
    assert_close(i.rx_arcsec, -2.0, 1e-12);
    assert_close(i.ry_arcsec, 3.0, 1e-12);
    assert_close(i.rz_arcsec, -4.0, 1e-12);
    assert_close(i.scale_ppm, -5.0, 1e-12);
}

#[test]
fn helmert_inverse_round_trips() {
    let e = ecef(4_000_000.0, -2_000_000.0, 4_500_000.0);
    let back = FULL.inverse_first_order().apply_ecef(FULL.apply_ecef(e));
    // First-order inverse: sub-cm residual from the dropped second-order term.
    assert_close(back.x_m, e.x_m, 1e-2);
    assert_close(back.y_m, e.y_m, 1e-2);
    assert_close(back.z_m, e.z_m, 1e-2);
}

#[test]
fn first_order_inverse_is_approximate_for_a_real_seven_parameter_set() {
    // WGS-84 → OSGB36 (EPSG:1314, position-vector convention): a published
    // full seven-parameter set, with the large scale and translation that
    // make the dropped cross terms visible. `Helmert::inverse` documents a
    // residual of about a centimeter for this set: nonzero, and under 2 cm.
    let osgb = Helmert {
        tx_m: -446.448,
        ty_m: 125.157,
        tz_m: -542.06,
        rx_arcsec: -0.1502,
        ry_arcsec: -0.247,
        rz_arcsec: -0.8421,
        scale_ppm: 20.4894,
    };
    let e = ecef(3_980_000.0, -10_000.0, 4_970_000.0); // near 51.5°N, 0°
    let back = osgb.inverse_first_order().apply_ecef(osgb.apply_ecef(e));
    let residual =
        ((back.x_m - e.x_m).powi(2) + (back.y_m - e.y_m).powi(2) + (back.z_m - e.z_m).powi(2))
            .sqrt();
    assert!(residual > 5e-3, "residual {residual} m should be visible");
    assert!(residual < 0.02, "residual {residual} m is over 2 cm");
}

#[test]
fn catalog_has_the_three_classic_datums() {
    // (datum, shift, published accuracy): NIMA TR8350.2, 3rd ed., Amdt. 1.
    let cases = [
        // App. B.6, NAS-C: −8 ±5, 160 ±5, 176 ±6.
        (
            Crs::Nad27,
            (-8.0, 160.0, 176.0),
            Some(9.273_618_495_495_704),
        ),
        // App. B.3, TOY-M: −148 ±20, 507 ±5, 685 ±20.
        (
            Crs::Tokyo,
            (-148.0, 507.0, 685.0),
            Some(28.722_813_232_690_143),
        ),
        // App. C.2, PUK (Russia): no error estimate is published.
        (Crs::Pulkovo42, (28.0, -130.0, -95.0), None),
    ];
    for (datum, (tx, ty, tz), accuracy) in cases {
        let t = DatumTransform::catalog_to_wgs84(datum).expect("catalogued");
        assert_eq!((t.from_crs(), t.to_crs()), (datum, Crs::Wgs84));
        assert_eq!(
            t.helmert(),
            Helmert {
                tx_m: tx,
                ty_m: ty,
                tz_m: tz,
                ..Helmert::IDENTITY
            }
        );
        assert_eq!(t.accuracy(), accuracy.map(Length::from_meters), "{datum:?}");
    }
}

#[test]
fn rss_combines_the_three_axes() {
    assert_eq!(rss_m(3.0, 4.0, 12.0), 13.0);
    assert_eq!(rss_m(0.0, 0.0, 2.0), 2.0);
}

#[test]
fn new_validates_its_systems_parameters_and_accuracy() {
    let shift = Helmert {
        tx_m: 1.0,
        ..Helmert::IDENTITY
    };
    let t = DatumTransform::new(Crs::Tokyo, Crs::Nad27, shift, Some(Length::ZERO)).unwrap();
    assert_eq!(
        (t.from_crs(), t.to_crs(), t.helmert(), t.accuracy()),
        (Crs::Tokyo, Crs::Nad27, shift, Some(Length::ZERO))
    );
    assert!(DatumTransform::new(Crs::Wgs84, Crs::Wgs84, shift, None).is_ok());
    // A China system is a bad argument, named by the field that carried it;
    // NAD27 shows the error invents no "expected WGS-84".
    for (from, to, field, bad) in [
        (Crs::Gcj02, Crs::Nad27, "from_crs", Crs::Gcj02),
        (Crs::Wgs84, Crs::Bd09, "to_crs", Crs::Bd09),
        (Crs::Bd09, Crs::Gcj02, "from_crs", Crs::Bd09),
    ] {
        assert_eq!(
            DatumTransform::new(from, to, shift, None),
            Err(Error::InvalidValue {
                field,
                detail: format!("{bad} is not a geodetic datum and has no ellipsoid"),
            })
        );
    }
    let bad_parameters = [
        Helmert {
            tx_m: f64::NAN,
            ..Helmert::IDENTITY
        },
        Helmert {
            ty_m: f64::INFINITY,
            ..Helmert::IDENTITY
        },
        Helmert {
            tz_m: f64::NAN,
            ..Helmert::IDENTITY
        },
        Helmert {
            rx_arcsec: f64::NAN,
            ..Helmert::IDENTITY
        },
        Helmert {
            ry_arcsec: f64::NEG_INFINITY,
            ..Helmert::IDENTITY
        },
        Helmert {
            rz_arcsec: f64::NAN,
            ..Helmert::IDENTITY
        },
        Helmert {
            scale_ppm: f64::INFINITY,
            ..Helmert::IDENTITY
        },
    ];
    for h in bad_parameters {
        assert!(
            matches!(
                DatumTransform::new(Crs::Nad27, Crs::Wgs84, h, None),
                Err(Error::InvalidValue {
                    field: "helmert",
                    ..
                })
            ),
            "{h:?}"
        );
    }
    for accuracy in [-1.0, -f64::MIN_POSITIVE, f64::NAN, f64::INFINITY] {
        assert!(
            matches!(
                DatumTransform::new(
                    Crs::Nad27,
                    Crs::Wgs84,
                    shift,
                    Some(Length::from_meters(accuracy))
                ),
                Err(Error::InvalidValue {
                    field: "accuracy",
                    ..
                })
            ),
            "{accuracy}"
        );
    }
}

/// C15: the ellipsoids come from the systems, so a NAD27 → WGS-84 identity
/// shift still moves a point by the Clarke 1866 / WGS 84 difference, and a
/// hand-built transform cannot pair a datum with another datum's ellipsoid.
#[test]
fn apply_uses_the_ellipsoid_of_each_system() {
    let identity = DatumTransform::new(Crs::Nad27, Crs::Wgs84, Helmert::IDENTITY, None).unwrap();
    let nad27 = Coordinate::new(40.0, -100.0, Crs::Nad27)
        .unwrap()
        .with_height(Height::Ellipsoidal(0.0))
        .unwrap();
    let w = identity.apply(nad27).unwrap();
    // The same ECEF point, re-expressed on WGS 84: the height changes by the
    // ellipsoids' separation there (tens of meters), not by zero.
    let Some(Height::Ellipsoidal(h)) = w.height() else {
        panic!("expected ellipsoidal height, got {:?}", w.height());
    };
    assert!(h.abs() > 1.0, "{h}");
    let back = Ecef::try_from_coordinate(w).unwrap();
    let start = Ecef::try_from_coordinate(nad27).unwrap();
    assert_close(back.x_m, start.x_m, 1e-7);
    assert_close(back.y_m, start.y_m, 1e-7);
    assert_close(back.z_m, start.z_m, 1e-7);
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_validates() {
    let t = DatumTransform::catalog_to_wgs84(Crs::Tokyo).unwrap();
    let json = serde_json::to_string(&t).unwrap();
    assert_eq!(serde_json::from_str::<DatumTransform>(&json).unwrap(), t);
    let pulkovo = DatumTransform::catalog_to_wgs84(Crs::Pulkovo42).unwrap();
    let json = serde_json::to_string(&pulkovo).unwrap();
    assert!(json.contains(r#""accuracy":null"#), "{json}");
    assert_eq!(
        serde_json::from_str::<DatumTransform>(&json).unwrap(),
        pulkovo
    );
    let gcj = json.replace(r#""from_crs":"Pulkovo42""#, r#""from_crs":"Gcj02""#);
    assert_ne!(gcj, json);
    assert!(serde_json::from_str::<DatumTransform>(&gcj).is_err());
    let negative = json.replace(r#""accuracy":null"#, r#""accuracy":{"meters":-1.0}"#);
    assert_ne!(negative, json);
    assert!(serde_json::from_str::<DatumTransform>(&negative).is_err());
}

#[test]
fn non_helmert_systems_have_no_catalog_entry() {
    assert!(DatumTransform::catalog_to_wgs84(Crs::Wgs84).is_none());
    assert!(DatumTransform::catalog_to_wgs84(Crs::Gcj02).is_none());
    assert!(DatumTransform::catalog_to_wgs84(Crs::Bd09).is_none());
}

#[test]
fn transform_nad27_to_wgs84_reference() {
    // Independent reference (Python): geodetic -> ECEF(Clarke66) -> shift ->
    // geodetic(WGS84) for (40°N, 100°W, 0 m). NAD27 → WGS84 in CONUS shifts
    // the longitude west by ~1.5″ and the ellipsoidal height down ~35 m.
    let dt = DatumTransform::catalog_to_wgs84(Crs::Nad27).unwrap();
    let nad27 = Coordinate::new(40.0, -100.0, Crs::Nad27)
        .unwrap()
        .with_height(Height::Ellipsoidal(0.0))
        .unwrap();
    let w = dt.apply(nad27).unwrap();
    assert_eq!(w.crs(), Crs::Wgs84);
    assert_close(w.lat(), 40.000_009_482_759, 1e-8);
    assert_close(w.lon(), -100.000_417_622_218_8, 1e-8);
    match w.height() {
        Some(Height::Ellipsoidal(h)) => assert_close(h, -35.215_786_937_624_216, 1e-3),
        other => panic!("expected ellipsoidal height, got {other:?}"),
    }
}

#[test]
fn transform_round_trips_through_inverse() {
    // Translation-only Helmert is exactly invertible, so a there-and-back
    // datum shift recovers the original geodetic coordinate.
    let dt = DatumTransform::catalog_to_wgs84(Crs::Tokyo).unwrap();
    let tokyo = Coordinate::new(35.0, 139.0, Crs::Tokyo)
        .unwrap()
        .with_height(Height::Ellipsoidal(50.0))
        .unwrap();
    let w = dt.apply(tokyo).unwrap();
    let back = dt.inverse_first_order().apply(w).unwrap();
    assert_eq!(back.crs(), Crs::Tokyo);
    assert_close(back.lat(), 35.0, 1e-9);
    assert_close(back.lon(), 139.0, 1e-9);
    match back.height() {
        Some(Height::Ellipsoidal(h)) => assert_close(h, 50.0, 1e-4),
        other => panic!("expected ellipsoidal height, got {other:?}"),
    }
}

#[test]
fn datum_transform_inverse_swaps_systems_and_keeps_the_accuracy() {
    let dt = DatumTransform::catalog_to_wgs84(Crs::Nad27).unwrap();
    let inv = dt.inverse_first_order();
    assert_eq!((inv.from_crs(), inv.to_crs()), (Crs::Wgs84, Crs::Nad27));
    assert_eq!(inv.helmert(), dt.helmert().inverse_first_order());
    assert_close(inv.helmert().tx_m, 8.0, 1e-12); // negated −8
    assert_eq!(inv.accuracy(), dt.accuracy());
}

#[test]
fn transform_rejects_a_mismatched_source_crs_and_preserves_missing_height() {
    let dt = DatumTransform::catalog_to_wgs84(Crs::Nad27).unwrap();
    assert!(dt.apply(Coordinate::wgs84(40.0, -100.0).unwrap()).is_err());
    let result = dt
        .apply(Coordinate::new(40.0, -100.0, Crs::Nad27).unwrap())
        .unwrap();
    assert_eq!(result.crs(), Crs::Wgs84);
    assert_eq!(result.height(), None);
}
