//! `centroid`, `dbscan` and the grid buckets: order independence, the antimeridian
//! and the poles, and agreement with brute force on small sets.
#![cfg(feature = "cluster")]

use geocoordinates::cluster::{Cluster, PointLabel, bucket_by_geohash, centroid, dbscan};
use geocoordinates::geodesy::geodesic_distance;
use geocoordinates::index::GeoBounds;
use geocoordinates::{Approx, Coordinate, Crs, Error, Length};
use proptest::prelude::*;

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

/// Points scattered near a place where planar reasoning breaks.
fn tricky_points() -> impl Strategy<Value = Vec<Coordinate>> {
    let center = prop_oneof![
        Just((0.0, 180.0)),
        Just((-17.0, -180.0)),
        Just((90.0, 0.0)),
        Just((-90.0, 0.0)),
        Just((48.85, 2.35)),
    ];
    center.prop_flat_map(|(lat, lon): (f64, f64)| {
        prop::collection::vec((-30_i32..30, -30_i32..30), 1..24).prop_map(move |offsets| {
            offsets
                .into_iter()
                .map(|(dy, dx)| {
                    let lat = (lat + f64::from(dy) * 1e-4).clamp(-90.0, 90.0);
                    let mut lon = lon + f64::from(dx) * 1e-4;
                    if lon > 180.0 {
                        lon -= 360.0;
                    }
                    if lon < -180.0 {
                        lon += 360.0;
                    }
                    wgs(lat, lon)
                })
                .collect()
        })
    })
}

/// A permutation of `0..len`, from a shuffle-driving seed.
fn permutation(len: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    let mut state = seed | 1;
    for i in (1..len).rev() {
        state = state.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
        order.swap(i, (state % (i as u64 + 1)) as usize);
    }
    order
}

/// The clusters as sets of points, in a form that ignores numbering and input order.
fn partition(points: &[Coordinate], clusters: &[Cluster]) -> Vec<Vec<(u64, u64)>> {
    let mut sets: Vec<Vec<(u64, u64)>> = clusters
        .iter()
        .map(|cluster| {
            let mut set: Vec<(u64, u64)> = cluster
                .members()
                .iter()
                .map(|member| {
                    (
                        points[*member].lat().to_bits(),
                        points[*member].lon().to_bits(),
                    )
                })
                .collect();
            set.sort_unstable();
            set
        })
        .collect();
    sets.sort();
    sets
}

/// DBSCAN from the full distance matrix, with the crate's border rule.
fn brute_force(points: &[Coordinate], eps_m: f64, min_points: usize) -> Vec<Vec<usize>> {
    let n = points.len();
    let near = |a: usize, b: usize| geodesic_distance(points[a], points[b]).unwrap().meters();
    let neighbors: Vec<Vec<usize>> = (0..n)
        .map(|a| (0..n).filter(|b| near(a, *b) <= eps_m).collect())
        .collect();
    let core: Vec<bool> = neighbors
        .iter()
        .map(|hits| hits.len() >= min_points)
        .collect();
    let mut group: Vec<Option<usize>> = vec![None; n];
    for start in 0..n {
        if !core[start] || group[start].is_some() {
            continue;
        }
        let mut stack = vec![start];
        group[start] = Some(start);
        while let Some(at) = stack.pop() {
            for &next in &neighbors[at] {
                if core[next] && group[next].is_none() {
                    group[next] = Some(start);
                    stack.push(next);
                }
            }
        }
    }
    for a in 0..n {
        if core[a] {
            continue;
        }
        let nearest = neighbors[a].iter().filter(|b| core[**b]).min_by(|x, y| {
            near(a, **x)
                .total_cmp(&near(a, **y))
                .then(points[**x].lat().total_cmp(&points[**y].lat()))
                .then(points[**x].lon().total_cmp(&points[**y].lon()))
        });
        group[a] = nearest.and_then(|b| group[*b]);
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slots: Vec<Option<usize>> = vec![None; n];
    for (a, assigned) in group.iter().enumerate() {
        if let Some(g) = *assigned {
            let slot = *slots[g].get_or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[slot].push(a);
        }
    }
    groups
}

proptest! {
    #[test]
    fn centroid_is_order_free_near_and_bounded(points in tricky_points(), seed in any::<u64>()) {
        let found = centroid(&points).unwrap();
        let reordered: Vec<Coordinate> =
            permutation(points.len(), seed).into_iter().map(|i| points[i]).collect();
        let again = centroid(&reordered).unwrap();
        prop_assert!(geodesic_distance(*found.value(), *again.value()).unwrap().meters() < 1e-3);

        // The bound holds, and it is tight to about 1% plus the round-off slack.
        let farthest = points
            .iter()
            .map(|point| geodesic_distance(*found.value(), *point).unwrap().meters())
            .fold(0.0, f64::max);
        let bound = found.max_error().meters();
        prop_assert!(bound >= farthest, "{bound} < {farthest}");
        prop_assert!(bound <= farthest * 1.011 + 1e-3, "{bound} > {farthest}");
    }

    #[test]
    fn dbscan_labels_do_not_depend_on_input_order(
        points in tricky_points(),
        seed in any::<u64>(),
        min_points in 1_usize..5,
    ) {
        let eps = Length::from_meters(40.0);
        let found = dbscan(&points, eps, min_points).unwrap();
        let reordered: Vec<Coordinate> =
            permutation(points.len(), seed).into_iter().map(|i| points[i]).collect();
        let again = dbscan(&reordered, eps, min_points).unwrap();
        prop_assert_eq!(
            partition(&points, found.clusters()),
            partition(&reordered, again.clusters())
        );
    }

    #[test]
    fn dbscan_agrees_with_brute_force(points in tricky_points(), min_points in 1_usize..5) {
        let found = dbscan(&points, Length::from_meters(40.0), min_points).unwrap();
        let members: Vec<Vec<usize>> =
            found.clusters().iter().map(|c| c.members().to_vec()).collect();
        prop_assert_eq!(members, brute_force(&points, 40.0, min_points));

        // Labels agree with the clusters, and every id counts up by earliest member.
        for (position, label) in found.labels().iter().enumerate() {
            match label {
                PointLabel::Noise => prop_assert!(
                    found.clusters().iter().all(|c| !c.members().contains(&position))
                ),
                PointLabel::Cluster { id } => {
                    prop_assert!(found.clusters()[*id].members().contains(&position));
                }
            }
        }
        for (id, cluster) in found.clusters().iter().enumerate() {
            prop_assert_eq!(cluster.id(), id);
            let held: Vec<Coordinate> = cluster.members().iter().map(|m| points[*m]).collect();
            prop_assert_eq!(cluster.bounds(), GeoBounds::from_points(held.clone()).unwrap());
            prop_assert_eq!(cluster.centroid(), centroid(&held).unwrap());
            prop_assert_eq!(cluster.count(), held.len());
        }
        let firsts: Vec<usize> = found.clusters().iter().map(|c| c.members()[0]).collect();
        prop_assert!(firsts.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn geohash_buckets_partition_the_input(points in tricky_points(), seed in any::<u64>()) {
        let buckets = bucket_by_geohash(&points, 7).unwrap();
        let mut seen: Vec<usize> = buckets
            .iter()
            .flat_map(|bucket| bucket.cluster().members().iter().copied())
            .collect();
        seen.sort_unstable();
        prop_assert_eq!(seen, (0..points.len()).collect::<Vec<_>>());

        let reordered: Vec<Coordinate> =
            permutation(points.len(), seed).into_iter().map(|i| points[i]).collect();
        let again = bucket_by_geohash(&reordered, 7).unwrap();
        let clusters = |list: &[geocoordinates::cluster::GridBucket]| -> Vec<Cluster> {
            list.iter().map(|bucket| bucket.cluster().clone()).collect()
        };
        prop_assert_eq!(
            partition(&points, &clusters(&buckets)),
            partition(&reordered, &clusters(&again))
        );
    }
}

#[test]
fn centroid_of_one_point_is_that_point() {
    let found = centroid(&[wgs(-33.0, 151.0)]).unwrap();
    assert!((found.value().lat() + 33.0).abs() < 1e-9);
    assert!((found.value().lon() - 151.0).abs() < 1e-9);
    assert!(found.max_error().meters() < 1e-3);
}

#[test]
fn centroid_finds_the_middle_on_the_ellipsoid_of_the_crs() {
    // The middle of a meridian arc, reached from either side of the pole.
    let over_pole = centroid(&[wgs(89.0, 0.0), wgs(89.0, 180.0)]).unwrap();
    assert!((over_pole.value().lat() - 90.0).abs() < 1e-9);
    let side = centroid(&[wgs(10.0, 20.0), wgs(20.0, 20.0)]).unwrap();
    assert!((side.value().lat() - 15.0).abs() < 1e-9);
    assert!((side.value().lon() - 20.0).abs() < 1e-9);

    let tokyo = |lat, lon| Coordinate::new(lat, lon, Crs::Tokyo).unwrap();
    let found = centroid(&[tokyo(35.0, 139.0), tokyo(35.0, 140.0)]).unwrap();
    assert_eq!(found.value().crs(), Crs::Tokyo);
    assert!((found.value().lon() - 139.5).abs() < 1e-9);
}

#[test]
fn centroid_treats_nearly_cancelling_points_by_their_mean_length() {
    // Two points about 1e-9 rad short of antipodal: a mean length of 5e-10.
    let short = 1e-9_f64.to_degrees();
    let nearly = [wgs(0.0, 0.0), wgs(0.0, 180.0 - short)];
    assert!(matches!(
        centroid(&nearly),
        Err(Error::InvalidValue {
            field: "points",
            ..
        })
    ));
    // Ten times further from antipodal, the same pair has a direction.
    let clear = [wgs(0.0, 0.0), wgs(0.0, 180.0 - 100.0 * short)];
    assert!(centroid(&clear).is_ok());
}

#[test]
fn centroid_rejects_what_has_no_mean() {
    for points in [
        vec![],
        vec![wgs(10.0, 20.0), wgs(-10.0, -160.0)],
        vec![wgs(90.0, 0.0), wgs(-90.0, 0.0)],
        vec![wgs(0.0, 0.0), wgs(0.0, 120.0), wgs(0.0, -120.0)],
    ] {
        assert!(
            matches!(
                centroid(&points),
                Err(Error::InvalidValue {
                    field: "points",
                    ..
                })
            ),
            "{points:?}"
        );
    }
    assert!(matches!(
        centroid(&[Coordinate::gcj02(1.0, 1.0).unwrap()]),
        Err(Error::CrsMismatch { .. })
    ));
    let tokyo = Coordinate::new(1.0, 1.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        centroid(&[wgs(1.0, 1.0), tokyo]),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn dbscan_counts_the_point_itself_and_includes_the_eps_distance() {
    let (a, b) = (wgs(0.0, 0.0), wgs(0.0, 0.001));
    let apart = geodesic_distance(a, b).unwrap();
    // At exactly the distance they are neighbors; just under it they are not.
    assert_eq!(dbscan(&[a, b], apart, 2).unwrap().clusters().len(), 1);
    let under = Length::from_meters(apart.meters() - 1e-3);
    assert_eq!(dbscan(&[a, b], under, 2).unwrap().clusters().len(), 0);
    // A point is its own neighbor, so a minimum of one makes every point core.
    assert_eq!(dbscan(&[a, b], under, 1).unwrap().clusters().len(), 2);
    assert_eq!(
        dbscan(&[a, b], under, 3).unwrap().labels(),
        [PointLabel::Noise, PointLabel::Noise]
    );
    // Zero eps groups exact duplicates only.
    let zero = Length::from_meters(0.0);
    assert_eq!(
        dbscan(&[a, a, b], zero, 2).unwrap().clusters()[0].members(),
        [0, 1]
    );
}

#[test]
fn dbscan_gives_a_border_point_to_its_nearest_core_point() {
    // Two dense groups with one point between them, nearer the eastern one. Input
    // order puts the western group first, so a first-come rule would pick wrongly.
    let at = |lon: f64| wgs(0.0, lon);
    let west = [at(-0.0006), at(-0.00062), at(-0.00063), at(-0.00064)];
    let east = [at(0.0005), at(0.00062), at(0.00063), at(0.00064)];
    let between = at(0.0);
    let points: Vec<Coordinate> = west
        .iter()
        .chain(&[between])
        .chain(&east)
        .copied()
        .collect();
    let found = dbscan(&points, Length::from_meters(67.0), 4).unwrap();
    assert_eq!(found.clusters().len(), 2);
    assert_eq!(found.labels()[4], PointLabel::Cluster { id: 1 });
    assert_eq!(found.clusters()[1].members(), [4, 5, 6, 7, 8]);

    // Equidistant cores: the lower longitude wins whichever comes first.
    let left = [at(-0.0005), at(-0.00056), at(-0.00057), at(-0.00058)];
    let right = [at(0.0005), at(0.00056), at(0.00057), at(0.00058)];
    let eps = Length::from_meters(56.5);
    let tied: Vec<Coordinate> = left
        .iter()
        .chain(&[between])
        .chain(&right)
        .copied()
        .collect();
    let found = dbscan(&tied, eps, 4).unwrap();
    assert_eq!(found.clusters()[0].members(), [0, 1, 2, 3, 4]);
    let flipped: Vec<Coordinate> = right
        .iter()
        .chain(&[between])
        .chain(&left)
        .copied()
        .collect();
    let found = dbscan(&flipped, eps, 4).unwrap();
    assert_eq!(found.clusters()[0].members(), [0, 1, 2, 3]);
    assert_eq!(found.clusters()[1].members(), [4, 5, 6, 7, 8]);
}

#[test]
fn dbscan_handles_empty_input_and_chains() {
    let found = dbscan(&[], Length::from_meters(1.0), 1).unwrap();
    assert!(found.labels().is_empty() && found.clusters().is_empty());
    assert!(matches!(
        dbscan(&[], Length::from_meters(f64::NAN), 1),
        Err(Error::InvalidValue { field: "eps", .. })
    ));
    assert!(matches!(
        dbscan(&[], Length::from_meters(f64::INFINITY), 1),
        Err(Error::InvalidValue { field: "eps", .. })
    ));
    assert!(matches!(
        dbscan(
            &[Coordinate::gcj02(1.0, 1.0).unwrap()],
            Length::from_meters(1.0),
            1
        ),
        Err(Error::CrsMismatch { .. })
    ));

    // Density-reachable through a chain, though the ends are far apart.
    let chain: Vec<Coordinate> = (0..20).map(|i| wgs(0.0, f64::from(i) * 0.0005)).collect();
    let found = dbscan(&chain, Length::from_meters(60.0), 2).unwrap();
    assert_eq!(found.clusters().len(), 1);
    assert_eq!(found.clusters()[0].count(), 20);
}

#[test]
fn dbscan_clusters_across_the_pole() {
    let points = [wgs(89.9999, 0.0), wgs(89.9999, 180.0), wgs(0.0, 0.0)];
    let found = dbscan(&points, Length::from_meters(50.0), 2).unwrap();
    assert_eq!(found.clusters()[0].members(), [0, 1]);
    assert!((found.clusters()[0].centroid().value().lat() - 90.0).abs() < 1e-6);
}

#[test]
fn dbscan_reports_a_cluster_with_no_mean() {
    // An eps around half the planet joins antipodes.
    let points = [wgs(10.0, 20.0), wgs(-10.0, -160.0)];
    assert!(matches!(
        dbscan(&points, Length::from_meters(21_000_000.0), 2),
        Err(Error::InvalidValue {
            field: "points",
            ..
        })
    ));
}

#[test]
fn cluster_new_checks_its_parts() {
    let point = wgs(1.0, 2.0);
    let centroid = Approx::new(point, Length::from_meters(0.0)).unwrap();
    let bounds = GeoBounds::from_points([point]).unwrap();
    assert!(Cluster::new(0, centroid, bounds, vec![1]).is_ok());
    for members in [vec![], vec![1, 1], vec![2, 1]] {
        assert!(matches!(
            Cluster::new(0, centroid, bounds, members),
            Err(Error::InvalidValue {
                field: "members",
                ..
            })
        ));
    }
    let tokyo = GeoBounds::new(1.0, 2.0, 1.0, 2.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        Cluster::new(0, centroid, tokyo, vec![1]),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn geohash_buckets_split_at_the_antimeridian_and_check_their_arguments() {
    let points = [wgs(0.0, 179.9999), wgs(0.0, -179.9999)];
    assert_eq!(bucket_by_geohash(&points, 4).unwrap().len(), 2);
    assert!(matches!(
        bucket_by_geohash(&[], 0),
        Err(Error::InvalidValue { .. })
    ));
    assert!(bucket_by_geohash(&[], 4).unwrap().is_empty());
}

#[cfg(feature = "h3")]
#[test]
fn h3_buckets_do_not_split_at_the_antimeridian() {
    let points = [wgs(0.0, 179.9999), wgs(0.0, -179.9999), wgs(40.0, 0.0)];
    let buckets = geocoordinates::cluster::bucket_by_h3(&points, 3).unwrap();
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].cluster().members(), [0, 1]);
    assert_eq!(buckets[1].cluster().id(), 1);
    assert!(geocoordinates::cluster::bucket_by_h3(&[], 16).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_rejects_invalid_values() {
    use geocoordinates::cluster::Dbscan;

    let points = [wgs(0.0, 0.0), wgs(0.0, 0.0001), wgs(5.0, 5.0)];
    let found = dbscan(&points, Length::from_meters(50.0), 2).unwrap();
    let json = serde_json::to_string(&found).unwrap();
    assert_eq!(serde_json::from_str::<Dbscan>(&json).unwrap(), found);

    let tampered = json.replacen("\"members\":[0,1]", "\"members\":[1,0]", 1);
    assert!(serde_json::from_str::<Dbscan>(&tampered).is_err());
    let mismatch = json.replacen("\"Noise\"", "{\"Cluster\":{\"id\":0}}", 1);
    assert!(serde_json::from_str::<Dbscan>(&mismatch).is_err());
    let unlabelled = json.replacen("\"members\":[0,1]", "\"members\":[0,9]", 1);
    assert!(serde_json::from_str::<Dbscan>(&unlabelled).is_err());
    let misnumbered = json.replacen("\"id\":0,\"centroid\"", "\"id\":4,\"centroid\"", 1);
    assert!(serde_json::from_str::<Dbscan>(&misnumbered).is_err());

    let bucket = bucket_by_geohash(&points, 3).unwrap();
    let bucket_json = serde_json::to_string(&bucket).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<geocoordinates::cluster::GridBucket>>(&bucket_json).unwrap(),
        bucket
    );
}
