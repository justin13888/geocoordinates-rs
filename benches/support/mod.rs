//! The deterministic point source every benchmark draws from, included by each
//! bench with `mod support;`. Not every bench uses every item.
#![allow(dead_code)]

use geocoordinates::Coordinate;

/// A small deterministic generator (SplitMix64), so every run sees the same points.
pub struct Rng(pub u64);

impl Rng {
    /// A number uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// A point uniform on the sphere.
    pub fn sphere(&mut self) -> Coordinate {
        let lat = (2.0 * self.unit() - 1.0).asin().to_degrees();
        let lon = 360.0 * self.unit() - 180.0;
        Coordinate::wgs84(lat, lon).expect("in range by construction")
    }
}
