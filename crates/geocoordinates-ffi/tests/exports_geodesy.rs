//! Calls every geodesy FFI export (ellipsoids, ECEF, local frames, geodesics,
//! rhumb lines, Helmert / datum transforms, conversion dispatch) at least once
//! with an asserted value.
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
        assert_eq!(ellipsoid_flattening(ellipsoid), 1.0 / inv_f);
    }
}

#[test]
fn ellipsoid_derived_quantities_match_published_wgs84_values() {
    let wgs84 = ellipsoid_wgs84();
    close(
        ellipsoid_flattening(wgs84),
        0.003_352_810_664_747_480_5,
        1e-18,
    );
    close(ellipsoid_semi_minor_m(wgs84), 6_356_752.314_245, 1e-6);
    close(
        ellipsoid_eccentricity_sq(wgs84),
        0.006_694_379_990_14,
        1e-14,
    );
    // Clarke 1866 publishes b = 6,356,583.8 m; deriving it from the rounded
    // inverse flattening lands about 1 µm short.
    close(
        ellipsoid_semi_minor_m(ellipsoid_clarke_1866()),
        6_356_583.8,
        1e-5,
    );
}

// --- ECEF ---

#[test]
fn ecef_round_trips_through_the_coordinate() {
    let origin = wgs84(40.0, -75.0);
    let ecef = ecef_from_coordinate(origin, ellipsoid_wgs84()).unwrap();
    close(ecef.x, 1_266_325.909_016_660_2, 1e-6);
    close(ecef.y, -4_725_992.631_391_02, 1e-6);
    close(ecef.z, 4_077_985.572_200_376, 1e-6);
    let back = ecef_to_coordinate(ecef, ellipsoid_wgs84(), Crs::Wgs84).unwrap();
    close(back.lat, 40.0, 1e-12);
    close(back.lon, -75.0, 1e-12);
    assert_eq!(back.crs, Crs::Wgs84);
    assert_eq!(back.height, Some(Height::Ellipsoidal { meters: 0.0 }));
}

#[test]
fn ecef_from_coordinate_on_the_equator_meets_the_semi_major_axis() {
    let ecef = ecef_from_coordinate(wgs84(0.0, 0.0), ellipsoid_wgs84()).unwrap();
    assert_eq!((ecef.x, ecef.y, ecef.z), (6_378_137.0, 0.0, 0.0));
}

// --- Local tangent frames ---

#[test]
fn enu_ned_and_aer_from_coordinate_agree() {
    let origin = wgs84(40.0, -75.0);
    let target = wgs84(40.1, -74.9);
    let enu = enu_from_coordinate(target, origin).unwrap();
    close(enu.east, 8_526.911_601_003_281, 1e-6);
    close(enu.north, 11_108.336_797_369_358, 1e-6);
    close(enu.up, -15.389_949_872_675_06, 1e-6);
    let ned = ned_from_coordinate(target, origin).unwrap();
    assert_eq!(
        (ned.north, ned.east, ned.down),
        (enu.north, enu.east, -enu.up)
    );
    let aer = aer_from_coordinate(target, origin).unwrap();
    close(aer.azimuth_deg, 37.510_338_419_019_85, 1e-9);
    close(aer.elevation_deg, -0.062_967_599_443_068_47, 1e-9);
    close(aer.range_m, 14_003.699_679_215_903, 1e-6);
}

#[test]
fn frame_offsets_recover_the_absolute_coordinate() {
    let origin = wgs84(40.0, -75.0);
    let target = wgs84(40.1, -74.9);
    let enu = enu_from_coordinate(target, origin).unwrap();
    let from_enu = enu_to_coordinate(enu, origin).unwrap();
    close(from_enu.lat, 40.1, 1e-9);
    close(from_enu.lon, -74.9, 1e-9);
    assert_eq!(from_enu.crs, Crs::Wgs84);

    let north = ned_to_coordinate(
        Ned {
            north: 1000.0,
            east: 0.0,
            down: 0.0,
        },
        origin,
    )
    .unwrap();
    close(north.lat, 40.009_006_191_934_64, 1e-12);
    close(north.lon, -75.0, 1e-12);

    let east = aer_to_coordinate(
        Aer {
            azimuth_deg: 90.0,
            elevation_deg: 0.0,
            range_m: 1000.0,
        },
        origin,
    )
    .unwrap();
    close(east.lat, 39.999_999_408_397_81, 1e-12);
    close(east.lon, -74.988_289_555_927_2, 1e-12);
}

#[test]
fn frame_conversions_swap_axes_and_signs() {
    let enu = Enu {
        east: 1.0,
        north: 2.0,
        up: 3.0,
    };
    assert_eq!(
        enu_to_ned(enu),
        Ned {
            north: 2.0,
            east: 1.0,
            down: -3.0,
        }
    );
    assert_eq!(
        ned_to_enu(Ned {
            north: 2.0,
            east: 1.0,
            down: -3.0,
        }),
        enu
    );
    let north = enu_to_aer(Enu {
        east: 0.0,
        north: 100.0,
        up: 0.0,
    });
    assert_eq!(
        (north.azimuth_deg, north.elevation_deg, north.range_m),
        (0.0, 0.0, 100.0)
    );
    let up_east = ned_to_aer(Ned {
        north: 0.0,
        east: 100.0,
        down: -100.0,
    });
    close(up_east.azimuth_deg, 90.0, 1e-12);
    close(up_east.elevation_deg, 45.0, 1e-12);
    close(up_east.range_m, 100.0 * 2f64.sqrt(), 1e-9);
}

#[test]
fn aer_converts_to_enu_and_ned() {
    let enu = aer_to_enu(Aer {
        azimuth_deg: 90.0,
        elevation_deg: 0.0,
        range_m: 100.0,
    });
    close(enu.east, 100.0, 1e-9);
    close(enu.north, 0.0, 1e-9);
    close(enu.up, 0.0, 1e-9);
    let ned = aer_to_ned(Aer {
        azimuth_deg: 0.0,
        elevation_deg: 90.0,
        range_m: 100.0,
    });
    close(ned.north, 0.0, 1e-9);
    close(ned.east, 0.0, 1e-9);
    close(ned.down, -100.0, 1e-9);
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
        initial_bearing(new_york(), london()).unwrap(),
        51.241_229_119_512_35,
        1e-9,
    );
    close(
        final_bearing(new_york(), london()).unwrap(),
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
    close(
        rhumb_distance_m(new_york(), london()).unwrap(),
        5_794_128.753_915_687,
        1e-6,
    );
    close(
        rhumb_bearing(new_york(), london()).unwrap(),
        78.044_080_868_104_5,
        1e-9,
    );
    let dest = rhumb_destination(new_york(), 45.0, 100_000.0).unwrap();
    close(dest.lat, 41.348_715_527_648_77, 1e-9);
    close(dest.lon, -73.162_999_276_612_94, 1e-9);
}

#[test]
fn cross_and_along_track_distances_are_pinned() {
    let start = wgs84(0.0, 0.0);
    let end = wgs84(0.0, 10.0);
    let point = wgs84(1.0, 5.0);
    // North of an eastbound path is to its left: negative.
    close(
        cross_track_distance_m(point, start, end).unwrap(),
        -111_195.080_233_532_85,
        1e-6,
    );
    close(
        along_track_distance_m(point, start, end).unwrap(),
        555_975.401_167_671_7,
        1e-6,
    );
}

#[test]
fn intersection_of_the_equator_and_the_prime_meridian_is_null_island() {
    let hit = intersection(wgs84(0.0, -10.0), 90.0, wgs84(-10.0, 0.0), 0.0)
        .unwrap()
        .expect("the equator and the prime meridian intersect");
    close(hit.lat, 0.0, 1e-12);
    close(hit.lon, 0.0, 1e-12);
    assert_eq!(hit.crs, Crs::Wgs84);
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
    let inverse = helmert_inverse(shift);
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
    assert_eq!(datum_transform_to_wgs84(Crs::Wgs84), None);
    assert_eq!(datum_transform_to_wgs84(Crs::Gcj02), None);
    assert_eq!(datum_transform_to_wgs84(Crs::Bd09), None);
    let cases = [
        (Crs::Nad27, ellipsoid_clarke_1866(), (-8.0, 160.0, 176.0)),
        (Crs::Tokyo, ellipsoid_bessel_1841(), (-148.0, 507.0, 685.0)),
        (
            Crs::Pulkovo42,
            ellipsoid_krasovsky_1940(),
            (28.0, -130.0, -95.0),
        ),
    ];
    for (crs, ellipsoid, (tx, ty, tz)) in cases {
        let dt = datum_transform_to_wgs84(crs).unwrap();
        assert_eq!((dt.from_crs, dt.to_crs), (crs, Crs::Wgs84));
        assert_eq!((dt.from, dt.to), (ellipsoid, ellipsoid_wgs84()));
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
    let dt = datum_transform_to_wgs84(Crs::Nad27).unwrap();
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
    let inverse = datum_transform_inverse(dt);
    assert_eq!((inverse.from_crs, inverse.to_crs), (Crs::Wgs84, Crs::Nad27));
    assert_eq!((inverse.from, inverse.to), (dt.to, dt.from));
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
    assert_eq!(out.max_error_m, 0.0);
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
