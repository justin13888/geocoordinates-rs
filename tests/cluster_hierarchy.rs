//! `ClusterIndex`: order independence, updates that reach the rebuild's result bit
//! for bit, agreement with a brute-force model, and the antimeridian and the poles.
#![cfg(feature = "cluster")]

use geocoordinates::cluster::{ClusterIndex, ClusterIndexOptions, MAX_ZOOM, MapCluster};
use geocoordinates::geodesy::geodesic_distance;
use geocoordinates::index::GeoBounds;
use geocoordinates::{Coordinate, Crs, Error};
use proptest::prelude::*;

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn world() -> GeoBounds {
    GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap()
}

fn tight() -> ClusterIndexOptions {
    // A small radius brings merges into the deep zooms too.
    ClusterIndexOptions::default()
        .with_radius_px(16.0)
        .with_tile_px(256.0)
}

/// Points quantized so exact duplicates occur, around awkward places.
fn points() -> impl Strategy<Value = Vec<Coordinate>> {
    let center = prop_oneof![
        Just((0.0, 180.0)),
        Just((88.0, -180.0)),
        Just((-90.0, 0.0)),
        Just((48.85, 2.35)),
    ];
    let spread = prop_oneof![Just(1e-1), Just(1e-3), Just(1e-5)];
    (center, spread).prop_flat_map(|((lat, lon), spread): ((f64, f64), f64)| {
        prop::collection::vec((-20_i32..20, -20_i32..20), 0..40).prop_map(move |offsets| {
            offsets
                .into_iter()
                .map(|(dy, dx)| {
                    let lat = (lat + f64::from(dy) * spread).clamp(-90.0, 90.0);
                    let mut lon = lon + f64::from(dx) * spread;
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

/// One cluster reduced to what must match bit for bit, members as positions.
type Summary = (u64, [u64; 3], [u64; 4], Vec<(u64, u64)>);

fn summary(index: &ClusterIndex, pin: &MapCluster) -> Summary {
    let bounds = pin.bounds();
    let mut members: Vec<(u64, u64)> = index
        .leaves(pin.id(), usize::MAX, 0)
        .unwrap()
        .into_iter()
        .map(|leaf| {
            let at = index.coordinate(leaf).unwrap();
            (at.lat().to_bits(), at.lon().to_bits())
        })
        .collect();
    members.sort_unstable();
    (
        pin.count(),
        [
            pin.centroid().value().lat().to_bits(),
            pin.centroid().value().lon().to_bits(),
            pin.centroid().max_error().meters().to_bits(),
        ],
        [
            bounds.south().to_bits(),
            bounds.west().to_bits(),
            bounds.north().to_bits(),
            bounds.east().to_bits(),
        ],
        members,
    )
}

/// Every zoom's clusters, in a form free of ids.
fn snapshot(index: &ClusterIndex) -> Vec<Vec<Summary>> {
    (0..=MAX_ZOOM)
        .map(|zoom| {
            let mut level: Vec<Summary> = index
                .clusters(world(), zoom)
                .unwrap()
                .iter()
                .map(|pin| summary(index, pin))
                .collect();
            level.sort();
            level
        })
        .collect()
}

fn permuted(points: &[Coordinate], seed: u64) -> Vec<Coordinate> {
    let mut list = points.to_vec();
    let mut state = seed | 1;
    for i in (1..list.len()).rev() {
        state = state.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
        list.swap(i, (state % (i as u64 + 1)) as usize);
    }
    list
}

/// The hierarchy by its definition, in plain quadratic code.
fn brute_force(points: &[Coordinate], options: ClusterIndexOptions) -> Vec<Vec<Vec<usize>>> {
    #[derive(Clone)]
    struct Item {
        key: (f64, f64, usize),
        members: Vec<usize>,
        x: f64,
        y: f64,
        sum: [f64; 3],
    }
    let xy = |lat: f64, lon: f64| {
        let x = (lon + 180.0) / 360.0;
        let sin = lat.to_radians().sin();
        let y = 0.5 - ((1.0 + sin) / (1.0 - sin)).ln() / (4.0 * std::f64::consts::PI);
        (if x >= 1.0 { x - 1.0 } else { x }, y.clamp(0.0, 1.0))
    };
    let order = |a: &Item, b: &Item| {
        a.key
            .0
            .total_cmp(&b.key.0)
            .then(a.key.1.total_cmp(&b.key.1))
            .then(a.key.2.cmp(&b.key.2))
    };
    let mut items: Vec<Item> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (x, y) = xy(p.lat(), p.lon());
            let (la, lo) = (p.lat().to_radians(), p.lon().to_radians());
            Item {
                key: (p.lat(), p.lon(), i),
                members: vec![i],
                x,
                y,
                sum: [la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()],
            }
        })
        .collect();
    items.sort_by(order);
    let mut levels = vec![Vec::new(); usize::from(MAX_ZOOM) + 1];
    for zoom in (0..=MAX_ZOOM).rev() {
        let radius = options.radius_px / options.tile_px / 2.0_f64.powi(i32::from(zoom));
        let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
        for (position, item) in items.iter().enumerate() {
            let seed = groups.iter_mut().find(|(seed, _)| {
                let at = &items[*seed];
                let dx = (at.x - item.x).abs();
                let (dx, dy) = (dx.min(1.0 - dx), at.y - item.y);
                dx * dx + dy * dy <= radius * radius
            });
            match seed {
                Some((_, group)) => group.push(position),
                None => groups.push((position, vec![position])),
            }
        }
        items = groups
            .into_iter()
            .map(|(seed, group)| {
                if group.len() == 1 {
                    return items[seed].clone();
                }
                let mut sum = [0.0; 3];
                let mut members = Vec::new();
                for position in &group {
                    let part = items[*position].sum;
                    sum = [sum[0] + part[0], sum[1] + part[1], sum[2] + part[2]];
                    members.extend_from_slice(&items[*position].members);
                }
                let lat = sum[2].atan2(sum[0].hypot(sum[1])).to_degrees();
                let lon = sum[1].atan2(sum[0]).to_degrees();
                let (x, y) = xy(lat, lon);
                Item {
                    key: items[seed].key,
                    members,
                    x,
                    y,
                    sum,
                }
            })
            .collect();
        let mut level: Vec<Vec<usize>> = items
            .iter()
            .map(|item| {
                let mut members = item.members.clone();
                members.sort_unstable();
                members
            })
            .collect();
        level.sort();
        levels[usize::from(zoom)] = level;
    }
    levels
}

proptest! {
    // A full snapshot per step makes shrinking slow, and a failure names its seed.
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 32,
        ..ProptestConfig::default()
    })]

    #[test]
    fn result_does_not_depend_on_input_order(points in points(), seed in any::<u64>()) {
        let index = ClusterIndex::new(points.iter().copied(), tight()).unwrap();
        let again = ClusterIndex::new(permuted(&points, seed), tight()).unwrap();
        prop_assert_eq!(snapshot(&index), snapshot(&again));
    }

    #[test]
    fn agrees_with_the_definition(points in points()) {
        let index = ClusterIndex::new(points.iter().copied(), tight()).unwrap();
        let expected = brute_force(&points, tight());
        for zoom in 0..=MAX_ZOOM {
            let mut level: Vec<Vec<usize>> = index
                .clusters(world(), zoom)
                .unwrap()
                .iter()
                .map(|pin| {
                    let mut leaves: Vec<usize> = index
                        .leaves(pin.id(), usize::MAX, 0)
                        .unwrap()
                        .into_iter()
                        .map(|leaf| leaf as usize)
                        .collect();
                    leaves.sort_unstable();
                    leaves
                })
                .collect();
            level.sort();
            prop_assert_eq!(&level, &expected[usize::from(zoom)], "zoom {}", zoom);
        }
    }

    #[test]
    fn inserts_reach_the_rebuilds_result(points in points(), seed in any::<u64>()) {
        let mut index = ClusterIndex::new([], tight()).unwrap();
        for point in permuted(&points, seed) {
            index.insert(point).unwrap();
        }
        let rebuilt = ClusterIndex::new(points.iter().copied(), tight()).unwrap();
        prop_assert_eq!(snapshot(&index), snapshot(&rebuilt));
    }

    #[test]
    fn removals_reach_the_rebuilds_result(
        points in points(),
        seed in any::<u64>(),
        keep in 0.0_f64..1.0,
    ) {
        let mut index = ClusterIndex::new(points.iter().copied(), tight()).unwrap();
        let order: Vec<usize> = {
            let positions: Vec<Coordinate> = (0..points.len())
                .map(|i| wgs(0.0, i as f64 / 1e3))
                .collect();
            permuted(&positions, seed)
                .into_iter()
                .map(|at| (at.lon() * 1e3).round() as usize)
                .collect()
        };
        let kept = (points.len() as f64 * keep) as usize;
        let (stay, go) = order.split_at(kept);
        for id in go {
            prop_assert_eq!(index.remove(*id as u64).unwrap(), points[*id]);
        }
        let mut stay = stay.to_vec();
        stay.sort_unstable();
        let rest: Vec<Coordinate> = stay.iter().map(|i| points[*i]).collect();
        let rebuilt = ClusterIndex::new(rest, tight()).unwrap();
        prop_assert_eq!(snapshot(&index), snapshot(&rebuilt));
        prop_assert_eq!(index.len(), stay.len());
    }

    #[test]
    fn a_cluster_tells_the_truth_about_its_points(points in points()) {
        prop_assume!(!points.is_empty());
        let index = ClusterIndex::new(points.iter().copied(), tight()).unwrap();
        for zoom in [0, 4, 9, 15, MAX_ZOOM] {
            let pins = index.clusters(world(), zoom).unwrap();
            let total: u64 = pins.iter().map(MapCluster::count).sum();
            prop_assert_eq!(total as usize, points.len());
            for pin in pins {
                prop_assert_eq!(pin.zoom(), zoom);
                let centroid = pin.centroid();
                let leaves = index.leaves(pin.id(), usize::MAX, 0).unwrap();
                prop_assert_eq!(leaves.len() as u64, pin.count());
                for leaf in leaves {
                    let at = index.coordinate(leaf).unwrap();
                    prop_assert!(pin.bounds().contains(at).unwrap());
                    let apart = geodesic_distance(*centroid.value(), at).unwrap().meters();
                    prop_assert!(
                        apart <= centroid.max_error().meters() + 1e-3,
                        "{} > {}", apart, centroid.max_error().meters()
                    );
                }
                let children = index.children(pin.id()).unwrap();
                let held: u64 = children.iter().map(MapCluster::count).sum();
                prop_assert_eq!(held, pin.count());
                prop_assert!(children.iter().all(|child| child.zoom() == zoom + 1));
            }
        }
    }

    #[test]
    fn a_viewport_finds_what_a_scan_finds(
        points in points(),
        south in -90.0_f64..90.0,
        north in -90.0_f64..90.0,
        west in -180.0_f64..180.0,
        east in -180.0_f64..180.0,
        zoom in 0..=MAX_ZOOM,
    ) {
        let (south, north) = (south.min(north), south.max(north));
        let bounds = GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap();
        let index = ClusterIndex::new(points.iter().copied(), tight()).unwrap();
        let expected: Vec<MapCluster> = index
            .clusters(world(), zoom)
            .unwrap()
            .into_iter()
            .filter(|pin| bounds.contains(*pin.centroid().value()).unwrap())
            .collect();
        prop_assert_eq!(index.clusters(bounds, zoom).unwrap(), expected);
    }
}

#[test]
fn merges_across_the_antimeridian_and_splits_with_zoom() {
    let index = ClusterIndex::new(
        [wgs(0.0, 179.99), wgs(0.0, -179.99)],
        ClusterIndexOptions::default(),
    )
    .unwrap();
    let top = index.clusters(world(), 0).unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].count(), 2);
    assert!(top[0].bounds().crosses_antimeridian());
    assert!((top[0].centroid().value().lon().abs() - 180.0).abs() < 1e-9);
    // 0.02° is 40 px, the default radius, between zooms 10 and 11.
    assert_eq!(index.clusters(world(), 10).unwrap().len(), 1);
    assert_eq!(index.clusters(world(), 11).unwrap().len(), 2);
}

#[test]
fn the_radius_is_exact_in_pixels() {
    // Exactly 64 px apart on 256-px tiles at zoom 0 is a quarter of the world.
    let options = ClusterIndexOptions::default()
        .with_radius_px(64.0)
        .with_tile_px(256.0);
    let index = ClusterIndex::new([wgs(0.0, 0.0), wgs(0.0, 90.0)], options).unwrap();
    assert_eq!(index.clusters(world(), 0).unwrap().len(), 1);
    assert_eq!(index.clusters(world(), 1).unwrap().len(), 2);

    let wider = ClusterIndexOptions::default()
        .with_radius_px(63.9)
        .with_tile_px(256.0);
    let index = ClusterIndex::new([wgs(0.0, 0.0), wgs(0.0, 90.0)], wider).unwrap();
    assert_eq!(index.clusters(world(), 0).unwrap().len(), 2);
}

#[test]
fn polar_points_flatten_onto_the_maps_edge() {
    // Both past the Mercator limit on opposite meridians: far apart on the map, so
    // separate even though they are neighbors on the ground.
    let index = ClusterIndex::new([wgs(89.9, 0.0), wgs(89.9, 180.0)], tight()).unwrap();
    assert_eq!(index.clusters(world(), 3).unwrap().len(), 2);

    // Past the limit on one meridian they share the edge, whatever their latitude.
    let index = ClusterIndex::new([wgs(86.0, 10.0), wgs(89.9, 10.0)], tight()).unwrap();
    assert_eq!(index.clusters(world(), MAX_ZOOM).unwrap().len(), 1);

    // A box reaching the pole holds a centroid exactly on it, at any longitude.
    let index = ClusterIndex::new([wgs(90.0, 37.0)], tight()).unwrap();
    let cap = GeoBounds::new(80.0, -10.0, 90.0, 10.0, Crs::Wgs84).unwrap();
    assert_eq!(index.clusters(cap, 3).unwrap().len(), 1);
}

#[test]
fn a_huge_radius_survives_points_that_cancel() {
    // The whole world within one radius: antipodes merge, and the seed stands in for
    // the mean direction they lack.
    let options = ClusterIndexOptions::default()
        .with_radius_px(5_000.0)
        .with_tile_px(256.0);
    let index = ClusterIndex::new([wgs(10.0, 20.0), wgs(-10.0, -160.0)], options).unwrap();
    let top = index.clusters(world(), 0).unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(*top[0].centroid().value(), wgs(-10.0, -160.0));
    let spread = top[0].centroid().max_error().meters();
    assert!(spread > 19_900_000.0, "{spread}");
}

#[test]
fn children_and_leaves_walk_down_to_the_points() {
    let points = [
        wgs(0.0, 0.0),
        wgs(0.0, 0.001),
        wgs(0.0, 0.002),
        wgs(1.0, 1.0),
    ];
    let index = ClusterIndex::new(points, tight()).unwrap();
    let top = index.clusters(world(), 0).unwrap();
    assert_eq!(top.len(), 1);

    // Walk one root to the bottom through `children`.
    let mut seen = Vec::new();
    let mut stack = vec![top[0]];
    while let Some(pin) = stack.pop() {
        let children = index.children(pin.id()).unwrap();
        // Each step goes one zoom deeper, which also keeps this walk finite.
        assert!(children.iter().all(|child| child.zoom() == pin.zoom() + 1));
        if children.is_empty() {
            assert_eq!(pin.zoom(), MAX_ZOOM + 1);
            assert_eq!(pin.centroid().max_error().meters(), 0.0);
            seen.extend(index.leaves(pin.id(), 9, 0).unwrap());
        }
        stack.extend(children);
    }
    seen.sort_unstable();
    assert_eq!(seen, [0, 1, 2, 3]);

    // Pages cut anywhere, and cover the set exactly once.
    let id = top[0].id();
    let all = index.leaves(id, 10, 0).unwrap();
    assert_eq!(all.len(), 4);
    for offset in 0..5 {
        for limit in 0..5 {
            let page = index.leaves(id, limit, offset).unwrap();
            let end = (offset + limit).min(4);
            let want: &[u64] = if offset >= 4 { &[] } else { &all[offset..end] };
            assert_eq!(page, want, "limit {limit} offset {offset}");
        }
    }
}

#[test]
fn updates_handle_every_id_and_crs_edge() {
    let mut index = ClusterIndex::new([], ClusterIndexOptions::default()).unwrap();
    assert!(index.is_empty() && index.crs().is_none());
    assert!(index.clusters(world(), 0).unwrap().is_empty());
    assert!(index.coordinate(0).is_none());
    assert!(matches!(
        index.children(0),
        Err(Error::InvalidValue {
            field: "cluster_id",
            ..
        })
    ));
    assert!(matches!(
        index.leaves(0, 1, 0),
        Err(Error::InvalidValue {
            field: "cluster_id",
            ..
        })
    ));
    assert!(matches!(
        index.remove(0),
        Err(Error::InvalidValue {
            field: "point_id",
            ..
        })
    ));
    // An id that does not fit the id space names nothing.
    assert!(index.coordinate(u64::MAX).is_none());

    let first = index.insert(wgs(1.0, 2.0)).unwrap();
    assert_eq!(index.len(), 1);
    assert!(!index.is_empty());
    assert_eq!(index.crs(), Some(Crs::Wgs84));
    let tokyo = Coordinate::new(1.0, 2.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        index.insert(tokyo),
        Err(Error::CrsMismatch { .. })
    ));
    assert_eq!(index.len(), 1);

    // Emptying the index frees its CRS but keeps ids unique.
    assert_eq!(index.remove(first).unwrap(), wgs(1.0, 2.0));
    assert!(index.is_empty() && index.crs().is_none());
    assert!(index.clusters(world(), 0).unwrap().is_empty());
    let second = index.insert(tokyo).unwrap();
    assert_eq!(second, first + 1);
    assert_eq!(index.crs(), Some(Crs::Tokyo));
    assert!(index.coordinate(first).is_none());
    assert!(matches!(
        index.clusters(world(), 0),
        Err(Error::CrsMismatch { .. })
    ));
    let tokyo_world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Tokyo).unwrap();
    assert_eq!(
        index.clusters(tokyo_world, 0).unwrap()[0]
            .centroid()
            .value()
            .crs(),
        Crs::Tokyo
    );
    let merged = index
        .insert(Coordinate::new(1.0, 2.0001, Crs::Tokyo).unwrap())
        .unwrap();
    assert_eq!(
        index
            .leaves(index.clusters(tokyo_world, 0).unwrap()[0].id(), 9, 0)
            .unwrap()
            .len(),
        2
    );
    let _ = merged;
}

#[test]
fn construction_checks_options_and_points() {
    let point = wgs(1.0, 2.0);
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            ClusterIndex::new([point], ClusterIndexOptions::default().with_radius_px(bad)),
            Err(Error::InvalidValue {
                field: "radius_px",
                ..
            })
        ));
        assert!(matches!(
            ClusterIndex::new([point], ClusterIndexOptions::default().with_tile_px(bad)),
            Err(Error::InvalidValue {
                field: "tile_px",
                ..
            })
        ));
    }
    let tokyo = Coordinate::new(1.0, 2.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        ClusterIndex::new([point, tokyo], ClusterIndexOptions::default()),
        Err(Error::CrsMismatch { .. })
    ));
    // A radius too small to matter still gets a bounded grid.
    let tiny = ClusterIndexOptions::default().with_radius_px(1e-300);
    let index = ClusterIndex::new([point, wgs(1.0, 2.0001)], tiny).unwrap();
    assert_eq!(index.clusters(world(), 0).unwrap().len(), 2);
}

#[test]
fn ids_name_the_zoom_and_the_seed_point() {
    let points = [wgs(1.0, 2.0), wgs(1.0, 2.0)];
    let one = ClusterIndex::new(points, ClusterIndexOptions::default()).unwrap();
    let two = ClusterIndex::new(points, ClusterIndexOptions::default()).unwrap();
    assert_eq!(
        one.clusters(world(), 7).unwrap()[0].id(),
        two.clusters(world(), 7).unwrap()[0].id()
    );
    assert_ne!(
        one.clusters(world(), 7).unwrap()[0].id(),
        one.clusters(world(), 8).unwrap()[0].id()
    );
}

#[cfg(feature = "serde")]
#[test]
fn map_clusters_and_options_serde() {
    let index = ClusterIndex::new([wgs(1.0, 2.0)], ClusterIndexOptions::default()).unwrap();
    let pin = index.clusters(world(), 2).unwrap()[0];
    let json = serde_json::to_string(&pin).unwrap();
    assert_eq!(serde_json::from_str::<MapCluster>(&json).unwrap(), pin);
    let bad_zoom = json.replacen("\"zoom\":2", "\"zoom\":3", 1);
    assert!(serde_json::from_str::<MapCluster>(&bad_zoom).is_err());
    let bad_count = json.replacen("\"count\":1", "\"count\":0", 1);
    assert!(serde_json::from_str::<MapCluster>(&bad_count).is_err());
    let mut mixed = serde_json::to_value(pin).unwrap();
    mixed["bounds"]["crs"] = serde_json::json!("Tokyo");
    let message = serde_json::from_value::<MapCluster>(mixed)
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("CRS") || message.contains("Tokyo"),
        "{message}"
    );

    let options: ClusterIndexOptions = serde_json::from_str("{\"radius_px\":9.0}").unwrap();
    assert_eq!(options, ClusterIndexOptions::default().with_radius_px(9.0));
}
