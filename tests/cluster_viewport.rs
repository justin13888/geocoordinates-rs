//! `ClusterIndex::clusters` over many pins, where a box walks the grid and not the
//! whole level: it must lose nothing a scan of the level finds.
#![cfg(feature = "cluster")]

use geocoordinates::cluster::{ClusterIndex, ClusterIndexOptions, MapCluster};
use geocoordinates::index::GeoBounds;
use geocoordinates::{Coordinate, Crs};

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn world() -> GeoBounds {
    GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap()
}

fn tight() -> ClusterIndexOptions {
    ClusterIndexOptions::default()
        .with_radius_px(16.0)
        .with_tile_px(256.0)
}

#[test]
fn viewports_over_many_pins_lose_nothing() {
    // Thousands of distinct pins, so every box below takes the grid path at these
    // zooms, with a pin on each pole and on both names of the antimeridian.
    let mut points: Vec<Coordinate> = (0..6_000_u32)
        .map(|i| {
            let (a, b) = (
                f64::from(i) * 0.618_033_988_7,
                f64::from(i) * 0.754_877_666_2,
            );
            wgs(a.fract() * 160.0 - 80.0, b.fract() * 359.0 - 179.5)
        })
        .collect();
    points.extend([
        wgs(90.0, 37.0),
        wgs(-90.0, 37.0),
        wgs(10.0, 180.0),
        wgs(-10.0, -180.0),
    ]);
    let count = points.len() as u64;
    let index = ClusterIndex::new(points, tight()).unwrap();
    for zoom in [3, 5, 8, 12] {
        let pins = index.clusters(world(), zoom).unwrap();
        assert_eq!(
            pins.iter().map(MapCluster::count).sum::<u64>(),
            count,
            "zoom {zoom}"
        );
        for (south, west, north, east) in [
            (-5.0, -40.0, 5.0, -20.0),
            (0.0, 150.0, 30.0, -150.0),
            (-30.0, -180.0, 30.0, -170.0),
            (-30.0, 170.0, 30.0, 180.0),
            (5.0, 180.0, 15.0, 180.0),
            (-15.0, -180.0, -5.0, -180.0),
            (-85.0, 100.0, 85.0, 120.0),
            // Every longitude, short of the poles: all the columns, on the grid path.
            (-85.0, -180.0, 85.0, 180.0),
            (-30.0, -120.0, 30.0, -20.0),
            (-30.0, -10.0, 30.0, -100.0),
            (-30.0, 0.001, 30.0, -100.0),
            (80.0, -10.0, 90.0, 10.0),
            (-90.0, -10.0, -80.0, 10.0),
        ] {
            let bounds = GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap();
            let expected: Vec<MapCluster> = pins
                .iter()
                .copied()
                .filter(|pin| bounds.contains(*pin.centroid().value()).unwrap())
                .collect();
            assert_eq!(
                index.clusters(bounds, zoom).unwrap(),
                expected,
                "zoom {zoom} {bounds:?}"
            );
        }
    }
    // The poles are found in a box of other longitudes only by the full walk.
    let cap = GeoBounds::new(80.0, -10.0, 90.0, 10.0, Crs::Wgs84).unwrap();
    assert!(
        index
            .clusters(cap, 12)
            .unwrap()
            .iter()
            .any(|pin| pin.centroid().value().lat() == 90.0)
    );
    let cap = GeoBounds::new(-90.0, -10.0, -80.0, 10.0, Crs::Wgs84).unwrap();
    assert!(
        index
            .clusters(cap, 12)
            .unwrap()
            .iter()
            .any(|pin| pin.centroid().value().lat() == -90.0)
    );
}

#[test]
fn viewports_find_pins_through_the_grid() {
    // Enough pins that a small box walks the grid rather than the whole level.
    let points: Vec<Coordinate> = (0..2_000)
        .map(|i| {
            wgs(
                f64::from(i % 40) * 0.5 - 10.0,
                f64::from(i % 50) * 7.0 - 175.0,
            )
        })
        .collect();
    let index = ClusterIndex::new(points, tight()).unwrap();
    for zoom in [4, 8, 12, 22] {
        for (south, west, north, east) in [
            (-5.0, -40.0, 5.0, -20.0),
            (0.0, 150.0, 9.0, -150.0),
            (-10.0, 175.0, 10.0, 175.0),
            (2.0, -180.0, 3.0, -180.0),
            (2.0, 180.0, 3.0, 180.0),
            (80.0, 0.0, 85.0, 10.0),
        ] {
            let bounds = GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap();
            let expected: Vec<MapCluster> = index
                .clusters(world(), zoom)
                .unwrap()
                .into_iter()
                .filter(|pin| bounds.contains(*pin.centroid().value()).unwrap())
                .collect();
            assert_eq!(index.clusters(bounds, zoom).unwrap(), expected);
        }
    }
    let inner = GeoBounds::new(-5.0, -40.0, 5.0, -20.0, Crs::Wgs84).unwrap();
    assert!(!index.clusters(inner, 12).unwrap().is_empty());
}
