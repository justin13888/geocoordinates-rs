//! Property: every producer's output is a valid coordinate in its input's CRS.

use proptest::prelude::*;

use super::*;
use crate::geodesy::{GeodesicIntersection, intersection};

fn latitude() -> impl Strategy<Value = f64> {
    prop_oneof![-90.0..=90.0_f64, Just(90.0), Just(-90.0), Just(0.0)]
}

fn longitude() -> impl Strategy<Value = f64> {
    prop_oneof![-180.0..=180.0_f64, Just(180.0), Just(-180.0)]
}

fn geodetic_crs() -> impl Strategy<Value = Crs> {
    prop::sample::select(vec![Crs::Wgs84, Crs::Nad27, Crs::Tokyo, Crs::Pulkovo42])
}

fn position(crs: Crs) -> impl Strategy<Value = Coordinate> {
    (latitude(), longitude()).prop_map(move |(lat, lon)| Coordinate::new(lat, lon, crs).unwrap())
}

/// A producer's output: in range, longitude in `[-180, 180)`, the input's CRS,
/// and no height.
fn assert_valid(p: &Coordinate, crs: Crs) -> core::result::Result<(), TestCaseError> {
    prop_assert!((-90.0..=90.0).contains(&p.lat()), "{p:?}");
    prop_assert!((-180.0..180.0).contains(&p.lon()), "{p:?}");
    prop_assert_eq!(p.crs(), crs);
    prop_assert_eq!(p.height(), None);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn producer_outputs_always_validate(
        (a, b, crs) in geodetic_crs().prop_flat_map(|crs| (position(crs), position(crs), Just(crs))),
        bearing in prop_oneof![-720.0..=720.0_f64, Just(0.0), Just(180.0)],
        bearing_b in 0.0..360.0_f64,
        distance in prop_oneof![0.0..=40_000_000.0_f64, Just(0.0)],
        fraction in 0.0..=1.0_f64,
    ) {
        assert_valid(&destination(a, bearing, Length::from_meters(distance)).unwrap(), crs)?;
        assert_valid(&midpoint(a, b).unwrap(), crs)?;
        assert_valid(&intermediate(a, b, fraction).unwrap(), crs)?;
        if let Intersection::Crossing(p) = intersection_spherical(a, bearing, b, bearing_b).unwrap() {
            assert_valid(&p, crs)?;
        }
        if let Ok(GeodesicIntersection::Crossing(p)) = intersection(a, bearing, b, bearing_b) {
            assert_valid(&p, crs)?;
        }
    }
}
