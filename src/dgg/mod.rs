//! Discrete global grid indexing: Uber H3 and Google S2.
//!
//! These index a coordinate to a hierarchical cell for spatial joins, binning,
//! and proximity. Backed by external crates (`h3o`, `s2`) and gated behind the
//! `h3` / `s2` cargo features. A cell covers an area, so decoding a cell to a
//! representative point returns [`Approx`](crate::Approx).
//!
use crate::approx::Approx;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};

fn validate_encoding_coordinate(coord: Coordinate) -> Result<()> {
    coord.validate()?;
    if coord.crs != Crs::Wgs84 {
        return Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: coord.crs,
        });
    }
    Ok(())
}

/// An H3 cell index at a given resolution (0–15).
#[cfg(feature = "h3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct H3Cell(pub u64);

#[cfg(feature = "h3")]
impl H3Cell {
    /// Encode (index) a WGS-84 coordinate to its H3 cell at `resolution`
    /// (0–15).
    pub fn encode(coord: Coordinate, resolution: u8) -> Result<Self> {
        use h3o::{LatLng, Resolution};

        validate_encoding_coordinate(coord)?;
        let res = Resolution::try_from(resolution).map_err(|_| Error::InvalidValue {
            field: "H3 resolution",
            detail: "must be in 0..=15".into(),
        })?;
        let cell = LatLng::new(coord.lat, coord.lon)
            .map_err(|error| Error::InvalidValue {
                field: "coordinate",
                detail: error.to_string(),
            })?
            .to_cell(res);
        Ok(H3Cell(u64::from(cell)))
    }

    /// Decode to the cell's center coordinate; the error bound is the hexagon
    /// circumradius derived from the cell's exact area.
    pub fn decode(self) -> Result<Approx<Coordinate>> {
        use h3o::{CellIndex, LatLng};

        let cell = CellIndex::try_from(self.0).map_err(|_| Error::InvalidCellId {
            grid: "H3",
            value: self.0,
        })?;
        let center = LatLng::from(cell);
        let center = Coordinate::wgs84(center.lat(), center.lng());
        let radius = cell
            .boundary()
            .iter()
            .map(|vertex| {
                crate::geodesy::haversine_distance(
                    &center,
                    &Coordinate::wgs84(vertex.lat(), vertex.lng()),
                )
                .map(|distance| distance.meters())
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .fold(0.0, f64::max);
        Ok(Approx::new(center, radius))
    }
}

/// An S2 cell id.
#[cfg(feature = "s2")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct S2CellId(pub u64);

#[cfg(feature = "s2")]
impl S2CellId {
    /// Encode (index) a WGS-84 coordinate to its S2 cell at `level` (0–30).
    pub fn encode(coord: Coordinate, level: u8) -> Result<Self> {
        use s2::cellid::CellID;
        use s2::latlng::LatLng;

        validate_encoding_coordinate(coord)?;
        if level > 30 {
            return Err(Error::InvalidValue {
                field: "S2 level",
                detail: "must be in 0..=30".into(),
            });
        }
        let leaf = CellID::from(LatLng::from_degrees(coord.lat, coord.lon));
        Ok(S2CellId(leaf.parent(u64::from(level)).0))
    }

    /// Decode to the cell's center coordinate; the error bound is the greatest
    /// spherical distance from the center to the cell's four actual vertices.
    pub fn decode(self) -> Result<Approx<Coordinate>> {
        use s2::cell::Cell;
        use s2::cellid::CellID;
        use s2::latlng::LatLng;

        let cell = CellID(self.0);
        if !cell.is_valid() {
            return Err(Error::InvalidCellId {
                grid: "S2",
                value: self.0,
            });
        }
        let center = LatLng::from(cell);
        let center = Coordinate::wgs84(center.lat.deg(), center.lng.deg());
        let radius = Cell::from(cell)
            .vertices()
            .iter()
            .map(|vertex| {
                let vertex = LatLng::from(*vertex);
                crate::geodesy::haversine_distance(
                    &center,
                    &Coordinate::wgs84(vertex.lat.deg(), vertex.lng.deg()),
                )
                .map(|distance| distance.meters())
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .fold(0.0, f64::max);
        Ok(Approx::new(center, radius))
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
        let cell = H3Cell::encode(c(40.7128, -74.006), 9).unwrap();
        assert_eq!(cell.0, 617_733_151_020_810_239);
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_round_trips_within_its_bound() {
        for res in [0u8, 5, 9, 12, 15] {
            let approx = H3Cell::encode(c(40.7128, -74.006), res)
                .unwrap()
                .decode()
                .unwrap();
            assert!(approx.max_error_m() > 0.0);
            assert_within_meters(approx.value(), &c(40.7128, -74.006), approx.max_error_m());
        }
        // Finer resolutions give tighter bounds.
        assert!(
            H3Cell::encode(c(40.7128, -74.006), 12)
                .unwrap()
                .decode()
                .unwrap()
                .max_error_m()
                < H3Cell::encode(c(40.7128, -74.006), 5)
                    .unwrap()
                    .decode()
                    .unwrap()
                    .max_error_m()
        );
        // A resolution-9 cell's farthest boundary vertex is ≈ 205 m away.
        let bound9 = H3Cell::encode(c(40.7128, -74.006), 9)
            .unwrap()
            .decode()
            .unwrap()
            .max_error_m();
        assert!((205.0..206.0).contains(&bound9), "res-9 bound {bound9}");
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_matches_the_reference_id() {
        // (40.7128, −74.006) at level 20, per the canonical S2 library — this
        // also confirms the `s2` crate indexes correctly.
        let cell = S2CellId::encode(c(40.7128, -74.006), 20).unwrap();
        assert_eq!(cell.0, 9_926_595_630_970_437_632);
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_round_trips_within_its_bound() {
        for level in [1u8, 10, 20, 30] {
            let approx = S2CellId::encode(c(40.7128, -74.006), level)
                .unwrap()
                .decode()
                .unwrap();
            assert!(approx.max_error_m() > 0.0);
            assert_within_meters(approx.value(), &c(40.7128, -74.006), approx.max_error_m());
        }
        assert!(
            S2CellId::encode(c(40.7128, -74.006), 25)
                .unwrap()
                .decode()
                .unwrap()
                .max_error_m()
                < S2CellId::encode(c(40.7128, -74.006), 5)
                    .unwrap()
                    .decode()
                    .unwrap()
                    .max_error_m()
        );
        // A level-20 cell's farthest corner is ≈ 6–7 m away.
        let bound20 = S2CellId::encode(c(40.7128, -74.006), 20)
            .unwrap()
            .decode()
            .unwrap()
            .max_error_m();
        assert!((6.0..7.0).contains(&bound20), "level-20 bound {bound20}");
    }

    #[test]
    fn invalid_ids_and_levels_are_rejected() {
        #[cfg(feature = "h3")]
        {
            assert!(H3Cell::encode(c(0.0, 0.0), 16).is_err());
            assert!(H3Cell(0).decode().is_err());
            assert!(matches!(
                H3Cell::encode(Coordinate::gcj02(39.9, 116.4), 9),
                Err(Error::CrsMismatch {
                    expected: crate::Crs::Wgs84,
                    found: crate::Crs::Gcj02,
                })
            ));
        }
        #[cfg(feature = "s2")]
        {
            assert!(S2CellId::encode(c(0.0, 0.0), 31).is_err());
            assert!(S2CellId(0).decode().is_err());
            assert!(matches!(
                S2CellId::encode(Coordinate::bd09(39.9, 116.4), 20),
                Err(Error::CrsMismatch {
                    expected: crate::Crs::Wgs84,
                    found: crate::Crs::Bd09,
                })
            ));
        }
    }
}
