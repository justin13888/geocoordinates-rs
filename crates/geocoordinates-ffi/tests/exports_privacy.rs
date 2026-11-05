//! Calls every privacy FFI export at least once with an asserted value, including
//! the failure each one documents.

use std::time::UNIX_EPOCH;

use geocoordinates_ffi::*;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

fn geohash(level: u8) -> Precision {
    Precision {
        grid: SnapGrid::Geohash,
        level,
    }
}

fn circle(lat: f64, lon: f64, radius_m: f64) -> ExclusionZone {
    ExclusionZone::Circle {
        center: wgs84(lat, lon),
        radius_m,
    }
}

#[test]
fn precision_records_are_checked_against_their_grid() {
    precision_validate(geohash(12)).unwrap();
    precision_validate(Precision {
        grid: SnapGrid::H3,
        level: 15,
    })
    .unwrap();
    precision_validate(Precision {
        grid: SnapGrid::S2,
        level: 30,
    })
    .unwrap();
    for (grid, level, field) in [
        (SnapGrid::Geohash, 0, "geohash length"),
        (SnapGrid::H3, 16, "H3 resolution"),
        (SnapGrid::S2, 31, "S2 level"),
    ] {
        match precision_validate(Precision { grid, level }) {
            Err(GeoError::InvalidValue { field: f, .. }) => assert_eq!(f, field),
            other => panic!("{grid:?} {level}: {other:?}"),
        }
    }
}

#[test]
fn a_size_maps_onto_the_finest_tall_enough_geohash() {
    assert_eq!(precision_at_least(1_000.0).unwrap(), geohash(5));
    assert_eq!(precision_at_least(0.0).unwrap(), geohash(12));
    assert!(matches!(
        precision_at_least(-1.0),
        Err(GeoError::InvalidValue { .. })
    ));
    let height = precision_min_cell_height_m(geohash(6)).unwrap().unwrap();
    assert!((height - 607.4).abs() < 0.1, "{height}");
    let h3 = Precision {
        grid: SnapGrid::H3,
        level: 9,
    };
    assert_eq!(precision_min_cell_height_m(h3).unwrap(), None);
    assert!(precision_min_cell_height_m(geohash(13)).is_err());
}

#[test]
fn coarsen_snaps_to_a_named_cell_with_a_bound() {
    let fix = coarsen(wgs84(57.64911, 10.40744), geohash(5)).unwrap();
    assert_eq!(fix.source.as_ref().unwrap().raw, "privacy:geohash:u4pru");
    let accuracy = fix.accuracy.unwrap();
    assert_eq!(accuracy.kind, AccuracyKind::Bound);
    assert!(accuracy.horizontal_m.unwrap() < 3_000.0);
    assert_eq!(coarsen(fix.coord, geohash(5)).unwrap(), fix);

    let h3 = Precision {
        grid: SnapGrid::H3,
        level: 9,
    };
    let fix = coarsen(wgs84(40.7128, -74.006), h3).unwrap();
    assert_eq!(fix.source.unwrap().raw, "privacy:h3:892a1072893ffff");
    let s2 = Precision {
        grid: SnapGrid::S2,
        level: 20,
    };
    assert!(coarsen(wgs84(40.7128, -74.006), s2).is_ok());

    let gcj = coordinate_new(39.9, 116.4, Crs::Gcj02).unwrap();
    assert!(matches!(
        coarsen(gcj, geohash(5)),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(matches!(
        coarsen(wgs84(0.0, 0.0), geohash(13)),
        Err(GeoError::InvalidValue { .. })
    ));
}

#[test]
fn zone_records_are_checked_and_measured_geodesically() {
    exclusion_zone_validate(circle(0.0, 0.0, 10.0)).unwrap();
    assert!(matches!(
        exclusion_zone_validate(circle(0.0, 0.0, f64::NAN)),
        Err(GeoError::InvalidValue { .. })
    ));
    let gcj = ExclusionZone::Circle {
        center: coordinate_new(39.9, 116.4, Crs::Gcj02).unwrap(),
        radius_m: 1.0,
    };
    assert!(matches!(
        exclusion_zone_validate(gcj),
        Err(GeoError::CrsMismatch { .. })
    ));
    let inverted = ExclusionZone::Bounds {
        bounds: GeoBounds {
            south: 10.0,
            west: 0.0,
            north: -10.0,
            east: 1.0,
            crs: Crs::Wgs84,
        },
    };
    assert!(exclusion_zone_validate(inverted).is_err());

    // Across the antimeridian, about 1.06 km apart.
    let zone = circle(-17.0, 179.995, 2_000.0);
    assert!(exclusion_zone_contains(zone, wgs84(-17.0, -179.995)).unwrap());
    assert!(!exclusion_zone_contains(zone, wgs84(-17.0, -179.97)).unwrap());
    let fiji = ExclusionZone::Bounds {
        bounds: GeoBounds {
            south: -21.0,
            west: 176.0,
            north: -12.0,
            east: -178.0,
            crs: Crs::Wgs84,
        },
    };
    assert!(exclusion_zone_contains(fiji, wgs84(-17.0, 180.0)).unwrap());
    let nad27 = coordinate_new(-17.0, 180.0, Crs::Nad27).unwrap();
    assert!(matches!(
        exclusion_zone_contains(fiji, nad27),
        Err(GeoError::CrsMismatch { .. })
    ));
}

#[test]
fn apply_passes_strips_or_coarsens() {
    let zones = vec![circle(10.0, 10.0, 100.0)];
    let mut fix = Fix {
        coord: wgs84(10.0, 10.0005),
        accuracy: None,
        timestamp: Some(UNIX_EPOCH),
        source: None,
    };
    assert_eq!(
        apply_exclusion_zones(fix.clone(), zones.clone(), ZoneAction::Strip).unwrap(),
        None
    );
    let coarse = apply_exclusion_zones(
        fix.clone(),
        zones.clone(),
        ZoneAction::Coarsen {
            precision: geohash(4),
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(coarse.timestamp, Some(UNIX_EPOCH));
    assert!(coarse.source.unwrap().raw.starts_with("privacy:geohash:"));

    fix.coord = wgs84(20.0, 20.0);
    assert_eq!(
        apply_exclusion_zones(fix.clone(), zones.clone(), ZoneAction::Strip).unwrap(),
        Some(fix.clone())
    );
    let bad = ZoneAction::Coarsen {
        precision: geohash(0),
    };
    assert!(matches!(
        apply_exclusion_zones(fix, zones, bad),
        Err(GeoError::InvalidValue { .. })
    ));
}
