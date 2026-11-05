//! Exclusion zones: construction, membership at the edges, across the
//! antimeridian and at the poles, and what `apply` returns.

use proptest::prelude::*;

use super::{ExclusionZone, Precision, ZoneAction, apply, coarsen};
use crate::fix::{Accuracy, AccuracyKind, Confidence, Fix, RawSource};
use crate::geodesy::{destination, geodesic_distance};
use crate::index::GeoBounds;
use crate::{Coordinate, Crs, Error, Height, Length};

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn circle(lat: f64, lon: f64, radius_m: f64) -> ExclusionZone {
    ExclusionZone::from_circle(wgs84(lat, lon), Length::from_meters(radius_m)).unwrap()
}

#[test]
fn a_circle_rejects_a_bad_radius_and_a_datum_with_no_ellipsoid() {
    for meters in [-0.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            ExclusionZone::from_circle(wgs84(0.0, 0.0), Length::from_meters(meters)),
            Err(Error::InvalidValue {
                field: "radius",
                ..
            })
        ));
    }
    for crs in [Crs::Gcj02, Crs::Bd09] {
        assert!(matches!(
            ExclusionZone::from_circle(
                Coordinate::new(39.9, 116.4, crs).unwrap(),
                Length::from_meters(1.0)
            ),
            Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                ..
            })
        ));
    }
    // A zero radius holds its centre alone.
    let point = circle(10.0, 20.0, 0.0);
    assert!(point.contains(wgs84(10.0, 20.0)).unwrap());
    assert!(!point.contains(wgs84(10.0, 20.000_000_1)).unwrap());
}

#[test]
fn getters_return_what_built_the_zone() {
    let zone = circle(1.0, 2.0, 3.0);
    assert_eq!(
        zone.circle(),
        Some((wgs84(1.0, 2.0), Length::from_meters(3.0)))
    );
    assert_eq!((zone.bounds(), zone.crs()), (None, Crs::Wgs84));
    let bounds = GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Gcj02).unwrap();
    let zone = ExclusionZone::from_bounds(bounds);
    assert_eq!(
        (zone.bounds(), zone.circle(), zone.crs()),
        (Some(bounds), None, Crs::Gcj02)
    );
}

#[test]
fn a_circle_edge_is_inclusive() {
    let center = wgs84(51.5, -0.12);
    let edge = destination(center, 37.0, Length::from_meters(1_000.0)).unwrap();
    let measured = geodesic_distance(center, edge).unwrap();
    let exactly = ExclusionZone::from_circle(center, measured).unwrap();
    assert!(exactly.contains(edge).unwrap());
    let short = ExclusionZone::from_circle(center, measured - Length::from_meters(1e-6)).unwrap();
    assert!(!short.contains(edge).unwrap());
}

#[test]
fn a_circle_reaches_across_the_antimeridian() {
    let zone = circle(-17.0, 179.995, 2_000.0);
    // About 1.06 km east of the centre, on the far side of 180°.
    assert!(zone.contains(wgs84(-17.0, -179.995)).unwrap());
    assert!(zone.contains(wgs84(-17.0, 180.0)).unwrap());
    assert!(!zone.contains(wgs84(-17.0, -179.97)).unwrap());
    assert!(!zone.contains(wgs84(-17.0, 0.005)).unwrap());
}

#[test]
fn a_circle_at_a_pole_holds_the_pole_at_every_longitude() {
    let north = circle(90.0, 0.0, 5_000.0);
    for lon in [-180.0, -90.0, 0.0, 45.0, 180.0] {
        assert!(north.contains(wgs84(90.0, lon)).unwrap());
        // 0.04° of latitude from the pole is about 4.47 km.
        assert!(north.contains(wgs84(89.96, lon)).unwrap());
        assert!(!north.contains(wgs84(89.95, lon)).unwrap());
    }
    // A zone beside the pole reaches over it to the other side.
    let beside = circle(-89.99, 0.0, 3_000.0);
    assert!(beside.contains(wgs84(-89.99, 180.0)).unwrap()); // about 2.2 km over the pole
    assert!(beside.contains(wgs84(-90.0, 77.0)).unwrap());
}

#[test]
fn a_box_zone_handles_the_antimeridian_and_the_poles() {
    let crossing = ExclusionZone::from_bounds(
        GeoBounds::new(-21.0, 176.0, -12.0, -178.0, Crs::Wgs84).unwrap(),
    );
    assert!(crossing.contains(wgs84(-12.0, 176.0)).unwrap()); // a corner
    assert!(crossing.contains(wgs84(-17.0, -180.0)).unwrap());
    assert!(!crossing.contains(wgs84(-17.0, -177.9)).unwrap());
    let cap =
        ExclusionZone::from_bounds(GeoBounds::new(85.0, 10.0, 90.0, 20.0, Crs::Wgs84).unwrap());
    assert!(cap.contains(wgs84(90.0, -100.0)).unwrap());
    assert!(!cap.contains(wgs84(89.0, -100.0)).unwrap());
}

#[test]
fn membership_off_the_zone_datum_is_refused() {
    let point = Coordinate::new(10.0, 20.0, Crs::Nad27).unwrap();
    assert!(matches!(
        circle(10.0, 20.0, 1.0).contains(point),
        Err(Error::CrsMismatch { .. })
    ));
    let bounds =
        ExclusionZone::from_bounds(GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Wgs84).unwrap());
    assert!(matches!(
        bounds.contains(point),
        Err(Error::CrsMismatch { .. })
    ));
}

fn detailed_fix(lat: f64, lon: f64) -> Fix {
    let mut fix = Fix::from_coord(
        wgs84(lat, lon)
            .with_height(Height::Orthometric(30.0))
            .unwrap(),
    );
    fix.accuracy = Some(
        Accuracy::new(
            AccuracyKind::Bound,
            Some(Length::from_meters(5.0)),
            Some(Length::from_meters(8.0)),
        )
        .unwrap(),
    );
    fix.timestamp = Some(std::time::UNIX_EPOCH);
    fix.source = Some(RawSource::new(
        format!("{lat}, {lon}"),
        Confidence::new(0.9).unwrap(),
    ));
    fix
}

#[test]
fn apply_returns_a_fix_outside_every_zone_unchanged() {
    let zones = [circle(0.0, 0.0, 100.0), circle(10.0, 10.0, 100.0)];
    let fix = detailed_fix(5.0, 5.0);
    for action in [
        ZoneAction::Strip,
        ZoneAction::Coarsen(Precision::geohash(4).unwrap()),
    ] {
        assert_eq!(
            apply(fix.clone(), &zones, action).unwrap(),
            Some(fix.clone())
        );
        assert_eq!(apply(fix.clone(), &[], action).unwrap(), Some(fix.clone()));
    }
}

#[test]
fn apply_strips_or_coarsens_a_fix_in_any_zone() {
    let zones = [circle(0.0, 0.0, 100.0), circle(10.0, 10.0, 100.0)];
    let fix = detailed_fix(10.0, 10.000_5);
    assert_eq!(apply(fix.clone(), &zones, ZoneAction::Strip).unwrap(), None);

    let precision = Precision::geohash(5).unwrap();
    let coarse = apply(fix.clone(), &zones, ZoneAction::Coarsen(precision))
        .unwrap()
        .unwrap();
    let cell = coarsen(fix.coord, precision).unwrap();
    assert_eq!(coarse.coord, cell.coord);
    assert_eq!(coarse.coord.height(), None);
    assert_eq!(coarse.timestamp, fix.timestamp);
    assert_eq!(coarse.source, cell.source);
    let accuracy = coarse.accuracy.unwrap();
    assert_eq!(accuracy.kind(), AccuracyKind::Bound);
    assert_eq!(accuracy.vertical(), None);
    assert_eq!(
        accuracy.horizontal(),
        Some(cell.accuracy.unwrap().horizontal().unwrap() + Length::from_meters(5.0))
    );
}

#[test]
fn apply_checks_every_zone_datum_even_after_a_match() {
    let nad27 = ExclusionZone::from_bounds(GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Nad27).unwrap());
    let zones = [circle(0.0, 0.0, 100.0), nad27];
    assert!(matches!(
        apply(Fix::from_coord(wgs84(0.0, 0.0)), &zones, ZoneAction::Strip),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn coarsening_a_zone_fix_off_wgs84_is_refused() {
    let center = Coordinate::new(35.0, 139.0, Crs::Tokyo).unwrap();
    let zones = [ExclusionZone::from_circle(center, Length::from_meters(10.0)).unwrap()];
    let action = ZoneAction::Coarsen(Precision::geohash(5).unwrap());
    assert!(matches!(
        apply(Fix::from_coord(center), &zones, action),
        Err(Error::CrsMismatch { .. })
    ));
    assert_eq!(
        apply(Fix::from_coord(center), &zones, ZoneAction::Strip).unwrap(),
        None
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_validates() {
    let zone = circle(1.0, 2.0, 3.0);
    let json = serde_json::to_string(&zone).unwrap();
    assert!(json.starts_with("{\"Circle\":{\"center\":"), "{json}");
    assert_eq!(serde_json::from_str::<ExclusionZone>(&json).unwrap(), zone);
    let negative = json.replace("\"meters\":3.0", "\"meters\":-3.0");
    assert!(
        serde_json::from_str::<ExclusionZone>(&negative).is_err(),
        "{negative}"
    );

    let bounds =
        ExclusionZone::from_bounds(GeoBounds::new(0.0, 0.0, 1.0, 1.0, Crs::Wgs84).unwrap());
    let json = serde_json::to_string(&bounds).unwrap();
    assert_eq!(
        serde_json::from_str::<ExclusionZone>(&json).unwrap(),
        bounds
    );

    let action = ZoneAction::Coarsen(Precision::geohash(5).unwrap());
    let json = serde_json::to_string(&action).unwrap();
    assert_eq!(json, r#"{"Coarsen":{"grid":"Geohash","level":5}}"#);
    assert_eq!(serde_json::from_str::<ZoneAction>(&json).unwrap(), action);
    assert!(serde_json::from_str::<Precision>(r#"{"grid":"Geohash","level":13}"#).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A point exactly on the boundary (measured back by the same solver) is
    /// inside, and a point a millimetre beyond it is outside, wherever the zone
    /// is: near a pole, across the antimeridian, or anywhere else.
    #[test]
    fn circle_membership_is_exact_at_the_edge(
        lat in prop_oneof![-90.0..=90.0_f64, 89.0..=90.0_f64, -90.0..=-89.0_f64],
        lon in prop_oneof![-180.0..=180.0_f64, 179.0..=180.0_f64, -180.0..=-179.0_f64],
        bearing in 0.0..360.0_f64,
        radius_m in 1.0..200_000.0_f64,
    ) {
        let center = wgs84(lat, lon);
        let edge = destination(center, bearing, Length::from_meters(radius_m)).unwrap();
        let measured = geodesic_distance(center, edge).unwrap();
        let zone = ExclusionZone::from_circle(center, measured).unwrap();
        prop_assert!(zone.contains(edge).unwrap());
        let beyond = destination(center, bearing, Length::from_meters(radius_m + 1e-3)).unwrap();
        prop_assert!(!zone.contains(beyond).unwrap());
    }

    /// Stripping never leaks a fix from inside a zone, and a fix outside every
    /// zone always passes through untouched.
    #[test]
    fn strip_matches_membership(
        (lat, lon) in (-90.0..=90.0_f64, -180.0..=180.0_f64),
        (zone_lat, zone_lon) in (-90.0..=90.0_f64, -180.0..=180.0_f64),
        radius_m in 0.0..3_000_000.0_f64,
    ) {
        let zone = circle(zone_lat, zone_lon, radius_m);
        let fix = Fix::from_coord(wgs84(lat, lon));
        let inside = zone.contains(fix.coord).unwrap();
        let shared = apply(fix.clone(), &[zone], ZoneAction::Strip).unwrap();
        prop_assert_eq!(shared, if inside { None } else { Some(fix) });
    }
}
