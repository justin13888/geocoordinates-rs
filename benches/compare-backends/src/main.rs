//! Compares three backing trees for exact geodesic queries. Each prefilters by
//! chord on reduced-latitude unit vectors and refines candidates with one Karney
//! geodesic, so their answers agree, which this checks, and only the tree differs.

use std::collections::HashSet;
use std::hint::black_box;
use std::time::{Duration, Instant};

use geocoordinates::index::SpatialIndex;
use geocoordinates::{Coordinate, Length};
use geographiclib_rs::{Geodesic, InverseGeodesic};
use kiddo::{ImmutableKdTree, SquaredEuclidean};
use rstar::{RTree, primitives::GeomWithData};

const SIZES: [usize; 2] = [100_000, 1_000_000];
const QUERIES: usize = 1_000;
const K: usize = 10;
const RADIUS_M: f64 = 50_000.0;
const FLATTENING: f64 = 1.0 / 298.257_223_563;
const SEMI_MINOR_M: f64 = 6_356_752.314_245;

type Vector = [f64; 3];

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn lat_lon(&mut self) -> (f64, f64) {
        (
            (2.0 * self.unit() - 1.0).asin().to_degrees(),
            360.0 * self.unit() - 180.0,
        )
    }
}

fn unit_vector((lat, lon): (f64, f64)) -> Vector {
    let (sin_lat, cos_lat) = lat.to_radians().sin_cos();
    let (sin_lon, cos_lon) = lon.to_radians().sin_cos();
    let z = (1.0 - FLATTENING) * sin_lat;
    let scale = z.hypot(cos_lat);
    [
        cos_lat / scale * cos_lon,
        cos_lat / scale * sin_lon,
        z / scale,
    ]
}

fn chord_sq_limit(meters: f64) -> f64 {
    (2.0 * (meters / SEMI_MINOR_M / 2.0).min(std::f64::consts::FRAC_PI_2).sin()).powi(2)
        * (1.0 + 1e-12)
        + 1e-14
}

struct Scene {
    geodesic: Geodesic,
    points: Vec<(f64, f64)>,
    vectors: Vec<Vector>,
    centers: Vec<(f64, f64)>,
}

impl Scene {
    fn distance(&self, center: (f64, f64), id: usize) -> f64 {
        let (lat, lon) = self.points[id];
        self.geodesic.inverse(center.0, center.1, lat, lon)
    }

    /// The chord-ranked candidates from a rival tree, refined to the exact k
    /// nearest: take the k nearest by chord, then every entry the k-th geodesic
    /// distance's chord limit still admits.
    fn refine_nearest(
        &self,
        center: (f64, f64),
        by_chord: impl Iterator<Item = (f64, usize)>,
    ) -> Vec<(f64, usize)> {
        let mut best: Vec<(f64, usize)> = Vec::with_capacity(K + 1);
        let mut limit = f64::INFINITY;
        for (chord_sq, id) in by_chord {
            if chord_sq > limit {
                break;
            }
            best.push((self.distance(center, id), id));
            best.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            best.truncate(K);
            if best.len() == K {
                limit = chord_sq_limit(best[K - 1].0);
            }
        }
        best
    }

    fn refine_within(&self, center: (f64, f64), ids: impl Iterator<Item = usize>) -> usize {
        ids.filter(|id| self.distance(center, *id) <= RADIUS_M)
            .count()
    }
}

fn report(backend: &str, size: usize, build: Duration, within: Duration, nearest: Duration) {
    let queries = QUERIES as u32;
    println!(
        "| {backend} | {size} | {build:.2?} | {:.2?} | {:.2?} |",
        within / queries,
        nearest / queries,
    );
}

fn in_crate(scene: &Scene) -> (Vec<usize>, Vec<HashSet<usize>>) {
    let size = scene.points.len();
    let coordinate = |(lat, lon)| Coordinate::wgs84(lat, lon).expect("in range");

    let start = Instant::now();
    let index = SpatialIndex::new(scene.points.iter().map(|&point| (coordinate(point), ())))
        .expect("one datum");
    let build = start.elapsed();

    let start = Instant::now();
    let radius = Length::from_meters(RADIUS_M);
    let counts = scene
        .centers
        .iter()
        .map(|&center| {
            black_box(index.within(coordinate(center), radius).expect("valid")).len()
        })
        .collect();
    let within = start.elapsed();

    let start = Instant::now();
    let nearest_sets = scene
        .centers
        .iter()
        .map(|&center| {
            black_box(index.nearest(coordinate(center), K).expect("valid"))
                .iter()
                .map(|hit| hit.index())
                .collect()
        })
        .collect();
    report("in-crate k-d tree", size, build, within, start.elapsed());
    (counts, nearest_sets)
}

fn with_rstar(scene: &Scene) -> (Vec<usize>, Vec<HashSet<usize>>) {
    let size = scene.points.len();
    let start = Instant::now();
    let tree: RTree<GeomWithData<Vector, usize>> = RTree::bulk_load(
        scene
            .vectors
            .iter()
            .enumerate()
            .map(|(id, vector)| GeomWithData::new(*vector, id))
            .collect(),
    );
    let build = start.elapsed();

    let start = Instant::now();
    let limit = chord_sq_limit(RADIUS_M);
    let counts = scene
        .centers
        .iter()
        .map(|&center| {
            let query = unit_vector(center);
            let ids = tree.locate_within_distance(query, limit).map(|hit| hit.data);
            black_box(scene.refine_within(center, ids))
        })
        .collect();
    let within = start.elapsed();

    let start = Instant::now();
    let nearest_sets = scene
        .centers
        .iter()
        .map(|&center| {
            let query = unit_vector(center);
            let walk = tree
                .nearest_neighbor_iter_with_distance_2(&query)
                .map(|(hit, chord_sq)| (chord_sq, hit.data));
            black_box(scene.refine_nearest(center, walk))
                .iter()
                .map(|hit| hit.1)
                .collect()
        })
        .collect();
    report("rstar 0.12", size, build, within, start.elapsed());
    (counts, nearest_sets)
}

fn with_kiddo(scene: &Scene) -> (Vec<usize>, Vec<HashSet<usize>>) {
    let size = scene.points.len();
    let start = Instant::now();
    let tree: ImmutableKdTree<f64, 3> = ImmutableKdTree::new_from_slice(&scene.vectors);
    let build = start.elapsed();

    let start = Instant::now();
    let limit = chord_sq_limit(RADIUS_M);
    let counts = scene
        .centers
        .iter()
        .map(|&center| {
            let query = unit_vector(center);
            let found = tree.within_unsorted::<SquaredEuclidean>(&query, limit);
            let ids = found.iter().map(|hit| hit.item as usize);
            black_box(scene.refine_within(center, ids))
        })
        .collect();
    let within = start.elapsed();

    // kiddo has no lazy walk by distance, so the exact form runs in two steps: the
    // k nearest by chord bound the answer, then a radius query at that bound.
    let start = Instant::now();
    let nearest_sets = scene
        .centers
        .iter()
        .map(|&center| {
            let query = unit_vector(center);
            let seed = tree.nearest_n::<SquaredEuclidean>(&query, K.try_into().expect("K > 0"));
            let bound = seed
                .iter()
                .map(|hit| scene.distance(center, hit.item as usize))
                .fold(0.0, f64::max);
            let mut found: Vec<(f64, usize)> = tree
                .within_unsorted::<SquaredEuclidean>(&query, chord_sq_limit(bound))
                .iter()
                .map(|hit| hit.item as usize)
                .map(|id| (scene.distance(center, id), id))
                .collect();
            found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            found.truncate(K);
            black_box(found).iter().map(|hit| hit.1).collect()
        })
        .collect();
    report("kiddo 5 (immutable)", size, build, within, start.elapsed());
    (counts, nearest_sets)
}

fn main() {
    println!("| backend | points | build | within 50 km, per query | nearest 10, per query |");
    println!("|---|---|---|---|---|");
    for size in SIZES {
        let mut rng = Rng(42);
        let points: Vec<(f64, f64)> = (0..size).map(|_| rng.lat_lon()).collect();
        let scene = Scene {
            geodesic: Geodesic::wgs84(),
            vectors: points.iter().copied().map(unit_vector).collect(),
            points,
            centers: (0..QUERIES).map(|_| rng.lat_lon()).collect(),
        };
        let ours = in_crate(&scene);
        let rstar = with_rstar(&scene);
        let kiddo = with_kiddo(&scene);
        assert!(ours == rstar, "rstar disagrees with the in-crate tree");
        assert!(ours == kiddo, "kiddo disagrees with the in-crate tree");
    }
}
