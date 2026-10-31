//! Pins `expand_by`'s edges to independently computed geodesic solutions, and
//! the boundary cases of `intersects` beside them.

use super::*;

fn bounds(south: f64, west: f64, north: f64, east: f64) -> GeoBounds {
    GeoBounds::new(south, west, north, east, Crs::Wgs84).unwrap()
}

fn geodesic() -> Geodesic {
    let wgs84 = Ellipsoid::for_crs(Crs::Wgs84).unwrap();
    Geodesic::new(wgs84.semi_major().meters(), wgs84.flattening())
}

fn sampled_reach(lat: f64, meters: f64) -> f64 {
    let g = geodesic();
    (0..=18_000)
        .map(|step| {
            let (_, lon): (f64, f64) = g.direct(lat, 0.0, f64::from(step) / 100.0, meters);
            lon
        })
        .fold(f64::NEG_INFINITY, f64::max)
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
