use proptest::prelude::*;

use super::*;
use crate::LengthUnit;
use crate::geodesy::{destination, geodesic_distance};

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn bounds(south: f64, west: f64, north: f64, east: f64) -> GeoBounds {
    GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap()
}

fn edges(b: GeoBounds) -> (f64, f64, f64, f64) {
    (b.south(), b.west(), b.north(), b.east())
}

fn km(value: f64) -> Length {
    Length::from_unit(value, LengthUnit::Kilometer)
}

fn from_lons(lons: &[f64]) -> (f64, f64) {
    let b = GeoBounds::from_points(lons.iter().map(|&lon| wgs84(0.0, lon))).unwrap();
    (b.west(), b.east())
}

#[test]
fn new_rejects_each_edge_by_name() {
    for (args, field) in [
        ((-90.5, 0.0, 0.0, 0.0), "south"),
        ((0.0, -180.5, 0.0, 0.0), "west"),
        ((0.0, 0.0, 90.5, 0.0), "north"),
        ((0.0, 0.0, 0.0, 180.5), "east"),
        ((f64::NAN, 0.0, 0.0, 0.0), "south"),
        ((0.0, f64::INFINITY, 0.0, 0.0), "west"),
        ((1.0, 0.0, 0.0, 0.0), "south"),
    ] {
        let found = GeoBounds::new(args.0, args.1, args.2, args.3, Crs::Wgs84);
        assert!(
            matches!(found, Err(Error::InvalidValue { field: f, .. }) if f == field),
            "{args:?}: {found:?}"
        );
    }
}

#[test]
fn new_accepts_the_extreme_edges() {
    assert_eq!(
        edges(bounds(-90.0, -180.0, 90.0, 180.0)),
        (-90.0, -180.0, 90.0, 180.0)
    );
}

#[test]
fn getters_read_back_and_crossing_is_west_past_east() {
    let b = GeoBounds::new(1.0, 2.0, 3.0, 4.0, Crs::Nad27).unwrap();
    assert_eq!(edges(b), (1.0, 2.0, 3.0, 4.0));
    assert_eq!(b.crs(), Crs::Nad27);
    assert!(!b.crosses_antimeridian());
    assert!(bounds(0.0, 4.0, 1.0, 2.0).crosses_antimeridian());
    assert!(!bounds(0.0, 4.0, 1.0, 4.0).crosses_antimeridian());
}

#[test]
fn from_points_rejects_empty_and_mixed() {
    assert!(matches!(
        GeoBounds::from_points([]),
        Err(Error::InvalidValue {
            field: "points",
            ..
        })
    ));
    let mixed = [wgs84(0.0, 0.0), Coordinate::gcj02(0.0, 0.0).unwrap()];
    assert!(matches!(
        GeoBounds::from_points(mixed),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
}

#[test]
fn from_points_takes_the_smallest_longitude_span() {
    assert_eq!(from_lons(&[10.0]), (10.0, 10.0));
    assert_eq!(from_lons(&[10.0, 20.0, 15.0]), (10.0, 20.0));
    // Around the antimeridian: 4° wide, not 356°.
    assert_eq!(from_lons(&[178.0, -178.0]), (178.0, -178.0));
    assert_eq!(from_lons(&[-178.0, 179.0, 178.0, -179.0]), (178.0, -178.0));
    // The widest gap wins wherever it falls.
    assert_eq!(from_lons(&[-100.0, 0.0, 100.0]), (-100.0, 100.0));
    assert_eq!(from_lons(&[0.0, 100.0, 170.0]), (0.0, 170.0));
    assert_eq!(from_lons(&[-170.0, 150.0, 170.0]), (150.0, -170.0));
}

#[test]
fn from_points_reads_the_antimeridian_as_one_meridian() {
    assert_eq!(from_lons(&[180.0]), (-180.0, -180.0));
    assert_eq!(from_lons(&[-180.0]), (-180.0, -180.0));
    assert_eq!(from_lons(&[180.0, -180.0]), (-180.0, -180.0));
    assert_eq!(from_lons(&[179.0, 180.0]), (179.0, 180.0));
    assert_eq!(from_lons(&[179.0, -180.0]), (179.0, 180.0));
    assert_eq!(from_lons(&[-179.0, 180.0]), (-180.0, -179.0));
}

#[test]
fn from_points_breaks_equal_gaps_at_the_lowest_west_edge() {
    // The wrap-around gap ties an inner one, and keeps the lowest west edge.
    assert_eq!(from_lons(&[0.0, 180.0]), (-180.0, 0.0));
    assert_eq!(from_lons(&[-90.0, 90.0]), (-90.0, 90.0));
    // Two inner gaps tie, and the first leaves the lower west edge.
    assert_eq!(from_lons(&[-170.0, 0.0, 170.0]), (0.0, -170.0));
}

#[test]
fn from_points_takes_latitudes_and_ignores_pole_longitudes() {
    let b = GeoBounds::from_points([wgs84(80.0, 10.0), wgs84(90.0, -100.0), wgs84(70.0, 20.0)])
        .unwrap();
    assert_eq!(edges(b), (70.0, 10.0, 90.0, 20.0));
    let b = GeoBounds::from_points([wgs84(-90.0, 50.0), wgs84(-80.0, 10.0)]).unwrap();
    assert_eq!(edges(b), (-90.0, 10.0, -80.0, 10.0));
    let b = GeoBounds::from_points([wgs84(90.0, 50.0), wgs84(-90.0, 60.0)]).unwrap();
    assert_eq!(edges(b), (-90.0, 0.0, 90.0, 0.0));
}

#[test]
fn contains_is_inclusive_and_crosses_the_antimeridian() {
    let plain = bounds(-1.0, 10.0, 1.0, 20.0);
    for (lat, lon, inside) in [
        (0.0, 15.0, true),
        (-1.0, 10.0, true),
        (1.0, 20.0, true),
        (1.5, 15.0, false),
        (-1.5, 15.0, false),
        (0.0, 9.0, false),
        (0.0, 21.0, false),
    ] {
        assert_eq!(plain.contains(wgs84(lat, lon)).unwrap(), inside);
    }
    let crossing = bounds(-1.0, 170.0, 1.0, -170.0);
    for (lon, inside) in [
        (170.0, true),
        (175.0, true),
        (180.0, true),
        (-180.0, true),
        (-175.0, true),
        (-170.0, true),
        (169.0, false),
        (-169.0, false),
        (0.0, false),
    ] {
        assert_eq!(crossing.contains(wgs84(0.0, lon)).unwrap(), inside, "{lon}");
    }
}

#[test]
fn contains_reads_the_antimeridian_and_the_poles_as_one_place() {
    assert!(
        bounds(0.0, 170.0, 1.0, 180.0)
            .contains(wgs84(0.5, -180.0))
            .unwrap()
    );
    assert!(
        bounds(0.0, -180.0, 1.0, -170.0)
            .contains(wgs84(0.5, 180.0))
            .unwrap()
    );
    assert!(
        !bounds(0.0, 0.0, 1.0, 10.0)
            .contains(wgs84(0.5, 180.0))
            .unwrap()
    );
    let cap = bounds(80.0, 0.0, 90.0, 10.0);
    assert!(cap.contains(wgs84(90.0, -120.0)).unwrap());
    assert!(!cap.contains(wgs84(-90.0, 5.0)).unwrap());
    let south_cap = bounds(-90.0, 0.0, -80.0, 10.0);
    assert!(south_cap.contains(wgs84(-90.0, -120.0)).unwrap());
    assert!(!south_cap.contains(wgs84(-85.0, -120.0)).unwrap());
    assert!(matches!(
        cap.contains(Coordinate::gcj02(85.0, 5.0).unwrap()),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
}

#[test]
fn intersects_handles_latitude_antimeridian_and_poles() {
    let a = bounds(0.0, 10.0, 10.0, 20.0);
    assert!(a.intersects(&a).unwrap());
    assert!(a.intersects(&bounds(10.0, 20.0, 15.0, 30.0)).unwrap()); // a shared corner
    assert!(!a.intersects(&bounds(11.0, 10.0, 15.0, 20.0)).unwrap());
    assert!(!a.intersects(&bounds(-5.0, 10.0, -1.0, 20.0)).unwrap());
    assert!(!a.intersects(&bounds(0.0, 21.0, 10.0, 30.0)).unwrap());
    assert!(a.intersects(&bounds(0.0, 12.0, 10.0, 15.0)).unwrap()); // inside
    assert!(bounds(0.0, 12.0, 10.0, 15.0).intersects(&a).unwrap());

    let crossing = bounds(-1.0, 170.0, 1.0, -170.0);
    assert!(
        crossing
            .intersects(&bounds(0.0, 175.0, 1.0, 180.0))
            .unwrap()
    );
    assert!(
        crossing
            .intersects(&bounds(0.0, -180.0, 1.0, -175.0))
            .unwrap()
    );
    assert!(
        !crossing
            .intersects(&bounds(0.0, -160.0, 1.0, 160.0))
            .unwrap()
    );

    // Two caps on one pole share it whatever their longitudes; on different
    // longitudes away from a pole they share nothing.
    let cap = bounds(80.0, 0.0, 90.0, 10.0);
    assert!(cap.intersects(&bounds(85.0, 100.0, 90.0, 110.0)).unwrap());
    assert!(!cap.intersects(&bounds(85.0, 100.0, 89.0, 110.0)).unwrap());
    let south = bounds(-90.0, 0.0, -80.0, 10.0);
    assert!(
        south
            .intersects(&bounds(-90.0, 100.0, -85.0, 110.0))
            .unwrap()
    );
    assert!(
        !south
            .intersects(&bounds(-89.0, 100.0, -85.0, 110.0))
            .unwrap()
    );
    assert!(!cap.intersects(&south).unwrap());
    let other = GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Gcj02).unwrap();
    assert!(matches!(
        a.intersects(&other),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn union_takes_the_short_way_around() {
    let union = |a: GeoBounds, b: GeoBounds| edges(a.union(&b).unwrap());
    let a = bounds(0.0, 10.0, 1.0, 20.0);
    // Contained, overlapping, and disjoint.
    assert_eq!(
        union(a, bounds(0.0, 12.0, 1.0, 15.0)),
        (0.0, 10.0, 1.0, 20.0)
    );
    assert_eq!(
        union(bounds(0.0, 12.0, 1.0, 15.0), a),
        (0.0, 10.0, 1.0, 20.0)
    );
    assert_eq!(
        union(a, bounds(5.0, 15.0, 6.0, 30.0)),
        (0.0, 10.0, 6.0, 30.0)
    );
    assert_eq!(
        union(bounds(-5.0, 15.0, -4.0, 30.0), a),
        (-5.0, 10.0, 1.0, 30.0)
    );
    assert_eq!(
        union(a, bounds(0.0, 40.0, 1.0, 50.0)),
        (0.0, 10.0, 1.0, 50.0)
    );
    // Disjoint across the antimeridian.
    assert_eq!(
        union(
            bounds(0.0, 170.0, 1.0, 175.0),
            bounds(0.0, -175.0, 1.0, -170.0)
        ),
        (0.0, 170.0, 1.0, -170.0)
    );
    assert_eq!(
        union(
            bounds(0.0, -175.0, 1.0, -170.0),
            bounds(0.0, 170.0, 1.0, 175.0)
        ),
        (0.0, 170.0, 1.0, -170.0)
    );
    // An east edge landing on the antimeridian reads 180.
    assert_eq!(
        union(
            bounds(0.0, 170.0, 1.0, 175.0),
            bounds(0.0, -180.0, 1.0, -180.0)
        ),
        (0.0, 170.0, 1.0, 180.0)
    );
}

#[test]
fn union_circles_the_globe_when_nothing_less_holds_both() {
    let a = bounds(0.0, -170.0, 1.0, 100.0);
    let b = bounds(0.0, 90.0, 1.0, -160.0);
    assert_eq!(edges(a.union(&b).unwrap()), (0.0, -180.0, 1.0, 180.0));
    let full = bounds(0.0, -180.0, 1.0, 180.0);
    assert_eq!(edges(full.union(&a).unwrap()), (0.0, -180.0, 1.0, 180.0));
    assert_eq!(edges(a.union(&full).unwrap()), (0.0, -180.0, 1.0, 180.0));
}

#[test]
fn union_of_one_arc_spelled_two_ways_is_commutative() {
    let (a, b) = (
        bounds(0.0, -180.0, 0.0, -180.0),
        bounds(0.0, 180.0, 0.0, 180.0),
    );
    assert_eq!(a.union(&b).unwrap(), a);
    assert_eq!(b.union(&a).unwrap(), a);
}

#[test]
fn union_is_commutative_when_both_ways_tie() {
    let a = bounds(0.0, 0.0, 1.0, 10.0);
    let b = bounds(0.0, 180.0, 1.0, -170.0);
    let one = a.union(&b).unwrap();
    assert_eq!(one, b.union(&a).unwrap());
    assert_eq!(edges(one), (0.0, 0.0, 1.0, -170.0));
}

#[test]
fn union_of_a_pole_box_keeps_only_real_longitudes() {
    let pole = bounds(90.0, 123.0, 90.0, 123.0);
    let south_pole = bounds(-90.0, 45.0, -90.0, 45.0);
    let a = bounds(80.0, 10.0, 85.0, 20.0);
    assert_eq!(edges(pole.union(&a).unwrap()), (80.0, 10.0, 90.0, 20.0));
    assert_eq!(edges(a.union(&pole).unwrap()), (80.0, 10.0, 90.0, 20.0));
    assert_eq!(
        edges(pole.union(&south_pole).unwrap()),
        (-90.0, 0.0, 90.0, 0.0)
    );
    let other = GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Gcj02).unwrap();
    assert!(matches!(a.union(&other), Err(Error::CrsMismatch { .. })));
}

#[test]
fn center_is_the_middle_of_the_edges() {
    let center = |b: GeoBounds| {
        let c = b.center();
        (c.lat(), c.lon())
    };
    assert_eq!(center(bounds(0.0, 10.0, 10.0, 30.0)), (5.0, 20.0));
    assert_eq!(center(bounds(-10.0, 170.0, 20.0, -160.0)), (5.0, -175.0));
    assert_eq!(center(bounds(-90.0, -180.0, 90.0, 180.0)), (0.0, 0.0));
    // The middle of an arc ending on the antimeridian, and one starting there.
    assert_eq!(center(bounds(0.0, 170.0, 0.0, 180.0)), (0.0, 175.0));
    assert_eq!(center(bounds(0.0, 180.0, 0.0, -170.0)), (0.0, -175.0));
    assert_eq!(bounds(0.0, 0.0, 1.0, 1.0).center().crs(), Crs::Wgs84);
    assert_eq!(
        GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Tokyo)
            .unwrap()
            .center()
            .crs(),
        Crs::Tokyo
    );
}

#[test]
fn expand_by_validates_its_inputs() {
    let b = bounds(0.0, 0.0, 1.0, 1.0);
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            b.expand_by(Length::from_meters(bad)),
            Err(Error::InvalidValue {
                field: "distance",
                ..
            })
        ));
    }
    let gcj = GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Gcj02).unwrap();
    assert!(matches!(
        gcj.expand_by(Length::from_meters(0.0)),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
}

#[test]
fn expand_by_zero_is_the_box_itself() {
    let b = bounds(89.0, 170.0, 90.0, -170.0);
    assert_eq!(b.expand_by(Length::from_meters(0.0)).unwrap(), b);
}

#[test]
fn expand_by_moves_each_edge_by_the_geodesic_distance() {
    let b = bounds(10.0, 20.0, 11.0, 21.0);
    let d = km(100.0);
    let grown = b.expand_by(d).unwrap();
    let north = geodesic_distance(wgs84(11.0, 21.0), wgs84(grown.north(), 21.0)).unwrap();
    let south = geodesic_distance(wgs84(10.0, 21.0), wgs84(grown.south(), 21.0)).unwrap();
    assert!((north.meters() - 100_000.0).abs() < 0.001, "{north:?}");
    assert!((south.meters() - 100_000.0).abs() < 0.001, "{south:?}");
    let reach = grown.east() - 21.0;
    assert!((reach - (20.0 - grown.west())).abs() < 1e-9);
    // About 100 km over a degree of 109 km at that latitude.
    assert!((reach - 0.9).abs() < 0.1, "{reach}");
}

#[test]
fn expand_by_measures_from_the_edge_nearest_a_pole() {
    for b in [bounds(-60.0, 0.0, 10.0, 1.0), bounds(-10.0, 0.0, 60.0, 1.0)] {
        let grown = b.expand_by(km(100.0)).unwrap();
        // 100 km at 60° is about 1.8° of longitude; at 10° it would be 0.9°.
        assert!((grown.east() - 1.0 - 1.8).abs() < 0.05, "{grown:?}");
    }
}

#[test]
fn expand_by_wraps_across_the_antimeridian_both_ways() {
    let east = bounds(0.0, 179.5, 1.0, 180.0).expand_by(km(100.0)).unwrap();
    assert!(east.crosses_antimeridian());
    assert!(east.east() > -180.0 && east.east() < -178.0, "{east:?}");
    let west = bounds(0.0, -180.0, 1.0, -179.5)
        .expand_by(km(100.0))
        .unwrap();
    assert!(west.crosses_antimeridian());
    assert!(west.west() > 178.0 && west.west() < 180.0, "{west:?}");
}

#[test]
fn expand_by_clamps_at_each_pole_and_takes_every_longitude() {
    let north = bounds(89.5, 0.0, 89.9, 1.0).expand_by(km(50.0)).unwrap();
    assert_eq!(
        (north.north(), north.west(), north.east()),
        (90.0, -180.0, 180.0)
    );
    assert!(north.south() < 89.5 && north.south() > 89.0);
    let south = bounds(-89.9, 0.0, -89.5, 1.0).expand_by(km(50.0)).unwrap();
    assert_eq!(
        (south.south(), south.west(), south.east()),
        (-90.0, -180.0, 180.0)
    );
    assert!(south.north() > -89.5 && south.north() < -89.0);
    // A box already on the pole, and the whole globe.
    let cap = bounds(89.0, 0.0, 90.0, 1.0).expand_by(km(1.0)).unwrap();
    assert_eq!((cap.north(), cap.west(), cap.east()), (90.0, -180.0, 180.0));
    let globe = bounds(0.0, 0.0, 1.0, 1.0).expand_by(km(30_000.0)).unwrap();
    assert_eq!(edges(globe), (-90.0, -180.0, 90.0, 180.0));
}

#[test]
fn expand_by_a_near_full_span_circles_the_globe() {
    let wide = bounds(0.0, -179.0, 1.0, 179.0);
    assert_eq!(
        edges(wide.expand_by(km(200.0)).unwrap()).1,
        -180.0,
        "the two edges pass each other"
    );
    let grown = wide.expand_by(km(200.0)).unwrap();
    assert_eq!(grown.east(), 180.0);
    // Just short of that, the gap stays open.
    let narrow = wide.expand_by(km(10.0)).unwrap();
    assert!(narrow.west() < -179.0 && narrow.west() > -180.0);
    assert!(narrow.east() > 179.0 && narrow.east() < 180.0);
}

#[test]
fn expand_by_uses_the_crs_ellipsoid() {
    let wgs = bounds(10.0, 20.0, 11.0, 21.0)
        .expand_by(km(1000.0))
        .unwrap();
    let tokyo = GeoBounds::new(10.0, 20.0, 11.0, 21.0, Crs::Tokyo)
        .unwrap()
        .expand_by(km(1000.0))
        .unwrap();
    assert_eq!(tokyo.crs(), Crs::Tokyo);
    assert_ne!(wgs.north(), tokyo.north());
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_validates() {
    let b = bounds(1.0, 170.0, 2.0, -170.0);
    let json = serde_json::to_string(&b).unwrap();
    assert_eq!(serde_json::from_str::<GeoBounds>(&json).unwrap(), b);
    let bad = r#"{"south":5.0,"west":0.0,"north":1.0,"east":0.0,"crs":"Wgs84"}"#;
    assert!(serde_json::from_str::<GeoBounds>(bad).is_err());
}

pub(super) fn lon_near_antimeridian() -> impl Strategy<Value = f64> {
    prop_oneof![
        -180.0..=180.0_f64,
        170.0..=180.0_f64,
        -180.0..=-170.0_f64,
        Just(180.0),
        Just(-180.0),
    ]
}

fn point() -> impl Strategy<Value = Coordinate> {
    (
        prop_oneof![-90.0..=90.0_f64, Just(90.0), Just(-90.0)],
        lon_near_antimeridian(),
    )
        .prop_map(|(lat, lon)| wgs84(lat, lon))
}

fn any_bounds() -> impl Strategy<Value = GeoBounds> {
    (
        -90.0..=90.0_f64,
        -90.0..=90.0_f64,
        lon_near_antimeridian(),
        lon_near_antimeridian(),
    )
        .prop_map(|(a, b, west, east)| bounds(a.min(b), west, a.max(b), east))
}

/// The span of the arc through `lons` by brute force: try each as the west edge.
fn brute_force_span(lons: &[f64]) -> f64 {
    lons.iter()
        .map(|&west| {
            lons.iter()
                .map(|&lon| LonArc::meridian(west).offset(lon))
                .fold(0.0, f64::max)
        })
        .fold(f64::INFINITY, f64::min)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn from_points_holds_every_point_in_the_minimal_span(
        points in prop::collection::vec(point(), 1..24),
    ) {
        let b = GeoBounds::from_points(points.iter().copied()).unwrap();
        for p in &points {
            prop_assert!(b.contains(*p).unwrap(), "{b:?} misses {p:?}");
        }
        let lons: Vec<f64> = points
            .iter()
            .filter(|p| p.lat().abs() < 90.0)
            .map(|p| p.lon())
            .collect();
        if !lons.is_empty() {
            prop_assert!((b.arc().span() - brute_force_span(&lons)).abs() < 1e-9);
        }
    }

    #[test]
    fn from_points_ignores_the_order_of_the_points(
        mut points in prop::collection::vec(point(), 1..24),
    ) {
        let forward = GeoBounds::from_points(points.iter().copied()).unwrap();
        points.reverse();
        prop_assert_eq!(forward, GeoBounds::from_points(points).unwrap());
    }

    #[test]
    fn union_holds_both_boxes_and_commutes(a in any_bounds(), b in any_bounds(), p in point()) {
        let union = a.union(&b).unwrap();
        prop_assert_eq!(union, b.union(&a).unwrap());
        if a.contains(p).unwrap() || b.contains(p).unwrap() {
            prop_assert!(union.contains(p).unwrap(), "{union:?} misses {p:?}");
        }
        prop_assert!(union.intersects(&a).unwrap() && union.intersects(&b).unwrap());
    }

    /// Whole-degree edges make the half-degree grid an exact model of the circle:
    /// the union's span is 360° less the widest run of grid points neither holds.
    #[test]
    fn union_is_the_shortest_arc_holding_both(
        a in (-180..=180_i32, -180..=180_i32),
        b in (-180..=180_i32, -180..=180_i32),
    ) {
        let arc = |(west, east): (i32, i32)| LonArc {
            west: f64::from(west),
            east: f64::from(east),
        };
        let (a, b) = (arc(a), arc(b));
        let held: Vec<bool> = (-360..360)
            .map(|half| f64::from(half) / 2.0)
            .map(|lon| a.contains(lon) || b.contains(lon))
            .collect();
        // Doubled, so a run may wrap the end of the grid.
        let (mut widest, mut run) = (0_u32, 0_u32);
        for index in 0..2 * held.len() {
            run = if held[index % held.len()] { 0 } else { run + 1 };
            widest = widest.max(run);
        }
        let expected = if widest == 0 {
            360.0
        } else {
            360.0 - f64::from(widest + 1) / 2.0
        };
        prop_assert_eq!(a.union(b).span(), expected, "{:?} {:?}", a, b);
    }

    #[test]
    fn intersects_commutes_and_is_true_for_a_shared_point(
        a in any_bounds(),
        b in any_bounds(),
        p in point(),
    ) {
        prop_assert_eq!(a.intersects(&b).unwrap(), b.intersects(&a).unwrap());
        if a.contains(p).unwrap() && b.contains(p).unwrap() {
            prop_assert!(a.intersects(&b).unwrap());
        }
    }

    #[test]
    fn center_is_inside_the_box(a in any_bounds()) {
        prop_assert!(a.contains(a.center()).unwrap());
    }

    #[test]
    fn expand_by_holds_every_point_within_the_distance(
        south in -89.0..=89.0_f64,
        height in 0.0..=1.0_f64,
        west in lon_near_antimeridian(),
        width in 0.0..=10.0_f64,
        distance_km in 0.001..=3000.0_f64,
        from_lat in 0.0..=1.0_f64,
        from_lon in 0.0..=1.0_f64,
        bearing in 0.0..360.0_f64,
    ) {
        let north = (south + height).min(90.0);
        let east = wrap_longitude(west + width);
        let b = bounds(south, west, north, east);
        let distance = km(distance_km);
        let grown = b.expand_by(distance).unwrap();

        let start = wgs84(
            south + from_lat * (north - south),
            wrap_longitude(west + from_lon * b.arc().span()),
        );
        prop_assert!(b.contains(start).unwrap());
        let target = destination(start, bearing, distance).unwrap();
        prop_assert!(grown.contains(target).unwrap(), "{grown:?} misses {target:?}");
        prop_assert!(grown.contains(start).unwrap());
    }
}
