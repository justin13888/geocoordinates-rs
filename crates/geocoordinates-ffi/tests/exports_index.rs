//! Calls every bounds and spatial-index FFI export at least once with an asserted
//! value, including the failure each one documents.

use geocoordinates_ffi::*;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

fn bounds(south: f64, west: f64, north: f64, east: f64) -> GeoBounds {
    GeoBounds {
        south,
        west,
        north,
        east,
        crs: Crs::Wgs84,
    }
}

fn bad_record() -> Coordinate {
    Coordinate {
        lat: 91.0,
        ..wgs84(0.0, 0.0)
    }
}

fn gcj() -> Coordinate {
    coordinate_new(1.0, 1.0, Crs::Gcj02).unwrap()
}

#[test]
fn validate_checks_a_hand_built_record() {
    assert!(geo_bounds_validate(bounds(0.0, 170.0, 1.0, -170.0)).is_ok());
    assert!(matches!(
        geo_bounds_validate(bounds(1.0, 0.0, 0.0, 0.0)),
        Err(GeoError::InvalidValue { field, .. }) if field == "south"
    ));
}

#[test]
fn from_points_takes_the_smallest_span() {
    let found = geo_bounds_from_points(vec![wgs84(1.0, 179.0), wgs84(2.0, -179.0)]).unwrap();
    assert_eq!(found, bounds(1.0, 179.0, 2.0, -179.0));
    assert!(geo_bounds_crosses_antimeridian(found).unwrap());
    assert!(!geo_bounds_crosses_antimeridian(bounds(0.0, 1.0, 1.0, 2.0)).unwrap());
    assert!(geo_bounds_crosses_antimeridian(bounds(2.0, 0.0, 1.0, 0.0)).is_err());
    assert!(matches!(
        geo_bounds_from_points(vec![]),
        Err(GeoError::InvalidValue { field, .. }) if field == "points"
    ));
    assert!(matches!(
        geo_bounds_from_points(vec![wgs84(0.0, 0.0), gcj()]),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(geo_bounds_from_points(vec![bad_record()]).is_err());
}

#[test]
fn contains_and_intersects_cross_the_antimeridian() {
    let crossing = bounds(-1.0, 170.0, 1.0, -170.0);
    assert!(geo_bounds_contains(crossing, wgs84(0.0, 180.0)).unwrap());
    assert!(!geo_bounds_contains(crossing, wgs84(0.0, 0.0)).unwrap());
    assert!(matches!(
        geo_bounds_contains(crossing, gcj()),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(geo_bounds_contains(crossing, bad_record()).is_err());
    assert!(geo_bounds_contains(bounds(1.0, 0.0, 0.0, 0.0), wgs84(0.0, 0.0)).is_err());

    assert!(geo_bounds_intersects(crossing, bounds(0.0, 175.0, 1.0, 180.0)).unwrap());
    assert!(!geo_bounds_intersects(crossing, bounds(0.0, 0.0, 1.0, 1.0)).unwrap());
    let other = GeoBounds {
        crs: Crs::Gcj02,
        ..crossing
    };
    assert!(matches!(
        geo_bounds_intersects(crossing, other),
        Err(GeoError::CrsMismatch { .. })
    ));
}

#[test]
fn union_center_and_expand_by_compute() {
    let west = bounds(0.0, 170.0, 1.0, 175.0);
    let east = bounds(2.0, -175.0, 3.0, -170.0);
    let both = geo_bounds_union(west, east).unwrap();
    assert_eq!(both, bounds(0.0, 170.0, 3.0, -170.0));
    let other = GeoBounds {
        crs: Crs::Gcj02,
        ..west
    };
    assert!(matches!(
        geo_bounds_union(west, other),
        Err(GeoError::CrsMismatch { .. })
    ));

    let center = geo_bounds_center(both).unwrap();
    assert_eq!((center.lat, center.lon), (1.5, 180.0 - 360.0));
    assert!(geo_bounds_center(bounds(2.0, 0.0, 1.0, 0.0)).is_err());

    let polar = geo_bounds_expand_by(bounds(89.9, 0.0, 89.95, 1.0), 50_000.0).unwrap();
    assert_eq!((polar.north, polar.west, polar.east), (90.0, -180.0, 180.0));
    assert!(matches!(
        geo_bounds_expand_by(west, -1.0),
        Err(GeoError::InvalidValue { field, .. }) if field == "distance"
    ));
    assert!(matches!(
        geo_bounds_expand_by(other, 1.0),
        Err(GeoError::CrsMismatch { .. })
    ));
}

#[test]
fn the_index_answers_exact_geodesic_queries() {
    let index =
        SpatialIndex::new(vec![wgs84(0.0, 179.9), wgs84(0.0, -179.9), wgs84(0.0, 0.0)]).unwrap();
    assert_eq!(index.len(), 3);
    assert!(!index.is_empty());
    assert_eq!(index.crs(), Some(Crs::Wgs84));

    let center = wgs84(0.0, -179.95);
    let nearest = index.nearest(center, 2).unwrap();
    assert_eq!((nearest[0].index, nearest[1].index), (1, 0));
    assert_eq!(nearest[0].coordinate, wgs84(0.0, -179.9));
    let exact = geodesic_distance_m(center, wgs84(0.0, -179.9)).unwrap();
    assert_eq!(nearest[0].distance_m, exact);

    let within = index.within(center, 50_000.0).unwrap();
    assert_eq!(within.len(), 2);
    assert_eq!(within[0], nearest[0]);
}

#[test]
fn the_index_reports_its_documented_failures() {
    let index = SpatialIndex::new(vec![wgs84(0.0, 0.0)]).unwrap();
    assert!(matches!(
        index.within(wgs84(0.0, 0.0), -1.0),
        Err(GeoError::InvalidValue { field, .. }) if field == "radius"
    ));
    assert!(matches!(
        index.within(gcj(), 1.0),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(matches!(
        index.nearest(gcj(), 1),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(index.within(bad_record(), 1.0).is_err());
    assert!(index.nearest(bad_record(), 1).is_err());
    assert!(matches!(
        SpatialIndex::new(vec![gcj()]),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(SpatialIndex::new(vec![bad_record()]).is_err());
}

#[test]
fn an_empty_index_is_valid_and_answers_nothing() {
    let index = SpatialIndex::new(vec![]).unwrap();
    assert_eq!(index.len(), 0);
    assert!(index.is_empty());
    assert_eq!(index.crs(), None);
    assert!(index.nearest(wgs84(0.0, 0.0), 3).unwrap().is_empty());
    assert!(index.within(wgs84(0.0, 0.0), 1.0).unwrap().is_empty());
}
