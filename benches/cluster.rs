//! Timings for every `cluster` tool at 10^4, 10^5 and 10^6 points, and for
//! `ClusterIndex` updates against a rebuild. It uses no benchmark framework, so it
//! adds no dependency: `cargo bench --features cluster --bench cluster`.

use std::hint::black_box;
use std::time::{Duration, Instant, UNIX_EPOCH};

use geocoordinates::cluster::{
    ClusterIndex, bucket_by_geohash, centroid, dbscan, segment_trips, segment_visits,
};
use geocoordinates::fix::Fix;
use geocoordinates::index::GeoBounds;
use geocoordinates::{Coordinate, Crs, Length};

const SIZES: [usize; 3] = [10_000, 100_000, 1_000_000];
const CITIES: usize = 200;
const UPDATES: usize = 1_000;
const QUERIES: usize = 100;

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

    /// A photo-library shape: points scattered about 10 km around a few cities.
    fn photo(&mut self, cities: &[(f64, f64)]) -> Coordinate {
        let (lat, lon) = cities[(self.unit() * cities.len() as f64) as usize];
        let lat = lat + (self.unit() - 0.5) * 0.2;
        let lon = lon + (self.unit() - 0.5) * 0.2;
        Coordinate::wgs84(lat.clamp(-90.0, 90.0), lon.clamp(-180.0, 180.0))
            .expect("in range by construction")
    }
}

fn timed<T>(label: &str, size: usize, run: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = black_box(run());
    println!("n={size}: {label} {:?}", start.elapsed());
    result
}

fn main() -> geocoordinates::Result<()> {
    let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    let city_box = GeoBounds::new(40.0, -5.0, 50.0, 10.0, Crs::Wgs84)?;
    for size in SIZES {
        let mut rng = Rng(42);
        let cities: Vec<(f64, f64)> = (0..CITIES)
            .map(|_| {
                (
                    (2.0 * rng.unit() - 1.0).asin().to_degrees() * 0.8,
                    360.0 * rng.unit() - 180.0,
                )
            })
            .collect();
        let points: Vec<Coordinate> = (0..size).map(|_| rng.photo(&cities)).collect();

        timed("centroid", size, || centroid(&points))?;
        timed("bucket_by_geohash(6)", size, || {
            bucket_by_geohash(&points, 6)
        })?;

        // Few points fall within 30 m of each other here even at a million, so this
        // measures the query path and not the dense worst case.
        timed("dbscan(30 m, 4)", size, || {
            dbscan(&points, Length::from_meters(30.0), 4)
        })?;

        let fixes: Vec<Fix> = points
            .iter()
            .enumerate()
            .map(|(second, point)| {
                let mut fix = Fix::from_coord(*point);
                fix.timestamp = Some(UNIX_EPOCH + Duration::from_secs(second as u64));
                fix
            })
            .collect();
        let visits = timed("segment_visits(1 km, 0 s)", size, || {
            segment_visits(&fixes, Length::from_meters(1_000.0), Duration::ZERO)
        })?;
        timed("segment_trips(1 h)", size, || {
            segment_trips(&visits, Duration::from_secs(3_600))
        })?;

        let build = Instant::now();
        let mut index = black_box(ClusterIndex::new(
            points.iter().copied(),
            Default::default(),
        )?);
        let rebuild = build.elapsed();
        println!("n={size}: ClusterIndex build {rebuild:?}");

        let start = Instant::now();
        for zoom in 0..QUERIES {
            black_box(index.clusters(city_box, (zoom % 23) as u8)?);
        }
        println!(
            "n={size}: clusters(10° x 15° box) {:?}/query",
            start.elapsed() / QUERIES as u32
        );
        println!(
            "n={size}: clusters(world, zoom 0) {} pins",
            index.clusters(world, 0)?.len()
        );

        let added: Vec<Coordinate> = (0..UPDATES).map(|_| rng.photo(&cities)).collect();
        let start = Instant::now();
        let mut ids = Vec::with_capacity(UPDATES);
        for point in added {
            ids.push(index.insert(point)?);
        }
        let per_insert = start.elapsed() / UPDATES as u32;
        let start = Instant::now();
        for id in ids {
            index.remove(id)?;
        }
        let per_remove = start.elapsed() / UPDATES as u32;
        println!(
            "n={size}: insert {per_insert:?}, remove {per_remove:?}, a rebuild is worth {:.0} updates",
            rebuild.as_secs_f64() / per_insert.max(per_remove).as_secs_f64()
        );
    }
    Ok(())
}
