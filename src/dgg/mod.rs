//! Discrete global grid indexing: Uber H3 and Google S2.
//!
//! These index a coordinate to a hierarchical cell for spatial joins, binning,
//! and proximity. Backed by external crates (`h3o`, `s2`) and gated behind the
//! `h3` / `s2` cargo features. A cell covers an area, so decoding a cell to a
//! representative point returns [`Approx`](crate::Approx).
//!
//! **FFI / wasm note:** H3 is exposed across the FFI boundary. S2 is **native
//! Rust only** — the `s2` crate pulls a `float_extras` dependency that does not
//! compile for `wasm32`, so it is excluded from the (wasm-targeting) FFI
//! bindings. Both are correct (each matches its canonical reference index);
//! only their binding reach differs.

use crate::approx::Approx;
use crate::coord::Coordinate;

/// An H3 cell index at a given resolution (0–15).
#[cfg(feature = "h3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct H3Cell(pub u64);

#[cfg(feature = "h3")]
impl H3Cell {
    /// Encode (index) a coordinate to its H3 cell at `resolution` (0–15;
    /// clamped). Exact.
    #[must_use]
    pub fn encode(coord: Coordinate, resolution: u8) -> Self {
        use h3o::{LatLng, Resolution};

        let res = Resolution::try_from(resolution.min(15)).unwrap_or(Resolution::Zero);
        let cell = LatLng::new(coord.lat, coord.lon)
            .expect("coordinate components are finite")
            .to_cell(res);
        H3Cell(u64::from(cell))
    }

    /// Decode to the cell's center coordinate; the error bound is the hexagon
    /// circumradius derived from the cell's exact area.
    #[must_use]
    pub fn decode(self) -> Approx<Coordinate> {
        use h3o::{CellIndex, LatLng};

        let cell = CellIndex::try_from(self.0).expect("valid H3 cell index");
        let center = LatLng::from(cell);
        // Regular-hexagon circumradius R from area A: A = (3√3/2)·R².
        let radius = (cell.area_m2() * 2.0 / (3.0 * 3.0_f64.sqrt())).sqrt();
        Approx::new(Coordinate::wgs84(center.lat(), center.lng()), radius)
    }
}

/// An S2 cell id.
#[cfg(feature = "s2")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct S2CellId(pub u64);

#[cfg(feature = "s2")]
impl S2CellId {
    /// Encode (index) a coordinate to its S2 cell at `level` (0–30; clamped).
    #[must_use]
    pub fn encode(coord: Coordinate, level: u8) -> Self {
        use s2::cellid::CellID;
        use s2::latlng::LatLng;

        let leaf = CellID::from(LatLng::from_degrees(coord.lat, coord.lon));
        S2CellId(leaf.parent(u64::from(level.min(30))).0)
    }

    /// Decode to the cell's center coordinate; the error bound is a conservative
    /// cell radius from the average cell area at this level.
    #[must_use]
    pub fn decode(self) -> Approx<Coordinate> {
        use s2::cellid::CellID;
        use s2::latlng::LatLng;

        /// IUGG mean Earth radius, meters.
        const EARTH_RADIUS_M: f64 = 6_371_008.8;

        let cell = CellID(self.0);
        let center = LatLng::from(cell);
        // Average cell area at level L = 4πR²/(6·4ᴸ); its equivalent-circle
        // radius is R·√(2/3)/2ᴸ. The ×1.5 covers S2's max-to-average size spread.
        let radius =
            1.5 * EARTH_RADIUS_M * (2.0_f64 / 3.0).sqrt() / 2.0_f64.powi(cell.level() as i32);
        Approx::new(
            Coordinate::wgs84(center.lat.deg(), center.lng.deg()),
            radius,
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::test_support::assert_within_meters;

    fn c(lat: f64, lon: f64) -> Coordinate {
        Coordinate::wgs84(lat, lon)
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_matches_the_reference_index() {
        // (40.7128, −74.006) at resolution 9, per the canonical H3 library
        // (0x892a1072893ffff).
        let cell = H3Cell::encode(c(40.7128, -74.006), 9);
        assert_eq!(cell.0, 617_733_151_020_810_239);
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_round_trips_within_its_bound() {
        for res in [0u8, 5, 9, 12, 15] {
            let approx = H3Cell::encode(c(40.7128, -74.006), res).decode();
            assert!(approx.max_error_m() > 0.0);
            assert_within_meters(approx.value(), &c(40.7128, -74.006), approx.max_error_m());
        }
        // Finer resolutions give tighter bounds.
        assert!(
            H3Cell::encode(c(40.7128, -74.006), 12)
                .decode()
                .max_error_m()
                < H3Cell::encode(c(40.7128, -74.006), 5)
                    .decode()
                    .max_error_m()
        );
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_matches_the_reference_id() {
        // (40.7128, −74.006) at level 20, per the canonical S2 library — this
        // also confirms the `s2` crate indexes correctly.
        let cell = S2CellId::encode(c(40.7128, -74.006), 20);
        assert_eq!(cell.0, 9_926_595_630_970_437_632);
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_round_trips_within_its_bound() {
        for level in [1u8, 10, 20, 30] {
            let approx = S2CellId::encode(c(40.7128, -74.006), level).decode();
            assert!(approx.max_error_m() > 0.0);
            assert_within_meters(approx.value(), &c(40.7128, -74.006), approx.max_error_m());
        }
        assert!(
            S2CellId::encode(c(40.7128, -74.006), 25)
                .decode()
                .max_error_m()
                < S2CellId::encode(c(40.7128, -74.006), 5)
                    .decode()
                    .max_error_m()
        );
    }
}
