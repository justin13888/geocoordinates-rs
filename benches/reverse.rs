//! Load time and query latency of the bundled GeoNames `cities1000` gazetteer. It
//! uses no benchmark framework, so it adds no dependency:
//! `cargo bench --features reverse-bundled --bench reverse`.

mod support;

use std::hint::black_box;
use std::time::Instant;

use support::Rng;

use geocoordinates::reverse::{FeatureClass, Gazetteer, ReverseGeocoder, ReverseOptions};
use geocoordinates::{Coordinate, Length, LengthUnit};

const LOADS: u32 = 5;
const QUERIES: usize = 100_000;

impl Rng {
    /// A point about 10 km from a random place in the gazetteer: where photos are.
    fn near(&mut self, places: &[Coordinate]) -> Coordinate {
        let p = places[(self.unit() * places.len() as f64) as usize];
        let lat = (p.lat() + (self.unit() - 0.5) * 0.2).clamp(-90.0, 90.0);
        let lon = (p.lon() + (self.unit() - 0.5) * 0.2).clamp(-180.0, 180.0);
        Coordinate::wgs84(lat, lon).expect("in range by construction")
    }
}

fn per_query(label: &str, gazetteer: &Gazetteer, points: &[Coordinate], options: &ReverseOptions) {
    let start = Instant::now();
    let mut found = 0;
    for &point in points {
        if black_box(gazetteer.reverse(point, options)).is_ok_and(|place| place.is_some()) {
            found += 1;
        }
    }
    let each = start.elapsed() / points.len() as u32;
    println!(
        "{label}: {each:?} per query, {found} of {} resolved",
        points.len()
    );
}

fn main() -> geocoordinates::Result<()> {
    let start = Instant::now();
    for _ in 0..LOADS {
        black_box(Gazetteer::bundled());
    }
    println!("load (decode + index): {:?}", start.elapsed() / LOADS);

    let gazetteer = Gazetteer::bundled();
    let mut rng = Rng(42);
    // Real place centers: the place nearest each of 2,000 random points, at any
    // distance.
    let seeds: Vec<Coordinate> = (0..2_000).map(|_| rng.sphere()).collect();
    let wide = ReverseOptions::default()
        .with_max_distance(Length::from_unit(20_000.0, LengthUnit::Kilometer));
    let places: Vec<Coordinate> = seeds
        .iter()
        .filter_map(|&s| gazetteer.reverse(s, &wide).ok().flatten())
        .map(|place| place.coordinate())
        .collect();

    let near: Vec<Coordinate> = (0..QUERIES).map(|_| rng.near(&places)).collect();
    let anywhere: Vec<Coordinate> = (0..QUERIES).map(|_| rng.sphere()).collect();
    let defaults = ReverseOptions::default();
    per_query("near a place, defaults", &gazetteer, &near, &defaults);
    per_query("anywhere, defaults", &gazetteer, &anywhere, &defaults);
    let seats = ReverseOptions::default().with_min_feature_class(FeatureClass::Admin2Seat);
    per_query(
        "near a place, admin2 seats or larger",
        &gazetteer,
        &near,
        &seats,
    );
    let gcj: Vec<Coordinate> = near
        .iter()
        .take(QUERIES / 10)
        .map(|p| Coordinate::gcj02(p.lat(), p.lon()).expect("in range"))
        .collect();
    per_query("near a place, GCJ-02 input", &gazetteer, &gcj, &defaults);
    Ok(())
}
