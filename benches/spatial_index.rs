//! Build and query timings for `SpatialIndex` at 10^5 and 10^6 points, against a
//! brute-force geodesic scan. It uses no benchmark framework, so it adds no
//! dependency: `cargo bench --features index --bench spatial_index`.

use std::hint::black_box;
use std::time::{Duration, Instant};

use geocoordinates::geodesy::geodesic_distance;
use geocoordinates::index::SpatialIndex;
use geocoordinates::{Coordinate, Length, LengthUnit};

const SIZES: [usize; 2] = [100_000, 1_000_000];
const QUERIES: usize = 1_000;
const BRUTE_FORCE_QUERIES: usize = 3;
const NEAREST_K: usize = 10;
const RADIUS_KM: f64 = 50.0;

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

    /// A point uniform on the sphere.
    fn coordinate(&mut self) -> Coordinate {
        let lat = (2.0 * self.unit() - 1.0).asin().to_degrees();
        let lon = 360.0 * self.unit() - 180.0;
        Coordinate::wgs84(lat, lon).expect("in range by construction")
    }
}

fn per_query(total: Duration, queries: usize) -> Duration {
    total / u32::try_from(queries).expect("few queries")
}

fn main() -> geocoordinates::Result<()> {
    let radius = Length::from_unit(RADIUS_KM, LengthUnit::Kilometer);
    for size in SIZES {
        let mut rng = Rng(42);
        let points: Vec<Coordinate> = (0..size).map(|_| rng.coordinate()).collect();
        let centers: Vec<Coordinate> = (0..QUERIES).map(|_| rng.coordinate()).collect();

        let start = Instant::now();
        let index = SpatialIndex::new(points.iter().map(|point| (*point, ())))?;
        println!("n={size}: build {:?}", start.elapsed());

        let start = Instant::now();
        let mut hits = 0;
        for center in &centers {
            hits += black_box(index.within(*center, radius)?).len();
        }
        println!(
            "n={size}: within({RADIUS_KM} km) {:?}/query, {} hits/query",
            per_query(start.elapsed(), QUERIES),
            hits / QUERIES
        );

        let start = Instant::now();
        for center in &centers {
            black_box(index.nearest(*center, NEAREST_K)?);
        }
        println!(
            "n={size}: nearest({NEAREST_K}) {:?}/query",
            per_query(start.elapsed(), QUERIES)
        );

        let start = Instant::now();
        for center in &centers[..BRUTE_FORCE_QUERIES] {
            for point in &points {
                black_box(geodesic_distance(*center, *point)?);
            }
        }
        println!(
            "n={size}: brute-force scan {:?}/query",
            per_query(start.elapsed(), BRUTE_FORCE_QUERIES)
        );
    }
    Ok(())
}
