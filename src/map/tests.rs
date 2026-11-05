//! Reference values and round trips for the `map` feature.
//!
//! Provenance: every projection, tile, bounds, quadkey and covering-set value
//! below was printed by mercantile 1.2.1 (`mercantile.xy`, `lnglat`, `tile`,
//! `bounds`, `quadkey`, `quadkey_to_tile`, `tiles`), installed with
//! `uv run --no-project --with mercantile==1.2.1`. mercantile takes longitude
//! first; the calls here take latitude first.

mod viewport;

use super::{MAX_LATITUDE, MAX_TILE_ZOOM, Tile, WebMercator, WorldPixel, tiles_covering};
use crate::index::GeoBounds;
use crate::{Coordinate, Crs, Error};

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn tile(x: u32, y: u32, z: u8) -> Tile {
    Tile::new(z, x, y).unwrap()
}

#[test]
fn projects_as_mercantile_does() {
    // (lng, lat) -> mercantile.xy(lng, lat)
    for (lon, lat, x_m, y_m) in [
        (-105.0, 40.0, -11_688_546.533_293_726, 4_865_942.279_503_176),
        (116.4, 39.9, 12_957_588.728_337_044, 4_851_421.175_183_359),
        (179.9, -16.8, 20_026_376.393_709_917, -1_897_556.232_409_338),
        (-180.0, 0.0, -20_037_508.342_789_244, 0.0),
        (0.0, 85.0, 0.0, 19_971_868.880_408_563),
        (2.25, 48.5, 250_468.854_284_865_52, 6_190_443.809_135_445),
    ] {
        let projected = WebMercator::from_coordinate(wgs(lat, lon));
        assert!(
            (projected.x_m() - x_m).abs() < 1e-6,
            "{lon} {lat}: {projected:?}"
        );
        assert!(
            (projected.y_m() - y_m).abs() < 1e-6,
            "{lon} {lat}: {projected:?}"
        );
    }
    // mercantile.lnglat(x, y)
    for (x_m, y_m, lon, lat) in [
        (-11_688_546.533_293_726, 4_865_942.279_503_176, -105.0, 40.0),
        (
            12_957_468.0,
            4_852_834.0,
            116.398_915_478_896_07,
            39.909_735_877_816_75,
        ),
        (0.0, 20_037_508.342_789_244, 0.0, 85.051_128_779_806_6),
    ] {
        let back = WebMercator::new(x_m, y_m, Crs::Wgs84)
            .unwrap()
            .to_coordinate();
        assert!((back.lon() - lon).abs() < 1e-12, "{x_m}: {back:?}");
        assert!((back.lat() - lat).abs() < 1e-12, "{y_m}: {back:?}");
    }
}

#[test]
fn projection_clamps_at_the_poles_and_keeps_the_datum() {
    let north = WebMercator::from_coordinate(wgs(90.0, 10.0));
    let south = WebMercator::from_coordinate(wgs(-90.0, 10.0));
    assert_eq!(north.y_m(), -south.y_m());
    assert!((north.to_coordinate().lat() - MAX_LATITUDE).abs() < 1e-12);
    let east = WebMercator::from_coordinate(wgs(0.0, 180.0)).to_coordinate();
    assert_eq!(east.lon(), 180.0);
    let mars = Coordinate::gcj02(39.9, 116.4).unwrap();
    assert_eq!(
        WebMercator::from_coordinate(mars).to_coordinate().crs(),
        Crs::Gcj02
    );
    for (x, y, field) in [
        (f64::NAN, 0.0, "x_m"),
        (2.1e7, 0.0, "x_m"),
        (0.0, -2.1e7, "y_m"),
        (0.0, f64::INFINITY, "y_m"),
    ] {
        assert!(matches!(
            WebMercator::new(x, y, Crs::Wgs84),
            Err(Error::InvalidValue { field: f, .. }) if f == field
        ));
    }
}

#[test]
fn world_pixels_round_trip_and_wrap() {
    for (lat, lon) in [(40.0, -105.0), (-33.9, 151.2), (0.0, -180.0), (85.0, 179.0)] {
        for (zoom, tile_px) in [(0.0, 256.0), (3.7, 512.0), (17.25, 256.0), (30.0, 512.0)] {
            let pixel = WorldPixel::from_coordinate(wgs(lat, lon), zoom, tile_px).unwrap();
            let back = pixel.to_coordinate(zoom, tile_px, Crs::Wgs84).unwrap();
            assert!((back.lat() - lat).abs() < 1e-9, "{lat} {lon} z{zoom}");
            assert!((back.lon() - lon).abs() < 1e-9, "{lat} {lon} z{zoom}");
        }
    }
    // A pixel a world to the west is the same place.
    let west = WorldPixel { x: -64.0, y: 128.0 };
    assert_eq!(
        west.to_coordinate(0.0, 256.0, Crs::Wgs84).unwrap().lon(),
        90.0
    );
    let bottom = WorldPixel { x: 0.0, y: 256.0 }.to_coordinate(0.0, 256.0, Crs::Wgs84);
    assert!((bottom.unwrap().lat() + MAX_LATITUDE).abs() < 1e-9);
    let pixel = WorldPixel { x: 1.0, y: 1.0 };
    for (result, field) in [
        (pixel.to_coordinate(f64::NAN, 256.0, Crs::Wgs84), "zoom"),
        (pixel.to_coordinate(30.5, 256.0, Crs::Wgs84), "zoom"),
        (pixel.to_coordinate(1.0, 0.0, Crs::Wgs84), "tile_px"),
        (
            pixel.to_coordinate(1.0, f64::INFINITY, Crs::Wgs84),
            "tile_px",
        ),
        (
            WorldPixel {
                x: f64::NAN,
                y: 1.0,
            }
            .to_coordinate(1.0, 256.0, Crs::Wgs84),
            "x",
        ),
        (
            WorldPixel { x: 1.0, y: 512.5 }.to_coordinate(1.0, 256.0, Crs::Wgs84),
            "y",
        ),
        (
            WorldPixel {
                x: 1.0,
                y: f64::NAN,
            }
            .to_coordinate(1.0, 256.0, Crs::Wgs84),
            "y",
        ),
    ] {
        assert!(
            matches!(result, Err(Error::InvalidValue { field: f, .. }) if f == field),
            "{field}"
        );
    }
    assert!(WorldPixel::from_coordinate(wgs(0.0, 0.0), 1.0, -1.0).is_err());
}

#[test]
fn tiles_match_mercantile() {
    // mercantile.tile(lng, lat, z)
    for (lon, lat, z, expected) in [
        (-105.0, 40.0, 14, tile(3413, 6202, 14)),
        (116.4, 39.9, 12, tile(3372, 1552, 12)),
        (179.9, -16.8, 8, tile(255, 140, 8)),
        (-179.9, -16.8, 8, tile(0, 140, 8)),
        (0.0, 0.0, 1, tile(1, 1, 1)),
        (2.25, 48.5, 18, tile(132_710, 90_578, 18)),
        (178.0, -17.7, 4, tile(15, 8, 4)),
    ] {
        assert_eq!(Tile::from_coordinate(wgs(lat, lon), z).unwrap(), expected);
    }
    // mercantile.bounds(x, y, z) as (west, south, east, north), and quadkey.
    for (t, (west, south, east, north), quadkey) in [
        (
            tile(486, 332, 10),
            (
                -9.140_625,
                53.120_405_283_106_564,
                -8.789_062_5,
                53.330_872_983_017_05,
            ),
            "0313102310",
        ),
        (
            tile(0, 0, 0),
            (-180.0, -MAX_LATITUDE, 180.0, MAX_LATITUDE),
            "",
        ),
        (
            tile(3413, 6202, 14),
            (
                -105.007_324_218_75,
                39.993_955_693_973_3,
                -104.985_351_562_5,
                40.010_787_140_465_52,
            ),
            "02310101232121",
        ),
        (tile(1, 1, 1), (0.0, -MAX_LATITUDE, 180.0, 0.0), "3"),
        (
            tile(255, 0, 8),
            (178.593_75, 84.928_320_929_499_63, 180.0, MAX_LATITUDE),
            "11111111",
        ),
        (
            tile(123_456, 654_321, 20),
            (
                -137.614_746_093_75,
                -40.710_312_503_405_89,
                -137.614_402_770_996_1,
                -40.710_052_258_428_966,
            ),
            "20033332203223220002",
        ),
    ] {
        let bounds = t.bounds(Crs::Wgs84);
        for (got, want) in [
            (bounds.west(), west),
            (bounds.south(), south),
            (bounds.east(), east),
            (bounds.north(), north),
        ] {
            assert!((got - want).abs() < 1e-12, "{t:?}: {got} vs {want}");
        }
        assert_eq!(t.quadkey(), quadkey);
        assert_eq!(Tile::from_quadkey(quadkey).unwrap(), t);
    }
    // mercantile.quadkey_to_tile
    for (quadkey, expected) in [
        ("213", tile(3, 5, 3)),
        ("0231010332302013", tile(13_731, 25_065, 16)),
    ] {
        assert_eq!(Tile::from_quadkey(quadkey).unwrap(), expected);
    }
}

#[test]
fn tiles_and_quadkeys_round_trip_and_reject_bad_input() {
    for z in [0, 1, 5, 17, MAX_TILE_ZOOM] {
        let last = ((1_u64 << z) - 1) as u32;
        for (x, y) in [(0, 0), (last, last), (last / 3, last / 2)] {
            let t = tile(x, y, z);
            assert_eq!(Tile::from_quadkey(&t.quadkey()).unwrap(), t);
            // The centre of a tile lies in it.
            let center = t.bounds(Crs::Wgs84).center();
            assert_eq!(Tile::from_coordinate(center, z).unwrap(), t, "{t:?}");
        }
    }
    // The world's east and bottom edges fall in the last column and row.
    assert_eq!(
        Tile::from_coordinate(wgs(-90.0, 180.0), 3).unwrap(),
        tile(7, 7, 3)
    );
    assert_eq!(
        Tile::from_coordinate(wgs(90.0, -180.0), 3).unwrap(),
        tile(0, 0, 3)
    );
    for (z, x, y, field) in [(31, 0, 0, "z"), (2, 4, 0, "x"), (2, 0, 4, "y")] {
        assert!(matches!(
            Tile::new(z, x, y),
            Err(Error::InvalidValue { field: f, .. }) if f == field
        ));
    }
    assert!(Tile::new(2, 3, 3).is_ok());
    for bad in ["4", "01a", "0".repeat(31).as_str(), "-1"] {
        assert!(
            matches!(Tile::from_quadkey(bad), Err(Error::InvalidGridRef(_))),
            "{bad}"
        );
    }
    assert_eq!(
        Tile::from_quadkey(&"3".repeat(30)).unwrap(),
        tile(u32::MAX >> 2, u32::MAX >> 2, 30)
    );
}

fn covering(south: f64, west: f64, north: f64, east: f64, z: u8) -> Vec<(u32, u32)> {
    let bounds = GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap();
    let mut found: Vec<(u32, u32)> = tiles_covering(bounds, z, 1_000)
        .unwrap()
        .iter()
        .inspect(|t| assert_eq!(t.z(), z))
        .map(|t| (t.x(), t.y()))
        .collect();
    let count = found.len();
    found.sort_unstable();
    found.dedup();
    assert_eq!(found.len(), count, "a tile listed twice");
    found
}

fn grid(xs: std::ops::RangeInclusive<u32>, ys: std::ops::RangeInclusive<u32>) -> Vec<(u32, u32)> {
    xs.flat_map(|x| ys.clone().map(move |y| (x, y))).collect()
}

#[test]
fn covering_sets_match_mercantile() {
    // mercantile.tiles(west, south, east, north, [z]), sorted.
    assert_eq!(
        covering(39.99, -105.0, 40.0, -104.99, 14),
        [(3413, 6202), (3413, 6203)]
    );
    assert_eq!(covering(40.0, -10.0, 52.0, 5.0, 6), grid(30..=32, 21..=24));
    assert_eq!(
        covering(-20.0, 170.0, -10.0, -170.0, 5),
        [(0, 16), (0, 17), (31, 16), (31, 17)]
    );
    assert_eq!(covering(-85.0, -180.0, 85.0, 180.0, 2), grid(0..=3, 0..=3));
    assert_eq!(
        covering(10.0, 100.0, 30.0, 120.0, 7),
        grid(99..=106, 52..=60)
    );
}

#[test]
fn covering_handles_edges_poles_and_its_bound() {
    // An east or south edge on a tile edge takes nothing beyond it.
    assert_eq!(covering(0.0, 0.0, 10.0, 90.0, 2), [(2, 1)]);
    // A point on a tile corner is one tile.
    assert_eq!(covering(0.0, 0.0, 0.0, 0.0, 2), [(2, 2)]);
    // Past the map's edge is the edge row; the whole world is every tile once.
    assert_eq!(covering(-90.0, -180.0, 90.0, 180.0, 1), grid(0..=1, 0..=1));
    // Almost all the way round, across the antimeridian: still each column once.
    assert_eq!(covering(-1.0, 10.0, 1.0, 9.9, 1), grid(0..=1, 0..=1));
    // Across the antimeridian, west to east: the last column first.
    let bounds = GeoBounds::new(-1.0, 179.0, 1.0, -179.0, Crs::Wgs84).unwrap();
    let order: Vec<u32> = tiles_covering(bounds, 3, 4)
        .unwrap()
        .iter()
        .map(Tile::x)
        .collect();
    assert_eq!(order, [7, 7, 0, 0]);
    // The 180 meridian alone, written either way.
    assert_eq!(covering(-1.0, 180.0, 1.0, 180.0, 1), [(0, 0), (0, 1)]);
    // The bound is checked before anything is built.
    let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap();
    assert_eq!(tiles_covering(world, 3, 64).unwrap().len(), 64);
    assert!(matches!(
        tiles_covering(world, 3, 63),
        Err(Error::InvalidValue {
            field: "max_tiles",
            ..
        })
    ));
    assert!(matches!(
        tiles_covering(world, MAX_TILE_ZOOM, 1 << 20),
        Err(Error::InvalidValue {
            field: "max_tiles",
            ..
        })
    ));
    assert!(matches!(
        tiles_covering(world, 31, 1),
        Err(Error::InvalidValue { field: "z", .. })
    ));
}
