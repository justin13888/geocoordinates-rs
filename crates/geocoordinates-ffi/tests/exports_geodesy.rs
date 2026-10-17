//! Calls every geodesy FFI export (ellipsoids, ECEF, geodesics, rhumb lines,
//! Helmert / datum transforms, conversion dispatch) at least once with an
//! asserted value. The local frames are in `exports_frames.rs`.
//!
//! Values are pinned to what the core crate produces today: the Karney
//! geodesic is reference-grade (`geographiclib-rs`), the spherical and rhumb
//! functions use the mean Earth radius. A change in either the mirror or the
//! core surfaces here as a failing assertion.

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

fn new_york() -> Coordinate {
    wgs84(40.7128, -74.006)
}

fn london() -> Coordinate {
    wgs84(51.5074, -0.1278)
}

// --- Ellipsoids ---

#[test]
fn ellipsoid_constructors_carry_their_defining_parameters() {
    let cases = [
        (ellipsoid_wgs84(), 6_378_137.0, 298.257_223_563),
        (ellipsoid_grs80(), 6_378_137.0, 298.257_222_101),
        (ellipsoid_krasovsky_1940(), 6_378_245.0, 298.3),
        (ellipsoid_airy_1830(), 6_377_563.396, 299.324_964_6),
        (ellipsoid_bessel_1841(), 6_377_397.155, 299.152_812_8),
        (ellipsoid_clarke_1866(), 6_378_206.4, 294.978_698_2),
    ];
    for (ellipsoid, a, inv_f) in cases {
        assert_eq!(ellipsoid.semi_major_m, a);
        assert_eq!(ellipsoid.inverse_flattening, inv_f);
        assert_eq!(ellipsoid_flattening(ellipsoid).unwrap(), 1.0 / inv_f);
        assert_eq!(ellipsoid_new(a, inv_f).unwrap(), ellipsoid);
    }
}

#[test]
fn ellipsoid_new_and_the_getters_validate() {
    assert_eq!(
        ellipsoid_new(6_371_000.0, f64::INFINITY).unwrap(),
        Ellipsoid {
            semi_major_m: 6_371_000.0,
            inverse_flattening: f64::INFINITY,
        }
    );
    let bad_axis = Ellipsoid {
        semi_major_m: -1.0,
        inverse_flattening: 298.0,
    };
    let bad_flattening = Ellipsoid {
        semi_major_m: 6_378_137.0,
        inverse_flattening: 0.5,
    };
    assert_eq!(ellipsoid_validate(ellipsoid_wgs84()), Ok(()));
    for e in [bad_axis, bad_flattening] {
        assert!(matches!(
            ellipsoid_new(e.semi_major_m, e.inverse_flattening),
            Err(GeoError::InvalidValue { .. })
        ));
        assert!(matches!(
            ellipsoid_validate(e),
            Err(GeoError::InvalidValue { .. })
        ));
        assert!(matches!(
            ellipsoid_flattening(e),
            Err(GeoError::InvalidValue { .. })
        ));
        assert!(matches!(
            ellipsoid_semi_minor_m(e),
            Err(GeoError::InvalidValue { .. })
        ));
        assert!(matches!(
            ellipsoid_eccentricity_sq(e),
            Err(GeoError::InvalidValue { .. })
        ));
    }
}

#[test]
fn ellipsoid_for_crs_names_each_datum_ellipsoid() {
    assert_eq!(ellipsoid_for_crs(Crs::Wgs84), Some(ellipsoid_wgs84()));
    assert_eq!(ellipsoid_for_crs(Crs::Nad27), Some(ellipsoid_clarke_1866()));
    assert_eq!(ellipsoid_for_crs(Crs::Tokyo), Some(ellipsoid_bessel_1841()));
    assert_eq!(
        ellipsoid_for_crs(Crs::Pulkovo42),
        Some(ellipsoid_krasovsky_1940())
    );
    assert_eq!(ellipsoid_for_crs(Crs::Gcj02), None);
    assert_eq!(ellipsoid_for_crs(Crs::Bd09), None);
}

#[test]
fn ellipsoid_derived_quantities_match_published_wgs84_values() {
    let wgs84 = ellipsoid_wgs84();
    close(
        ellipsoid_flattening(wgs84).unwrap(),
        0.003_352_810_664_747_480_5,
        1e-18,
    );
    close(
        ellipsoid_semi_minor_m(wgs84).unwrap(),
        6_356_752.314_245,
        1e-6,
    );
    close(
        ellipsoid_eccentricity_sq(wgs84).unwrap(),
        0.006_694_379_990_14,
        1e-14,
    );
    // Clarke 1866 publishes b = 6,356,583.8 m; deriving it from the rounded
    // inverse flattening lands about 1 µm short.
    close(
        ellipsoid_semi_minor_m(ellipsoid_clarke_1866()).unwrap(),
        6_356_583.8,
        1e-5,
    );
}

// --- ECEF ---

#[test]
fn ecef_round_trips_through_the_coordinate() {
    let origin = wgs84(40.0, -75.0);
    let ecef = ecef_from_coordinate(origin).unwrap();
    close(ecef.x, 1_266_325.909_016_660_2, 1e-6);
    close(ecef.y, -4_725_992.631_391_02, 1e-6);
    close(ecef.z, 4_077_985.572_200_376, 1e-6);
    let back = ecef_to_coordinate(ecef, Crs::Wgs84).unwrap();
    close(back.lat, 40.0, 1e-12);
    close(back.lon, -75.0, 1e-12);
    assert_eq!(back.crs, Crs::Wgs84);
    let Some(Height::Ellipsoidal { meters }) = back.height else {
        panic!("expected an ellipsoidal height, got {:?}", back.height);
    };
    close(meters, 0.0, 1e-8);
}

#[test]
fn ecef_from_coordinate_uses_the_ellipsoid_of_the_crs() {
    let ecef = ecef_from_coordinate(wgs84(0.0, 0.0)).unwrap();
    assert_eq!((ecef.x, ecef.y, ecef.z), (6_378_137.0, 0.0, 0.0));
    let nad27 = coordinate_new(0.0, 0.0, Crs::Nad27).unwrap();
    let ecef = ecef_from_coordinate(nad27).unwrap();
    assert_eq!((ecef.x, ecef.y, ecef.z), (6_378_206.4, 0.0, 0.0));
    let back = ecef_to_coordinate(ecef, Crs::Nad27).unwrap();
    assert_eq!((back.lat, back.lon, back.crs), (0.0, 0.0, Crs::Nad27));
    // A GCJ-02 coordinate is on the wrong datum; BD-09 named as the target
    // system is an invalid `crs` argument.
    let gcj = coordinate_new(39.9, 116.4, Crs::Gcj02).unwrap();
    assert!(matches!(
        ecef_from_coordinate(gcj),
        Err(GeoError::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
    match ecef_to_coordinate(ecef, Crs::Bd09) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "crs"),
        other => panic!("expected InvalidValue for `crs`, got {other:?}"),
    }
}

// --- Geodesics ---

#[test]
fn geodesic_distance_and_bearings_are_pinned() {
    close(
        geodesic_distance_m(new_york(), london()).unwrap(),
        5_585_233.578_931_302,
        1e-6,
    );
    close(
        initial_bearing_deg(new_york(), london()).unwrap(),
        51.241_229_119_512_35,
        1e-9,
    );
    close(
        final_bearing_deg(new_york(), london()).unwrap(),
        108.368_998_113_182_66,
        1e-9,
    );
}

#[test]
fn destination_follows_the_equator() {
    let dest = destination(wgs84(0.0, 0.0), 90.0, 111_319.49).unwrap();
    close(dest.lat, 0.0, 1e-12);
    close(dest.lon, 1.0, 1e-7);
    assert_eq!(dest.crs, Crs::Wgs84);
}

#[test]
fn midpoint_and_intermediate_lie_on_the_geodesic() {
    let mid = midpoint(new_york(), london()).unwrap();
    close(mid.lat, 52.389_474_472_737_035, 1e-9);
    close(mid.lon, -41.277_901_428_957_82, 1e-9);
    let quarter = intermediate(new_york(), london(), 0.25).unwrap();
    close(quarter.lat, 47.724_788_823_203_14, 1e-9);
    close(quarter.lon, -59.451_851_864_168_37, 1e-9);
    let start = intermediate(new_york(), london(), 0.0).unwrap();
    close(start.lat, 40.7128, 1e-9);
    close(start.lon, -74.006, 1e-9);
}

#[test]
fn rhumb_line_distance_bearing_and_destination_are_pinned() {
    // GeographicLib RhumbSolve on WGS-84: I 40.7128 -74.006 51.5074 -0.1278
    // and D 40.7128 -74.006 45 100000.
    close(
        rhumb_distance_m(new_york(), london()).unwrap(),
        5_809_801.212_288_193,
        1e-5,
    );
    close(
        rhumb_bearing_deg(new_york(), london()).unwrap(),
        78.081_345_760_192_42,
        1e-9,
    );
    let dest = rhumb_destination(new_york(), 45.0, 100_000.0).unwrap();
    close(dest.lat, 41.349_520_443_859_53, 1e-9);
    close(dest.lon, -73.165_151_728_798_76, 1e-9);
    // A non-meridional line cannot reach a pole.
    assert!(matches!(
        rhumb_destination(wgs84(80.0, 0.0), 45.0, 2_000_000.0),
        Err(GeoError::InvalidValue { .. })
    ));
    // Coincident points have no bearing.
    assert!(matches!(
        rhumb_bearing_deg(new_york(), new_york()),
        Err(GeoError::InvalidValue { .. })
    ));
    // From a pole the bearing is read against the start's own meridian, so
    // destination inverts it: (90, 0) → (0, 90) is bearing 90°.
    let (pole, target) = (wgs84(90.0, 0.0), wgs84(0.0, 90.0));
    let bearing = rhumb_bearing_deg(pole, target).unwrap();
    close(bearing, 90.0, 1e-9);
    let reached =
        rhumb_destination(pole, bearing, rhumb_distance_m(pole, target).unwrap()).unwrap();
    close(reached.lat, 0.0, 1e-9);
    close(reached.lon, 90.0, 1e-9);
}

#[test]
fn cross_and_along_track_distances_are_pinned() {
    let start = wgs84(0.0, 0.0);
    let end = wgs84(0.0, 10.0);
    let point = wgs84(1.0, 5.0);
    // North of an eastbound path is to its left: negative.
    close(
        cross_track_distance_spherical_m(point, start, end).unwrap(),
        -111_195.080_233_532_85,
        1e-6,
    );
    close(
        along_track_distance_spherical_m(point, start, end).unwrap(),
        555_975.401_167_671_7,
        1e-6,
    );
    // A path from a point to itself fixes no great circle.
    assert!(cross_track_distance_spherical_m(point, start, start).is_err());
    assert!(along_track_distance_spherical_m(point, start, start).is_err());
    // A pole of the path's great circle has no along-track distance.
    match along_track_distance_spherical_m(wgs84(90.0, 0.0), start, end) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "point"),
        other => panic!("expected InvalidValue for `point`, got {other:?}"),
    }
}

#[test]
fn ellipsoidal_cross_and_along_track_distances_are_pinned() {
    let (start, end, point) = (wgs84(0.0, 0.0), wgs84(0.0, 10.0), wgs84(1.0, 5.0));
    // Meridians meet the equator square: the foot is (0, 5), the cross-track
    // distance its meridian arc (to the left), the along-track a · 5°.
    let arc = geodesic_distance_m(wgs84(0.0, 5.0), point).unwrap();
    close(arc, 110_574.388_557_799_2, 1e-6);
    close(
        cross_track_distance_m(point, start, end).unwrap(),
        -arc,
        1e-6,
    );
    close(
        along_track_distance_m(point, start, end).unwrap(),
        556_597.453_966_367_9,
        1e-6,
    );
    assert!(cross_track_distance_m(point, start, start).is_err());
    match along_track_distance_m(wgs84(90.0, 0.0), start, end) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "point"),
        other => panic!("expected InvalidValue for `point`, got {other:?}"),
    }
    let tokyo = coordinate_new(1.0, 5.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        cross_track_distance_m(tokyo, start, end),
        Err(GeoError::CrsMismatch { .. })
    ));
}

#[test]
fn ellipsoidal_intersection_names_each_outcome() {
    let GeodesicIntersectionResult::Crossing { point } =
        intersection(wgs84(0.0, 0.0), 90.0, wgs84(-10.0, 10.0), 0.0).unwrap()
    else {
        panic!("the equator and the 10° E meridian cross");
    };
    close(point.lat, 0.0, 1e-12);
    close(point.lon, 10.0, 1e-12);
    assert_eq!(point.crs, Crs::Wgs84);
    assert_eq!(
        intersection(wgs84(0.0, 0.0), 90.0, wgs84(0.0, 30.0), 270.0).unwrap(),
        GeodesicIntersectionResult::SameGeodesic
    );
    assert_eq!(
        intersection(wgs84(0.0, 0.0), 0.0, wgs84(0.0, 30.0), 180.0).unwrap(),
        GeodesicIntersectionResult::NoForwardCrossing
    );
    match intersection(wgs84(0.0, 0.0), f64::NAN, wgs84(0.0, 30.0), 0.0) {
        Err(GeoError::InvalidValue { field, .. }) => assert_eq!(field, "bearing"),
        other => panic!("expected InvalidValue for `bearing`, got {other:?}"),
    }
}

#[test]
fn intersection_of_the_equator_and_the_prime_meridian_is_null_island() {
    let IntersectionResult::Crossing { point: hit } =
        intersection_spherical(wgs84(0.0, -10.0), 90.0, wgs84(-10.0, 0.0), 0.0).unwrap()
    else {
        panic!("the equator and the prime meridian intersect");
    };
    close(hit.lat, 0.0, 1e-12);
    close(hit.lon, 0.0, 1e-12);
    assert_eq!(hit.crs, Crs::Wgs84);
}

#[test]
fn intersection_names_each_outcome() {
    // From the north pole, 180° runs down the given meridian to (0, 10).
    let IntersectionResult::Crossing { point } =
        intersection_spherical(wgs84(90.0, 10.0), 180.0, wgs84(0.0, 0.0), 90.0).unwrap()
    else {
        panic!("the meridian meets the equator");
    };
    close(point.lat, 0.0, 1e-9);
    close(point.lon, 10.0, 1e-9);
    assert_eq!(
        intersection_spherical(wgs84(0.0, 0.0), 90.0, wgs84(0.0, 5.0), 90.0).unwrap(),
        IntersectionResult::SameGreatCircle
    );
    assert_eq!(
        intersection_spherical(wgs84(10.0, 0.0), 270.0, wgs84(0.0, 10.0), 0.0).unwrap(),
        IntersectionResult::NoForwardCrossing
    );
    let gcj = coordinate_new(30.0, 110.0, Crs::Gcj02).unwrap();
    assert!(intersection_spherical(gcj, 0.0, gcj, 90.0).is_err());
}

#[test]
fn geodesics_use_the_datum_ellipsoid_and_reject_obfuscated_datums() {
    let tokyo = |lat, lon| coordinate_new(lat, lon, Crs::Tokyo).unwrap();
    let on_bessel = geodesic_distance_m(tokyo(35.0, 139.0), tokyo(36.0, 140.0)).unwrap();
    let on_wgs84 = geodesic_distance_m(wgs84(35.0, 139.0), wgs84(36.0, 140.0)).unwrap();
    assert!((on_wgs84 - on_bessel).abs() > 1.0);
    assert_eq!(
        destination(tokyo(35.0, 139.0), 0.0, 1_000.0).unwrap().crs,
        Crs::Tokyo
    );
    let gcj = |lat, lon| coordinate_new(lat, lon, Crs::Gcj02).unwrap();
    assert!(geodesic_distance_m(gcj(30.0, 110.0), gcj(31.0, 111.0)).is_err());
    assert!(initial_bearing_deg(new_york(), new_york()).is_err());
}

// --- Helmert and datum transforms ---

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
            x: 10.0,
            y: 20.0,
            z: 30.0,
        },
    );
    assert_eq!(
        moved,
        Ecef {
            x: 11.0,
            y: 22.0,
            z: 33.0,
        }
    );
    let inverse = helmert_inverse_first_order(shift);
    assert_eq!(
        (inverse.tx_m, inverse.ty_m, inverse.tz_m),
        (-1.0, -2.0, -3.0)
    );
    assert_eq!(
        helmert_apply_ecef(inverse, moved),
        Ecef {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        }
    );
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

// --- Runtime conversion dispatch ---

#[test]
fn convert_routes_through_the_wgs84_hub() {
    let nad27 = Coordinate {
        lat: 40.0,
        lon: -100.0,
        height: None,
        crs: Crs::Nad27,
    };
    let out = convert(nad27, Crs::Wgs84).unwrap();
    // The Helmert leg reports TR8350.2's accuracy for the NAD27 mean shift.
    assert_eq!(out.max_error_m, 86f64.sqrt());
    assert_eq!(out.coord.crs, Crs::Wgs84);
    close(out.coord.lat, 40.000_009_482_759, 1e-9);
    close(out.coord.lon, -100.000_417_622_218_8, 1e-9);
}

#[test]
fn convert_covers_every_pair() {
    let all = [
        Crs::Wgs84,
        Crs::Gcj02,
        Crs::Bd09,
        Crs::Nad27,
        Crs::Tokyo,
        Crs::Pulkovo42,
    ];
    for from in all {
        let coord = coordinate_new(39.915, 116.404, from).unwrap();
        for to in all {
            let out = convert(coord, to).unwrap_or_else(|e| panic!("{from:?} -> {to:?}: {e}"));
            assert_eq!(out.coord.crs, to);
        }
    }
}
