//! `fit_bounds` against `viewport_bounds`, the clusters in view, and the GCJ-02
//! display helpers.

use crate::cluster::{ClusterIndex, ClusterIndexOptions, MAX_ZOOM};
use crate::index::GeoBounds;
use crate::map::{
    Camera, DisplayDatum, MAX_LATITUDE, Viewport, clusters_in_view, display_coordinate, fit_bounds,
    tap_to_wgs84, viewport_bounds,
};
use crate::{Coordinate, Crs, Error, Height};

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn view(width_px: f64, height_px: f64, tile_px: f64) -> Viewport {
    Viewport::new(width_px, height_px, tile_px).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn fit_bounds_inverts_viewport_bounds() {
    // Cameras whose view stays off the map's edges, including views across the
    // antimeridian, centred on each name of it, and on both tile sizes.
    for (lat, lon, zoom) in [
        (48.86, 2.35, 11.5),
        (-17.0, 179.9, 6.0),
        (-17.0, -179.9, 6.25),
        (0.0, 180.0, 3.0),
        (0.0, -180.0, 3.0),
        (35.0, 139.0, 2.5),
        (60.0, -150.0, 18.75),
    ] {
        for viewport in [view(400.0, 300.0, 512.0), view(390.0, 844.0, 256.0)] {
            let camera = Camera::new(wgs(lat, lon), zoom).unwrap();
            let bounds = viewport_bounds(camera, viewport).unwrap();
            assert!(
                bounds.north() < 90.0 && bounds.south() > -90.0,
                "{camera:?}"
            );
            let fitted = fit_bounds(bounds, viewport, 0.0, 22.0).unwrap();
            assert!(close(fitted.zoom(), zoom), "{camera:?}: {fitted:?}");
            assert!(close(fitted.center().lat(), lat), "{camera:?}: {fitted:?}");
            let dlon = (fitted.center().lon() - lon).rem_euclid(360.0);
            assert!(dlon < 1e-9 || dlon > 360.0 - 1e-9, "{camera:?}: {fitted:?}");
            // And the box shown by the fitted camera is the box it was fitted to.
            let again = viewport_bounds(fitted, viewport).unwrap();
            assert!(close(again.west(), bounds.west()) && close(again.east(), bounds.east()));
            assert_eq!(again.crosses_antimeridian(), bounds.crosses_antimeridian());
        }
    }
}

#[test]
fn fitted_camera_shows_the_whole_box() {
    let viewport = view(800.0, 600.0, 512.0);
    for (south, west, north, east) in [
        (-21.0, 176.0, -12.0, -178.0),
        (44.0, 5.0, 48.0, 16.0),
        (-60.0, -170.0, 70.0, 170.0),
        (10.0, 179.0, 11.0, 180.0),
        (-80.0, -180.0, 89.0, 180.0),
    ] {
        let bounds = GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap();
        let camera = fit_bounds(bounds, viewport, 40.0, 20.0).unwrap();
        let shown = viewport_bounds(camera, viewport).unwrap();
        for corner in [
            wgs(south.max(-MAX_LATITUDE), west),
            wgs(north.min(MAX_LATITUDE), east),
            bounds.center(),
        ] {
            assert!(
                shown.contains(corner).unwrap(),
                "{bounds:?} {shown:?} {corner:?}"
            );
        }
        // Across the antimeridian, the camera looks across it.
        if bounds.crosses_antimeridian() {
            assert!(camera.center().lon().abs() > 170.0, "{camera:?}");
        }
    }
}

#[test]
fn fit_bounds_limits() {
    let viewport = view(512.0, 512.0, 512.0);
    // A single point takes max_zoom, on its own position.
    let point = GeoBounds::new(10.0, 20.0, 10.0, 20.0, Crs::Wgs84).unwrap();
    let camera = fit_bounds(point, viewport, 10.0, 17.5).unwrap();
    assert_eq!(camera.zoom(), 17.5);
    assert!(close(camera.center().lat(), 10.0) && close(camera.center().lon(), 20.0));
    // A box tall but not wide is limited by its height.
    let tall = GeoBounds::new(-10.0, 5.0, 10.0, 5.0, Crs::Wgs84).unwrap();
    let height_only = fit_bounds(tall, viewport, 0.0, 22.0).unwrap();
    // 20° of latitude at the equator is 0.0558 of the map's height: 2^4.16 such boxes.
    assert!(
        (height_only.zoom() - 4.1626).abs() < 1e-4,
        "{height_only:?}"
    );
    // The world into a view smaller than one tile clamps to zoom 0.
    let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap();
    let small = fit_bounds(world, view(100.0, 100.0, 512.0), 0.0, 22.0).unwrap();
    assert_eq!(small.zoom(), 0.0);
    // Padding narrows the view: twice the padding of half the view is one zoom out.
    let alps = GeoBounds::new(44.0, 5.0, 48.0, 16.0, Crs::Wgs84).unwrap();
    let bare = fit_bounds(alps, viewport, 0.0, 22.0).unwrap();
    let padded = fit_bounds(alps, viewport, 128.0, 22.0).unwrap();
    assert!(close(bare.zoom() - padded.zoom(), 1.0));
    // The centre keeps the box's CRS.
    let tokyo = GeoBounds::new(35.0, 139.0, 36.0, 140.0, Crs::Tokyo).unwrap();
    assert_eq!(
        fit_bounds(tokyo, viewport, 0.0, 22.0)
            .unwrap()
            .center()
            .crs(),
        Crs::Tokyo
    );
    for (padding, max_zoom, field) in [
        (-1.0, 10.0, "padding_px"),
        (f64::NAN, 10.0, "padding_px"),
        (256.0, 10.0, "padding_px"),
        (0.0, -0.1, "max_zoom"),
        (0.0, 31.0, "max_zoom"),
        (0.0, f64::INFINITY, "max_zoom"),
    ] {
        assert!(
            matches!(
                fit_bounds(alps, viewport, padding, max_zoom),
                Err(Error::InvalidValue { field: f, .. }) if f == field
            ),
            "{padding} {max_zoom}"
        );
    }
    // A wide, short view: only the height runs out.
    assert!(matches!(
        fit_bounds(alps, view(1000.0, 100.0, 512.0), 50.0, 10.0),
        Err(Error::InvalidValue {
            field: "padding_px",
            ..
        })
    ));
}

#[test]
fn viewport_bounds_at_the_edges_of_the_map() {
    // Wider than the world: every longitude.
    let wide = viewport_bounds(
        Camera::new(wgs(0.0, 30.0), 0.0).unwrap(),
        view(600.0, 300.0, 512.0),
    );
    let wide = wide.unwrap();
    assert_eq!((wide.west(), wide.east()), (-180.0, 180.0));
    // Exactly one world wide is every longitude too.
    let exact = viewport_bounds(
        Camera::new(wgs(0.0, 30.0), 0.0).unwrap(),
        view(512.0, 300.0, 512.0),
    );
    let exact = exact.unwrap();
    assert_eq!((exact.west(), exact.east()), (-180.0, 180.0));
    // A view exactly one world high ends on both edges, and so takes in both poles.
    let flush = viewport_bounds(
        Camera::new(wgs(0.0, 0.0), 0.0).unwrap(),
        view(100.0, 512.0, 512.0),
    )
    .unwrap();
    assert_eq!((flush.south(), flush.north()), (-90.0, 90.0));
    // Seeing the top edge takes in the pole; the bottom edge too.
    let arctic = viewport_bounds(
        Camera::new(wgs(80.0, 0.0), 2.0).unwrap(),
        view(400.0, 400.0, 256.0),
    );
    assert_eq!(arctic.unwrap().north(), 90.0);
    let antarctic = viewport_bounds(
        Camera::new(wgs(-80.0, 0.0), 2.0).unwrap(),
        view(400.0, 400.0, 256.0),
    );
    assert_eq!(antarctic.unwrap().south(), -90.0);
    // A view ending on the antimeridian ends at 180, not -180.
    let east_edge = viewport_bounds(
        Camera::new(wgs(0.0, 90.0), 1.0).unwrap(),
        view(512.0, 100.0, 512.0),
    );
    let east_edge = east_edge.unwrap();
    assert_eq!((east_edge.west(), east_edge.east()), (0.0, 180.0));
    assert!(!east_edge.crosses_antimeridian());
    // And one starting there starts at -180.
    let west_edge = viewport_bounds(
        Camera::new(wgs(0.0, -90.0), 1.0).unwrap(),
        view(512.0, 100.0, 512.0),
    );
    assert_eq!(west_edge.unwrap().west(), -180.0);
    // The box keeps the camera's CRS.
    let mars = Camera::new(Coordinate::gcj02(39.9, 116.4).unwrap(), 12.0).unwrap();
    assert_eq!(
        viewport_bounds(mars, view(100.0, 100.0, 256.0))
            .unwrap()
            .crs(),
        Crs::Gcj02
    );
}

#[test]
fn viewport_and_camera_validate() {
    for (w, h, t, field) in [
        (0.0, 1.0, 1.0, "width_px"),
        (f64::INFINITY, 1.0, 1.0, "width_px"),
        (1.0, -1.0, 1.0, "height_px"),
        (1.0, f64::NAN, 1.0, "height_px"),
        (1.0, 1.0, 0.0, "tile_px"),
    ] {
        assert!(matches!(
            Viewport::new(w, h, t),
            Err(Error::InvalidValue { field: f, .. }) if f == field
        ));
    }
    assert!(Camera::new(wgs(0.0, 0.0), 30.0).is_ok());
    assert!(Camera::new(wgs(0.0, 0.0), 30.01).is_err());
    assert!(Camera::new(wgs(0.0, 0.0), -0.01).is_err());
}

#[test]
fn clusters_in_view_floors_and_caps_the_zoom() {
    let points = [
        wgs(0.0, 0.0),
        wgs(0.0, 0.5),
        wgs(0.0, 179.99),
        wgs(0.0, -179.99),
    ];
    let options = ClusterIndexOptions::default().with_tile_px(256.0);
    let index = ClusterIndex::new(points, options).unwrap();
    let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap();
    for zoom in [0.0, 2.9, 5.5, 22.0, 22.99, 30.0] {
        let level = (zoom as u8).min(MAX_ZOOM);
        assert_eq!(
            clusters_in_view(&index, world, zoom).unwrap(),
            index.clusters(world, level).unwrap(),
            "{zoom}"
        );
    }
    // Across the antimeridian, only that pair is in view.
    let camera = Camera::new(wgs(0.0, 180.0), 12.0).unwrap();
    let pacific = viewport_bounds(camera, view(400.0, 300.0, 256.0)).unwrap();
    let pins = clusters_in_view(&index, pacific, camera.zoom()).unwrap();
    assert_eq!(pins.iter().map(|pin| pin.count()).sum::<u64>(), 2);
    for zoom in [-0.5, 30.5, f64::NAN] {
        assert!(matches!(
            clusters_in_view(&index, world, zoom),
            Err(Error::InvalidValue { field: "zoom", .. })
        ));
    }
    let tokyo = GeoBounds::new(-1.0, -1.0, 1.0, 1.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        clusters_in_view(&index, tokyo, 3.0),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn display_shifts_into_gcj02_and_taps_come_back() {
    let tiananmen = wgs(39.9087, 116.3975)
        .with_height(Height::Ellipsoidal(44.0))
        .unwrap();
    let shown = display_coordinate(tiananmen, DisplayDatum::Gcj02).unwrap();
    assert_eq!(shown.crs(), Crs::Gcj02);
    assert_eq!(shown.height(), tiananmen.height());
    // The same exact offset `Wgs84::to_gcj02` applies.
    let direct = crate::china::Wgs84::new(39.9087, 116.3975)
        .unwrap()
        .to_gcj02();
    assert_eq!((shown.lat(), shown.lon()), (direct.lat(), direct.lon()));
    let back = tap_to_wgs84(shown).unwrap();
    assert_eq!(back.value().crs(), Crs::Wgs84);
    assert_eq!(back.max_error().meters(), 0.5);
    assert!((back.value().lat() - 39.9087).abs() < 1e-6);
    assert!((back.value().lon() - 116.3975).abs() < 1e-6);
    // Outside China the offset is nothing; a WGS-84 tap is exact.
    let paris = wgs(48.86, 2.35);
    assert_eq!(
        display_coordinate(paris, DisplayDatum::Gcj02)
            .unwrap()
            .lat(),
        48.86
    );
    let tap = tap_to_wgs84(paris).unwrap();
    assert_eq!((*tap.value(), tap.max_error().meters()), (paris, 0.0));
    let tokyo = Coordinate::new(35.0, 139.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        display_coordinate(tokyo, DisplayDatum::Wgs84),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Tokyo
        })
    ));
    assert!(matches!(
        tap_to_wgs84(tokyo),
        Err(Error::CrsMismatch {
            expected: Crs::Gcj02,
            found: Crs::Tokyo
        })
    ));
}
