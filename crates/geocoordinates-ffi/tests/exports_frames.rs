//! Calls every local-frame FFI export (ENU / NED / AER) at least once with an
//! asserted value, including the validators and the classic-datum paths.

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

fn invalid_field<T: std::fmt::Debug>(result: Result<T, GeoError>) -> String {
    match result {
        Err(GeoError::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

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

/// A NAD27 pair round-trips through every frame and comes back tagged NAD27;
/// a pair on different datums, or on GCJ-02, is a `CrsMismatch`.
#[test]
fn frames_accept_any_shared_classic_datum() {
    let origin = coordinate_new(39.0, -98.0, Crs::Nad27).unwrap();
    let target = coordinate_new(39.05, -97.93, Crs::Nad27).unwrap();
    let enu = enu_from_coordinate(target, origin).unwrap();
    let ned = ned_from_coordinate(target, origin).unwrap();
    let aer = aer_from_coordinate(target, origin).unwrap();
    for back in [
        enu_to_coordinate(enu, origin).unwrap(),
        ned_to_coordinate(ned, origin).unwrap(),
        aer_to_coordinate(aer, origin).unwrap(),
    ] {
        assert_eq!(back.crs, Crs::Nad27);
        close(back.lat, 39.05, 1e-9);
        close(back.lon, -97.93, 1e-9);
    }

    assert_eq!(
        enu_from_coordinate(target, wgs84(39.0, -98.0)),
        Err(GeoError::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Nad27,
        })
    );
    let gcj = coordinate_new(39.0, -98.0, Crs::Gcj02).unwrap();
    assert!(matches!(
        aer_from_coordinate(gcj, gcj),
        Err(GeoError::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
}

#[test]
fn frame_conversions_swap_axes_and_signs() {
    let enu = Enu {
        east: 1.0,
        north: 2.0,
        up: 3.0,
    };
    let ned = Ned {
        north: 2.0,
        east: 1.0,
        down: -3.0,
    };
    assert_eq!(enu_to_ned(enu), Ok(ned));
    assert_eq!(ned_to_enu(ned), Ok(enu));
    let north = enu_to_aer(Enu {
        east: 0.0,
        north: 100.0,
        up: 0.0,
    })
    .unwrap();
    assert_eq!(
        (north.azimuth_deg, north.elevation_deg, north.range_m),
        (0.0, 0.0, 100.0)
    );
    let up_east = ned_to_aer(Ned {
        north: 0.0,
        east: 100.0,
        down: -100.0,
    })
    .unwrap();
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
    })
    .unwrap();
    close(enu.east, 100.0, 1e-9);
    close(enu.north, 0.0, 1e-9);
    close(enu.up, 0.0, 1e-9);
    let ned = aer_to_ned(Aer {
        azimuth_deg: 0.0,
        elevation_deg: 90.0,
        range_m: 100.0,
    })
    .unwrap();
    close(ned.north, 0.0, 1e-9);
    close(ned.east, 0.0, 1e-9);
    close(ned.down, -100.0, 1e-9);
}

#[test]
fn validators_name_the_invalid_field() {
    let enu = Enu {
        east: 0.0,
        north: 0.0,
        up: 0.0,
    };
    assert_eq!(enu_validate(enu), Ok(()));
    assert_eq!(
        invalid_field(enu_validate(Enu {
            east: f64::NAN,
            ..enu
        })),
        "east"
    );
    assert_eq!(
        invalid_field(enu_validate(Enu {
            north: f64::INFINITY,
            ..enu
        })),
        "north"
    );
    assert_eq!(
        invalid_field(enu_validate(Enu {
            up: f64::NAN,
            ..enu
        })),
        "up"
    );
    let huge = Enu {
        east: f64::MAX,
        north: f64::MAX,
        up: 0.0,
    };
    assert_eq!(invalid_field(enu_validate(huge)), "magnitude");

    let ned = Ned {
        north: 0.0,
        east: 0.0,
        down: 0.0,
    };
    assert_eq!(ned_validate(ned), Ok(()));
    assert_eq!(
        invalid_field(ned_validate(Ned {
            north: f64::NAN,
            ..ned
        })),
        "north"
    );
    assert_eq!(
        invalid_field(ned_validate(Ned {
            east: f64::NAN,
            ..ned
        })),
        "east"
    );
    assert_eq!(
        invalid_field(ned_validate(Ned {
            down: f64::NAN,
            ..ned
        })),
        "down"
    );

    let aer = Aer {
        azimuth_deg: 370.0,
        elevation_deg: 0.0,
        range_m: 1.0,
    };
    assert_eq!(aer_validate(aer), Ok(()));
    let az = Aer {
        azimuth_deg: f64::NAN,
        ..aer
    };
    assert_eq!(invalid_field(aer_validate(az)), "azimuth");
    let el = Aer {
        elevation_deg: 90.5,
        ..aer
    };
    assert_eq!(invalid_field(aer_validate(el)), "elevation");
    let range = Aer {
        range_m: -1.0,
        ..aer
    };
    assert_eq!(invalid_field(aer_validate(range)), "range");
}

/// Every export taking a frame record validates it, and an azimuth outside
/// `[0, 360)` comes back normalized.
#[test]
fn exports_reject_invalid_records_and_normalize_azimuth() {
    let origin = wgs84(40.0, -75.0);
    let bad_enu = Enu {
        east: f64::NAN,
        north: 0.0,
        up: 0.0,
    };
    assert_eq!(invalid_field(enu_to_coordinate(bad_enu, origin)), "east");
    assert_eq!(invalid_field(enu_to_ned(bad_enu)), "east");
    assert_eq!(invalid_field(enu_to_aer(bad_enu)), "east");
    let bad_ned = Ned {
        north: 0.0,
        east: 0.0,
        down: f64::INFINITY,
    };
    assert_eq!(invalid_field(ned_to_coordinate(bad_ned, origin)), "down");
    assert_eq!(invalid_field(ned_to_enu(bad_ned)), "down");
    assert_eq!(invalid_field(ned_to_aer(bad_ned)), "down");
    let bad_aer = Aer {
        azimuth_deg: 0.0,
        elevation_deg: -91.0,
        range_m: 1.0,
    };
    assert_eq!(
        invalid_field(aer_to_coordinate(bad_aer, origin)),
        "elevation"
    );
    assert_eq!(invalid_field(aer_to_enu(bad_aer)), "elevation");
    assert_eq!(invalid_field(aer_to_ned(bad_aer)), "elevation");

    let wrapped = Aer {
        azimuth_deg: -270.0,
        elevation_deg: 0.0,
        range_m: 100.0,
    };
    let back = enu_to_aer(aer_to_enu(wrapped).unwrap()).unwrap();
    close(back.azimuth_deg, 90.0, 1e-9);
}
