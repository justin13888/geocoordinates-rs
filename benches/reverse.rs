//! Load time and query latency of the bundled GeoNames `cities1000` gazetteer. It
//! uses no benchmark framework, so it adds no dependency:
//! `cargo bench --features reverse-bundled --bench reverse`.

use std::hint::black_box;
use std::time::Instant;

use geocoordinates::reverse::{FeatureClass, Gazetteer, ReverseGeocoder, ReverseOptions};
use geocoordinates::{Coordinate, Length, LengthUnit};

const LOADS: u32 = 5;
const QUERIES: usize = 100_000;

/// A small deterministic generator (SplitMix64), so every run sees the same points.
struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// A point uniform over the sphere, so most queries are at sea.
    fn anywhere(&mut self) -> Coordinate {
        let lat = (2.0 * self.unit() - 1.0).asin().to_degrees();
        Coordinate::wgs84(lat, 360.0 * self.unit() - 180.0).expect("in range by construction")
    }

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
    let seeds: Vec<Coordinate> = (0..2_000).map(|_| rng.anywhere()).collect();
    let wide = ReverseOptions::default()
        .with_max_distance(Length::from_unit(20_000.0, LengthUnit::Kilometer));
    let places: Vec<Coordinate> = seeds
        .iter()
        .filter_map(|&s| gazetteer.reverse(s, &wide).ok().flatten())
        .map(|place| place.coordinate())
        .collect();

    let near: Vec<Coordinate> = (0..QUERIES).map(|_| rng.near(&places)).collect();
    let anywhere: Vec<Coordinate> = (0..QUERIES).map(|_| rng.anywhere()).collect();
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
