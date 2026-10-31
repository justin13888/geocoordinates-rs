//! Pins `expand_by`'s edges to independently computed geodesic solutions, and
//! the boundary cases of `intersects` beside them.

use proptest::prelude::*;

use super::tests::lon_near_antimeridian;
use super::*;
use crate::geodesy::destination;

fn bounds(south: f64, west: f64, north: f64, east: f64) -> GeoBounds {
    GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap()
}

fn geodesic() -> Geodesic {
    let wgs84 = Ellipsoid::for_crs(Crs::Wgs84).unwrap();
    Geodesic::new(wgs84.semi_major().meters(), wgs84.flattening())
}

/// The meridian distance, in meters, from `lat` to its nearer pole.
fn pole_gap(lat: f64) -> f64 {
    let pole = if lat >= 0.0 { 90.0 } else { -90.0 };
    geodesic().inverse(lat, 0.0, pole, 0.0)
}

/// The best of 18,001 bearings, then of as many again around the best so far,
/// since the peak narrows and moves towards due north or south near a pole.
fn sampled_reach(lat: f64, meters: f64) -> f64 {
    let g = geodesic();
    let (mut low, mut step, mut best) = (0.0_f64, 0.01_f64, (0.0_f64, f64::NEG_INFINITY));
    for _ in 0..3 {
        for index in 0..=18_000 {
            let bearing = (low + f64::from(index) * step).min(180.0);
            let (_, lon): (f64, f64) = g.direct(lat, 0.0, bearing, meters);
            if lon > best.1 {
                best = (bearing, lon);
            }
        }
        low = (best.0 - step).max(0.0);
        step /= 9_000.0;
    }
    best.1
}

#[test]
fn longitude_reach_finds_the_peak_a_dense_sweep_finds() {
    for (lat, meters) in [
        (0.0, 1_000.0),
        (11.0, 100_000.0),
        (60.0, 100_000.0),
        (-45.0, 3_000_000.0),
        (85.0, 200_000.0),
    ] {
        let found = longitude_reach(&geodesic(), lat, meters);
        let sampled = sampled_reach(lat, meters);
        // The sweep can only fall short of the peak, by a second-order amount.
        assert!(
            found >= sampled - 1e-11,
            "{lat} {meters}: {found} < {sampled}"
        );
        assert!(
            found <= sampled + 1e-6,
            "{lat} {meters}: {found} > {sampled}"
        );
    }
}

#[test]
fn longitude_reach_finds_the_peak_up_to_the_pole_gap_cutoff() {
    for lat in [0.0, 30.0, -45.0, 60.0] {
        for fraction in [0.5, 0.9, 0.999, 0.999_999] {
            let meters = fraction * pole_gap(lat);
            let found = longitude_reach(&geodesic(), lat, meters);
            let sampled = sampled_reach(lat, meters);
            let close = (found - sampled).abs() < 1e-6;
            assert!(close, "{lat} {meters}: {found} against {sampled}");
        }
    }
}

#[test]
fn the_equator_reaches_almost_a_quarter_turn_just_inside_the_cutoff() {
    let gap = pole_gap(0.0);
    assert!((10_001_000.0..10_003_000.0).contains(&gap), "{gap}");
    let grown = bounds(0.0, 0.0, 0.0, 0.0)
        .expand_by(Length::from_meters(gap - 1.0))
        .unwrap();
    assert!(grown.north() < 90.0 && grown.south() > -90.0, "{grown:?}");
    assert!(grown.east() > 89.0 && grown.east() < 91.0, "{grown:?}");
    assert!(grown.west() < -89.0 && grown.west() > -91.0, "{grown:?}");
    // At the cutoff itself the pole is in reach, so the box takes every longitude.
    let at_cutoff = bounds(0.0, 0.0, 0.0, 0.0)
        .expand_by(Length::from_meters(gap))
        .unwrap();
    assert_eq!((at_cutoff.west(), at_cutoff.east()), (-180.0, 180.0));
}

proptest! {
    /// From 3000 km up to the cutoff, with every expanded box still open: past
    /// the cutoff every longitude is held, so containment would prove nothing.
    #[test]
    fn expand_by_holds_every_point_up_to_the_pole_gap_cutoff(
        south in -60.0..=60.0_f64,
        height in 0.0..=1.0_f64,
        west in lon_near_antimeridian(),
        width in 0.0..=10.0_f64,
        toward_cutoff in prop_oneof![0.0..=0.999_999_f64, 0.99..=0.999_999_f64],
        from_lat in 0.0..=1.0_f64,
        from_lon in 0.0..=1.0_f64,
        // The peak of reach closes on due north or south near the cutoff.
        bearing in prop_oneof![0.0..360.0_f64, -1.0..1.0_f64, 179.0..181.0_f64],
    ) {
        let north = south + height;
        let b = bounds(south, west, north, wrap_longitude(west + width));
        let cutoff = pole_gap(north).min(pole_gap(south));
        let meters = 3_000_000.0 + toward_cutoff * (cutoff - 3_000_000.0);
        let grown = b.expand_by(Length::from_meters(meters)).unwrap();
        prop_assert!(grown.north() < 90.0 && grown.south() > -90.0, "{grown:?}");
        prop_assert!(grown.arc().span() < 360.0, "{grown:?}");

        let start = Coordinate::wgs84(
            south + from_lat * height,
            wrap_longitude(west + from_lon * b.arc().span()),
        )
        .unwrap();
        let target = destination(start, bearing.rem_euclid(360.0), Length::from_meters(meters))
            .unwrap();
        prop_assert!(grown.contains(target).unwrap(), "{grown:?} misses {target:?}");
    }
}

#[test]
fn each_moved_edge_sits_one_margin_past_the_exact_solution() {
    const METERS: f64 = 100_000.0;
    let grown = bounds(10.0, 20.0, 11.0, 21.0)
        .expand_by(Length::from_meters(METERS))
        .unwrap();
    let g = geodesic();
    let (north, _): (f64, f64) = g.direct(11.0, 0.0, 0.0, METERS);
    let (south, _): (f64, f64) = g.direct(10.0, 0.0, 180.0, METERS);
    let close = |a: f64, b: f64| (a - b).abs() < 1e-11;
    assert!(close(grown.north(), north + EXPAND_MARGIN_DEG));
    assert!(close(grown.south(), south - EXPAND_MARGIN_DEG));
    // Longitude reach is measured from the edge nearest the pole.
    let reach = longitude_reach(&g, 11.0, METERS) + EXPAND_MARGIN_DEG;
    assert!(close(grown.east(), 21.0 + reach));
    assert!(close(grown.west(), 20.0 - reach));
}

#[test]
fn a_wide_box_stays_open_while_its_edges_have_not_met() {
    // 300° wide, grown by under a degree a side: far from circling the globe.
    let grown = bounds(0.0, -150.0, 1.0, 150.0)
        .expand_by(Length::from_meters(100_000.0))
        .unwrap();
    assert!(grown.west() < -150.0 && grown.west() > -152.0, "{grown:?}");
    assert!(grown.east() > 150.0 && grown.east() < 152.0, "{grown:?}");
}

#[test]
fn boxes_touching_along_one_parallel_intersect_either_way_round() {
    let (low, high) = (bounds(0.0, 0.0, 10.0, 5.0), bounds(10.0, 0.0, 20.0, 5.0));
    assert!(low.intersects(&high).unwrap());
    assert!(high.intersects(&low).unwrap());
}
