//! Calls every cluster FFI export at least once with an asserted value, including
//! the failure each one documents.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use geocoordinates_ffi::*;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

fn world() -> GeoBounds {
    GeoBounds {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
        crs: Crs::Wgs84,
    }
}

fn bad_record() -> Coordinate {
    Coordinate {
        lat: 91.0,
        ..wgs84(0.0, 0.0)
    }
}

fn at(second: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(second)
}

fn fix(lat: f64, lon: f64, second: u64) -> Fix {
    Fix {
        coord: wgs84(lat, lon),
        accuracy: None,
        timestamp: Some(at(second)),
        source: None,
    }
}

fn is_field<T>(result: Result<T, GeoError>, expected: &str) -> bool {
    matches!(result, Err(GeoError::InvalidValue { field, .. }) if field == expected)
}

#[test]
fn centroid_flattens_its_bound() {
    let found = centroid(vec![wgs84(0.0, 179.0), wgs84(0.0, -179.0)]).unwrap();
    assert!((found.coord.lon.abs() - 180.0).abs() < 1e-9);
    assert!((found.max_error_m - 111_694.0).abs() < 1_000.0);
    assert!(is_field(centroid(vec![]), "points"));
    assert!(centroid(vec![bad_record()]).is_err());
    assert!(matches!(
        centroid(vec![coordinate_new(1.0, 1.0, Crs::Gcj02).unwrap()]),
        Err(GeoError::CrsMismatch { .. })
    ));
}

#[test]
fn dbscan_labels_noise_and_numbers_by_earliest_member() {
    let points = vec![
        wgs84(9.0, 9.0),
        wgs84(0.0, 0.0),
        wgs84(0.0, 0.0001),
        wgs84(0.0, -0.0001),
    ];
    let found = dbscan(points.clone(), 50.0, 2).unwrap();
    assert_eq!(found.labels[0], PointLabel::Noise);
    assert_eq!(found.labels[1], PointLabel::Cluster { id: 0 });
    let cluster = &found.clusters[0];
    assert_eq!((cluster.id, cluster.count), (0, 3));
    assert_eq!(cluster.members, [1, 2, 3]);
    assert!(cluster.centroid_max_error_m > 11.0 && cluster.centroid_max_error_m < 12.0);
    assert_eq!(cluster.bounds.crs, Crs::Wgs84);

    assert!(is_field(dbscan(points.clone(), -1.0, 2), "eps"));
    assert!(is_field(dbscan(points, 1.0, 0), "min_points"));
    assert!(dbscan(vec![bad_record()], 1.0, 1).is_err());
}

#[test]
fn cluster_validate_checks_a_hand_built_record() {
    let found = dbscan(vec![wgs84(1.0, 2.0)], 1.0, 1).unwrap();
    let good = found.clusters[0].clone();
    assert!(cluster_validate(good.clone()).is_ok());
    assert!(is_field(
        cluster_validate(Cluster {
            count: 2,
            ..good.clone()
        }),
        "count"
    ));
    assert!(is_field(
        cluster_validate(Cluster {
            count: 0,
            members: vec![],
            ..good.clone()
        }),
        "members"
    ));
    assert!(
        cluster_validate(Cluster {
            centroid_max_error_m: -1.0,
            ..good.clone()
        })
        .is_err()
    );
    assert!(
        cluster_validate(Cluster {
            centroid: bad_record(),
            ..good
        })
        .is_err()
    );
}

#[test]
fn grid_buckets_carry_their_cell() {
    let points = vec![wgs84(57.64911, 10.40744), wgs84(0.0, 0.0)];
    let found = bucket_by_geohash(points.clone(), 5).unwrap();
    assert_eq!(found[0].key, "u4pru");
    assert_eq!(found[1].cluster.members, [1]);
    assert!(bucket_by_geohash(points.clone(), 0).is_err());
    assert!(bucket_by_geohash(vec![bad_record()], 5).is_err());

    let found = bucket_by_h3(vec![wgs84(40.7128, -74.006)], 9).unwrap();
    assert_eq!(found[0].key, "892a1072893ffff");
    assert!(bucket_by_h3(points, 16).is_err());
    assert!(bucket_by_h3(vec![bad_record()], 5).is_err());
}

#[test]
fn visits_and_trips_cross_with_their_times() {
    let fixes = vec![
        fix(0.0, 0.0, 0),
        fix(0.0, 0.0001, 1_200),
        fix(5.0, 5.0, 1_300),
        fix(5.0, 5.0001, 2_500),
    ];
    let visits = segment_visits(fixes, 100.0, Duration::from_secs(600)).unwrap();
    assert_eq!(visits.len(), 2);
    assert_eq!(visits[0].cluster.members, [0, 1]);
    assert_eq!((visits[0].arrival, visits[0].departure), (at(0), at(1_200)));

    let trips = segment_trips(visits.clone(), Duration::from_secs(100)).unwrap();
    assert_eq!(trips.len(), 1);
    assert_eq!(trips[0].visits, [0, 1]);
    assert_eq!((trips[0].start, trips[0].end), (at(0), at(2_500)));
    assert_eq!(trips[0].bounds.north, 5.0);
    let split = segment_trips(visits.clone(), Duration::from_secs(99)).unwrap();
    assert_eq!(split.len(), 2);

    assert!(visit_validate(visits[0].clone()).is_ok());
    let flipped = Visit {
        arrival: at(9),
        departure: at(1),
        ..visits[0].clone()
    };
    assert!(is_field(visit_validate(flipped.clone()), "departure"));
    assert!(is_field(
        segment_trips(vec![flipped], Duration::ZERO),
        "departure"
    ));

    let no_time = Fix {
        timestamp: None,
        ..fix(0.0, 0.0, 0)
    };
    assert!(is_field(
        segment_visits(vec![no_time], 1.0, Duration::ZERO),
        "timestamp"
    ));
    assert!(is_field(
        segment_visits(vec![], -1.0, Duration::ZERO),
        "distance"
    ));
    let bad = Fix {
        coord: bad_record(),
        ..fix(0.0, 0.0, 0)
    };
    assert!(segment_visits(vec![bad], 1.0, Duration::ZERO).is_err());
}

#[test]
fn cluster_index_answers_and_updates() {
    let options = cluster_index_options_default();
    assert_eq!((options.radius_px, options.tile_px), (40.0, 512.0));

    let index = ClusterIndex::new(vec![wgs84(0.0, 179.99), wgs84(0.0, -179.99)], options).unwrap();
    assert_eq!((index.len(), index.is_empty()), (2, false));
    assert_eq!(index.crs(), Some(Crs::Wgs84));
    assert_eq!(index.coordinate(1), Some(wgs84(0.0, -179.99)));
    assert_eq!(index.coordinate(9), None);

    let top = index.clusters(world(), 2).unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!((top[0].count, top[0].zoom), (2, 2));
    assert!(top[0].centroid_max_error_m > 0.0);
    assert!(geo_bounds_crosses_antimeridian(top[0].bounds).unwrap());
    assert_eq!(index.leaves(top[0].id, 1, 1).unwrap().len(), 1);
    assert_eq!(index.leaves(top[0].id, 5, 0).unwrap().len(), 2);
    assert_eq!(index.children(top[0].id).unwrap()[0].zoom, 3);

    let added = index.insert(wgs84(40.0, 0.0)).unwrap();
    assert_eq!(added, 2);
    assert_eq!(index.clusters(world(), 2).unwrap().len(), 2);
    assert_eq!(index.remove(added).unwrap(), wgs84(40.0, 0.0));
    assert_eq!(index.clusters(world(), 2).unwrap().len(), 1);

    assert!(is_field(index.clusters(world(), 23), "zoom"));
    assert!(is_field(index.children(u64::MAX), "cluster_id"));
    assert!(is_field(index.leaves(u64::MAX, 1, 0), "cluster_id"));
    assert!(is_field(index.remove(99), "point_id"));
    assert!(index.insert(bad_record()).is_err());
    assert!(matches!(
        index.insert(coordinate_new(1.0, 1.0, Crs::Tokyo).unwrap()),
        Err(GeoError::CrsMismatch { .. })
    ));
    let bad_box = GeoBounds {
        south: 9.0,
        north: 0.0,
        ..world()
    };
    assert!(is_field(index.clusters(bad_box, 0), "south"));

    let empty = ClusterIndex::new(vec![], options).unwrap();
    assert!(empty.is_empty() && empty.crs().is_none());
    let bad = ClusterIndexOptions {
        radius_px: 0.0,
        ..options
    };
    assert!(is_field(ClusterIndex::new(vec![], bad), "radius_px"));
    let bad = ClusterIndexOptions {
        tile_px: f64::NAN,
        ..options
    };
    assert!(is_field(ClusterIndex::new(vec![], bad), "tile_px"));
    assert!(ClusterIndex::new(vec![bad_record()], options).is_err());
}
