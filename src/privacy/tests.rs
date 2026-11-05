//! Precision construction and the size mapping, and coarsening: determinism,
//! idempotence, the stated bound, the poles and the antimeridian.

use proptest::prelude::*;

use super::{Precision, SnapGrid, coarsen, coarsen_fix, geohash_min_height_m};
use crate::fix::{Accuracy, AccuracyKind, Fix};
use crate::geodesy::geodesic_distance;
use crate::grids::Geohash;
use crate::{Coordinate, Crs, Error, Height, Length};

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn bound_m(fix: &Fix) -> f64 {
    let accuracy = fix.accuracy.unwrap();
    assert_eq!(accuracy.kind(), AccuracyKind::Bound);
    assert_eq!(accuracy.vertical(), None);
    accuracy.horizontal().unwrap().meters()
}

/// Every precision the enabled grids define.
fn every_precision() -> Vec<Precision> {
    let geohash = (1..=12).map(|n| Precision::geohash(n).unwrap());
    #[cfg(feature = "h3")]
    let geohash = geohash.chain((0..=15).map(|n| Precision::h3(n).unwrap()));
    #[cfg(feature = "s2")]
    let geohash = geohash.chain((0..=30).map(|n| Precision::s2(n).unwrap()));
    geohash.collect()
}

#[test]
fn the_size_table_in_the_docs_matches_the_mapping() {
    let table = [
        (1, 4_975_830.0),
        (2, 621_978.7),
        (3, 155_494.6),
        (4, 19_436.8),
        (5, 4_859.2),
        (6, 607.4),
        (7, 151.8),
        (8, 18.98),
        (9, 4.745),
        (10, 0.593),
        (11, 0.148),
        (12, 0.0185),
    ];
    for (length, stated) in table {
        let height = geohash_min_height_m(length);
        // Each stated value is the true one rounded down to the digits shown.
        assert!(
            height >= stated && height < stated * 1.005,
            "{length}: {height}"
        );
    }
}

#[test]
fn a_geohash_cell_height_is_never_under_its_stated_minimum() {
    // Latitude spans are the same everywhere, and the meridian degree is
    // shortest at the equator: check the span there and near a pole.
    for length in 1..=12 {
        let minimum = geohash_min_height_m(length);
        for lat in [0.0, 1.0e-9, 45.0, 89.9] {
            let cell = Geohash::encode(wgs84(lat, 10.0), length).unwrap().decode();
            let center = cell.value();
            let span_deg = 180.0 / 2_f64.powi(5 * i32::from(length) / 2);
            let south = (center.lat() - span_deg / 2.0).max(-90.0);
            let north = (south + span_deg).min(90.0);
            let height = geodesic_distance(wgs84(south, 10.0), wgs84(north, 10.0))
                .unwrap()
                .meters();
            assert!(
                height >= minimum,
                "length {length} at {lat}: {height} < {minimum}"
            );
        }
    }
}

#[test]
fn at_least_picks_the_finest_length_tall_enough() {
    let length = |m: f64| Precision::at_least(Length::from_meters(m)).unwrap().level();
    assert_eq!(length(0.0), 12);
    assert_eq!(length(0.0185), 12);
    assert_eq!(length(0.02), 11);
    assert_eq!(length(607.4), 6);
    assert_eq!(length(607.5), 5);
    assert_eq!(length(1_000.0), 5);
    assert_eq!(length(4_975_830.0), 1);
    for meters in [0.0, 0.5, 30.0, 1_000.0, 20_000.0, 1.0e6] {
        let precision = Precision::at_least(Length::from_meters(meters)).unwrap();
        assert_eq!(precision.grid(), SnapGrid::Geohash);
        assert!(precision.min_cell_height().unwrap().meters() >= meters);
        if precision.level() < 12 {
            let finer = Precision::geohash(precision.level() + 1).unwrap();
            assert!(finer.min_cell_height().unwrap().meters() < meters);
        }
    }
}

#[test]
fn at_least_rejects_sizes_it_cannot_honour() {
    for meters in [-1.0, f64::NAN, f64::INFINITY, 5.0e6] {
        assert!(
            matches!(
                Precision::at_least(Length::from_meters(meters)),
                Err(Error::InvalidValue { field: "size", .. })
            ),
            "{meters}"
        );
    }
}

#[test]
fn levels_outside_a_grid_are_rejected() {
    assert!(matches!(
        Precision::geohash(0),
        Err(Error::InvalidValue {
            field: "geohash length",
            ..
        })
    ));
    assert!(Precision::geohash(13).is_err());
    assert!(Precision::geohash(1).is_ok() && Precision::geohash(12).is_ok());
    #[cfg(feature = "h3")]
    {
        assert!(Precision::h3(0).is_ok() && Precision::h3(15).is_ok());
        assert!(matches!(
            Precision::new(SnapGrid::H3, 16),
            Err(Error::InvalidValue {
                field: "H3 resolution",
                ..
            })
        ));
        assert_eq!(Precision::h3(9).unwrap().min_cell_height(), None);
    }
    #[cfg(feature = "s2")]
    {
        assert!(Precision::s2(0).is_ok() && Precision::s2(30).is_ok());
        assert!(matches!(
            Precision::new(SnapGrid::S2, 31),
            Err(Error::InvalidValue {
                field: "S2 level",
                ..
            })
        ));
        assert_eq!(Precision::s2(9).unwrap().min_cell_height(), None);
    }
}

#[test]
fn coarsening_names_the_cell_and_drops_everything_else() {
    let point = wgs84(57.64911, 10.40744)
        .with_height(Height::Ellipsoidal(42.0))
        .unwrap();
    let fix = coarsen(point, Precision::geohash(5).unwrap()).unwrap();
    let source = fix.source.as_ref().unwrap();
    assert_eq!(source.raw, "privacy:geohash:u4pru");
    assert_eq!(source.confidence.value(), 1.0);
    assert_eq!(fix.coord.height(), None);
    assert_eq!(fix.timestamp, None);
    let cell = Geohash::parse("u4pru").unwrap().decode();
    assert_eq!(fix.coord, *cell.value());
    assert_eq!(bound_m(&fix), cell.max_error().meters());
}

#[cfg(feature = "h3")]
#[test]
fn h3_cells_are_named_by_their_hex_index() {
    let fix = coarsen(wgs84(40.7128, -74.006), Precision::h3(9).unwrap()).unwrap();
    assert_eq!(fix.source.unwrap().raw, "privacy:h3:892a1072893ffff");
}

#[cfg(feature = "s2")]
#[test]
fn s2_cells_are_named_by_their_hex_id() {
    let fix = coarsen(wgs84(40.7128, -74.006), Precision::s2(20).unwrap()).unwrap();
    assert_eq!(
        fix.source.unwrap().raw,
        format!("privacy:s2:{:016x}", 9_926_595_630_970_437_632_u64)
    );
}

#[test]
fn coarsening_off_wgs84_is_refused() {
    for crs in [Crs::Gcj02, Crs::Bd09, Crs::Nad27] {
        let coord = Coordinate::new(39.9, 116.4, crs).unwrap();
        for precision in every_precision() {
            assert!(matches!(
                coarsen(coord, precision),
                Err(Error::CrsMismatch {
                    expected: Crs::Wgs84,
                    ..
                })
            ));
        }
    }
}

#[test]
fn the_poles_and_the_antimeridian_snap_within_the_bound() {
    let edges = [
        (90.0, 0.0),
        (90.0, 123.0),
        (-90.0, -45.0),
        (89.999_999, 179.999_999),
        (0.0, 180.0),
        (0.0, -180.0),
        (-45.0, 179.999_999_9),
        (45.0, -179.999_999_9),
    ];
    for precision in every_precision() {
        for (lat, lon) in edges {
            let point = wgs84(lat, lon);
            let fix = coarsen(point, precision).unwrap();
            let distance = geodesic_distance(point, fix.coord).unwrap().meters();
            assert!(
                distance <= bound_m(&fix),
                "{precision:?} at ({lat}, {lon}): {distance} > {}",
                bound_m(&fix)
            );
            assert_eq!(coarsen(fix.coord, precision).unwrap(), fix, "{precision:?}");
        }
    }
    // 180 and -180 name one meridian, and a pole is one point whatever its
    // longitude, so each place lands in one cell however it is written.
    for precision in every_precision() {
        assert_eq!(
            coarsen(wgs84(10.0, 180.0), precision).unwrap(),
            coarsen(wgs84(10.0, -180.0), precision).unwrap()
        );
        for pole in [90.0, -90.0] {
            let reference = coarsen(wgs84(pole, 0.0), precision).unwrap();
            for lon in [-180.0, -77.0, 45.0, 180.0] {
                assert_eq!(coarsen(wgs84(pole, lon), precision).unwrap(), reference);
            }
        }
    }
}

#[test]
fn a_fix_keeps_its_time_and_widens_its_bound_by_its_own() {
    let precision = Precision::geohash(6).unwrap();
    let point = wgs84(48.8584, 2.2945);
    let cell = bound_m(&coarsen(point, precision).unwrap());
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    let with = |kind: AccuracyKind, horizontal: Option<f64>, vertical: Option<f64>| {
        let mut fix = Fix::from_coord(point);
        fix.timestamp = Some(time);
        fix.accuracy = Some(
            Accuracy::new(
                kind,
                horizontal.map(Length::from_meters),
                vertical.map(Length::from_meters),
            )
            .unwrap(),
        );
        coarsen_fix(fix, precision).unwrap()
    };

    let bound = with(AccuracyKind::Bound, Some(10.0), Some(3.0));
    assert_eq!(bound.timestamp, Some(time));
    assert_eq!(bound_m(&bound), cell + 10.0);

    let sigma = with(AccuracyKind::OneSigma, Some(4.0), None)
        .accuracy
        .unwrap();
    assert_eq!(sigma.kind(), AccuracyKind::Unspecified);
    assert_eq!(sigma.horizontal().unwrap().meters(), cell + 4.0);

    // A vertical-only accuracy says nothing about the horizontal error.
    assert_eq!(
        bound_m(&with(AccuracyKind::OneSigma, None, Some(4.0))),
        cell
    );
    assert_eq!(
        bound_m(&coarsen_fix(Fix::from_coord(point), precision).unwrap()),
        cell
    );
}

fn any_wgs84() -> impl Strategy<Value = Coordinate> {
    (-90.0..=90.0_f64, -180.0..=180.0_f64).prop_map(|(lat, lon)| wgs84(lat, lon))
}

fn any_precision() -> impl Strategy<Value = Precision> {
    prop::sample::select(every_precision())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn coarsening_twice_equals_coarsening_once(point in any_wgs84(), precision in any_precision()) {
        let once = coarsen(point, precision).unwrap();
        prop_assert_eq!(coarsen(once.coord, precision).unwrap(), once);
    }

    #[test]
    fn the_input_never_lies_beyond_the_stated_bound(
        point in any_wgs84(),
        precision in any_precision(),
    ) {
        let fix = coarsen(point, precision).unwrap();
        let distance = geodesic_distance(point, fix.coord).unwrap().meters();
        prop_assert!(distance <= bound_m(&fix), "{distance} > {}", bound_m(&fix));
    }

    #[test]
    fn every_point_of_a_cell_coarsens_to_the_same_fix(
        point in any_wgs84(),
        length in 1..=12_u8,
        (north, east) in (0.0..1.0_f64, 0.0..1.0_f64),
    ) {
        // A second point drawn inside the first point's geohash cell.
        let precision = Precision::geohash(length).unwrap();
        let cell = Geohash::encode(point, length).unwrap().decode();
        let lon_bits = (5 * i32::from(length) + 1) / 2;
        let lat_bits = 5 * i32::from(length) / 2;
        let half_height = 90.0 / 2_f64.powi(lat_bits);
        let half_width = 180.0 / 2_f64.powi(lon_bits);
        let center = cell.value();
        let other = wgs84(
            center.lat() + (2.0 * north - 1.0) * half_height * 0.999,
            center.lon() + (2.0 * east - 1.0) * half_width * 0.999,
        );
        prop_assert_eq!(coarsen(other, precision).unwrap(), coarsen(point, precision).unwrap());
    }
}
