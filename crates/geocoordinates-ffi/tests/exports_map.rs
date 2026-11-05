//! Calls every map FFI export at least once with an asserted value, including the
//! failure each one documents.

use geocoordinates_ffi::*;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

fn bad_record() -> Coordinate {
    Coordinate {
        lat: 91.0,
        ..wgs84(0.0, 0.0)
    }
}

fn is_field<T>(result: Result<T, GeoError>, expected: &str) -> bool {
    matches!(result, Err(GeoError::InvalidValue { field, .. }) if field == expected)
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

#[test]
fn constants_cross() {
    assert!((web_mercator_max_latitude_deg() - 85.051_128_78).abs() < 1e-8);
    assert_eq!(tile_max_zoom(), 30);
}

#[test]
fn web_mercator_round_trips_and_validates() {
    // mercantile 1.2.1: xy(-105, 40)
    let denver = web_mercator_from_coordinate(wgs84(40.0, -105.0)).unwrap();
    assert!((denver.x_m - -11_688_546.533_293_726).abs() < 1e-6);
    assert!((denver.y_m - 4_865_942.279_503_176).abs() < 1e-6);
    assert_eq!(denver.crs, Crs::Wgs84);
    let back = web_mercator_to_coordinate(denver).unwrap();
    assert!((back.lat - 40.0).abs() < 1e-12 && (back.lon + 105.0).abs() < 1e-12);
    assert!(web_mercator_from_coordinate(bad_record()).is_err());
    let off_map = WebMercator {
        y_m: 3.0e7,
        ..denver
    };
    assert!(is_field(web_mercator_validate(off_map), "y_m"));
    assert!(is_field(web_mercator_to_coordinate(off_map), "y_m"));
    assert!(web_mercator_validate(denver).is_ok());
}

#[test]
fn world_pixels_round_trip() {
    let pixel = world_pixel_from_coordinate(wgs84(0.0, 0.0), 0.0, 256.0).unwrap();
    assert_eq!(pixel, WorldPixel { x: 128.0, y: 128.0 });
    let back = world_pixel_to_coordinate(WorldPixel { x: 384.0, y: 128.0 }, 0.0, 256.0, Crs::Gcj02)
        .unwrap();
    assert_eq!((back.lat, back.lon, back.crs), (0.0, 0.0, Crs::Gcj02));
    assert!(is_field(
        world_pixel_from_coordinate(wgs84(0.0, 0.0), 31.0, 256.0),
        "zoom"
    ));
    assert!(is_field(
        world_pixel_from_coordinate(wgs84(0.0, 0.0), 1.0, 0.0),
        "tile_px"
    ));
    assert!(world_pixel_from_coordinate(bad_record(), 1.0, 256.0).is_err());
    assert!(is_field(
        world_pixel_to_coordinate(WorldPixel { x: 0.0, y: -1.0 }, 0.0, 256.0, Crs::Wgs84),
        "y"
    ));
    assert!(is_field(
        world_pixel_to_coordinate(
            WorldPixel {
                x: f64::NAN,
                y: 1.0
            },
            0.0,
            256.0,
            Crs::Wgs84
        ),
        "x"
    ));
}

#[test]
fn tiles_and_quadkeys() {
    // mercantile 1.2.1: tile(-105, 40, 14), bounds(486, 332, 10), quadkey(486, 332, 10)
    let denver = tile_from_coordinate(wgs84(40.0, -105.0), 14).unwrap();
    assert_eq!(
        denver,
        Tile {
            z: 14,
            x: 3413,
            y: 6202
        }
    );
    assert!(is_field(tile_from_coordinate(wgs84(40.0, -105.0), 31), "z"));
    assert!(tile_from_coordinate(bad_record(), 3).is_err());

    let tile = Tile {
        z: 10,
        x: 486,
        y: 332,
    };
    assert!(tile_validate(tile).is_ok());
    assert_eq!(tile_quadkey(tile).unwrap(), "0313102310");
    assert_eq!(tile_from_quadkey("0313102310".into()).unwrap(), tile);
    assert_eq!(
        quadkey_canonicalize("0313102310".into()).unwrap(),
        "0313102310"
    );
    let bounds = tile_bounds(tile, Crs::Wgs84).unwrap();
    assert_eq!((bounds.west, bounds.east), (-9.140_625, -8.789_062_5));
    assert!((bounds.north - 53.330_872_983_017_05).abs() < 1e-12);

    let off_grid = Tile { z: 2, x: 4, y: 0 };
    assert!(is_field(tile_validate(off_grid), "x"));
    assert!(is_field(tile_quadkey(off_grid), "x"));
    assert!(is_field(tile_bounds(off_grid, Crs::Wgs84), "x"));
    assert!(matches!(
        tile_from_quadkey("0314".into()),
        Err(GeoError::InvalidGridRef { .. })
    ));
    assert!(matches!(
        quadkey_canonicalize("x".into()),
        Err(GeoError::InvalidGridRef { .. })
    ));
}

#[test]
fn covering_tiles_cross_the_antimeridian() {
    // mercantile 1.2.1: tiles(170, -20, -170, -10, [5]), in this crate's west-to-east order
    let fiji = GeoBounds {
        south: -20.0,
        west: 170.0,
        north: -10.0,
        east: -170.0,
        crs: Crs::Wgs84,
    };
    let tiles = tiles_covering(fiji, 5, 16).unwrap();
    let xy: Vec<(u32, u32)> = tiles.iter().map(|t| (t.x, t.y)).collect();
    assert_eq!(xy, [(31, 16), (31, 17), (0, 16), (0, 17)]);
    assert!(is_field(tiles_covering(fiji, 5, 3), "max_tiles"));
    assert!(is_field(tiles_covering(fiji, 31, 3), "z"));
    let backwards = GeoBounds {
        south: 1.0,
        north: -1.0,
        ..fiji
    };
    assert!(is_field(tiles_covering(backwards, 5, 16), "south"));
}

#[test]
fn cameras_fit_and_show_bounds() {
    let view = Viewport {
        width_px: 512.0,
        height_px: 512.0,
        tile_px: 512.0,
    };
    assert!(viewport_validate(view).is_ok());
    let fiji = GeoBounds {
        south: -21.0,
        west: 176.0,
        north: -12.0,
        east: -161.5,
        crs: Crs::Wgs84,
    };
    let camera = fit_bounds(fiji, view, 0.0, 18.0).unwrap();
    assert!((camera.zoom - 4.0).abs() < 1e-9);
    assert!((camera.center.lon + 172.75).abs() < 1e-9);
    assert!(camera_validate(camera).is_ok());
    let shown = viewport_bounds(camera, view).unwrap();
    assert!((shown.west - 176.0).abs() < 1e-9 && (shown.east + 161.5).abs() < 1e-9);

    assert!(is_field(fit_bounds(fiji, view, 256.0, 18.0), "padding_px"));
    assert!(is_field(fit_bounds(fiji, view, 0.0, 40.0), "max_zoom"));
    let flat = Viewport {
        height_px: 0.0,
        ..view
    };
    assert!(is_field(viewport_validate(flat), "height_px"));
    assert!(is_field(fit_bounds(fiji, flat, 0.0, 18.0), "height_px"));
    assert!(is_field(viewport_bounds(camera, flat), "height_px"));
    let lost = Camera {
        zoom: f64::NAN,
        ..camera
    };
    assert!(is_field(camera_validate(lost), "zoom"));
    assert!(is_field(viewport_bounds(lost, view), "zoom"));
    assert!(
        camera_validate(Camera {
            center: bad_record(),
            zoom: 1.0
        })
        .is_err()
    );
}

#[test]
fn clusters_in_view_read_the_index() {
    let index = ClusterIndex::new(
        vec![wgs84(0.0, 179.99), wgs84(0.0, -179.99), wgs84(40.0, 0.0)],
        cluster_index_options_default(),
    )
    .unwrap();
    let pins = clusters_in_view(&index, world(), 2.7).unwrap();
    assert_eq!(pins, index.clusters(world(), 2).unwrap());
    assert_eq!(pins.len(), 2);
    assert!(is_field(clusters_in_view(&index, world(), -1.0), "zoom"));
    let tokyo = GeoBounds {
        crs: Crs::Tokyo,
        ..world()
    };
    assert!(matches!(
        clusters_in_view(&index, tokyo, 2.0),
        Err(GeoError::CrsMismatch { .. })
    ));

    let layer = clusters_in_view_geojson(&index, world(), 2.7, DisplayDatum::Wgs84).unwrap();
    assert!(layer.contains(r#""cluster":true"#) && layer.contains(r#""point_count":2"#));
    assert!(layer.contains(r#""point_id":2"#));
    assert!(is_field(
        clusters_in_view_geojson(&index, world(), f64::NAN, DisplayDatum::Wgs84),
        "zoom"
    ));
}

#[test]
fn display_datums_and_layers() {
    let beijing = wgs84(39.9087, 116.3975);
    let shown = display_coordinate(beijing, DisplayDatum::Gcj02).unwrap();
    assert_eq!(shown.crs, Crs::Gcj02);
    assert!((shown.lon - beijing.lon).abs() > 0.005);
    assert_eq!(
        display_coordinate(beijing, DisplayDatum::Wgs84).unwrap(),
        beijing
    );
    assert!(matches!(
        display_coordinate(shown, DisplayDatum::Gcj02),
        Err(GeoError::CrsMismatch { .. })
    ));
    let tap = tap_to_wgs84(shown).unwrap();
    assert_eq!((tap.coord.crs, tap.max_error_m), (Crs::Wgs84, 0.5));
    assert!((tap.coord.lat - beijing.lat).abs() < 1e-6);
    assert!(matches!(
        tap_to_wgs84(coordinate_new(39.9, 116.4, Crs::Bd09).unwrap()),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(tap_to_wgs84(bad_record()).is_err());

    let layer = points_to_geojson(vec![wgs84(48.5, 2.25)], DisplayDatum::Wgs84).unwrap();
    assert_eq!(
        layer,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[2.25,48.5]},"id":0,"properties":{"point_id":0}}]}"#
    );
    assert!(matches!(
        points_to_geojson(vec![shown], DisplayDatum::Wgs84),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(points_to_geojson(vec![bad_record()], DisplayDatum::Wgs84).is_err());
}
