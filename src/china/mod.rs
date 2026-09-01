//! China coordinate obfuscation datums: WGS-84 ↔ GCJ-02 ↔ BD-09.
//!
//! Real GPS (WGS-84) is illegal to publish on Chinese maps, so providers apply
//! a deliberate nonlinear offset (**GCJ-02**, "Mars coordinates"). Baidu adds a
//! second layer (**BD-09**).
//!
//! ## Exactness (visible in the type signatures)
//!
//! | Direction | Nature | API |
//! |---|---|---|
//! | WGS-84 → GCJ-02 | exact forward offset | [`Wgs84::try_to_gcj02`] / [`TryFrom`] |
//! | GCJ-02 → BD-09 | exact (empirical) forward | [`Gcj02::try_to_bd09`] / [`TryFrom`] |
//! | WGS-84 → BD-09 | exact composition | [`Wgs84::try_to_bd09`] / [`TryFrom`] |
//! | GCJ-02 → WGS-84 | **approximate** inverse | [`Gcj02::try_to_wgs84_refined`] → [`Approx`](crate::Approx) |
//! | BD-09 → GCJ-02 | **approximate** inverse | [`Bd09::try_to_gcj02_refined`] → [`Approx`](crate::Approx) |
//! | BD-09 → WGS-84 | **approximate** composition | [`Bd09::try_to_wgs84_refined`] → [`Approx`](crate::Approx) |
//!
//! Outside China (see [`out_of_china`]) every conversion is the identity. This
//! creates a documented discontinuity at the border. All conversion methods
//! validate finite, in-range coordinates before applying the transform.

mod baidu_mercator;
mod bd09;
mod gcj02;

pub use baidu_mercator::BaiduMercator;

use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};

/// Semi-major axis `a` of the **Krasovsky 1940** ellipsoid, in meters.
///
/// The GCJ-02 offset is defined against Krasovsky 1940 (`a = 6378245.0`,
/// `1/f = 298.3`), **not** WGS-84 — using WGS-84's `6378137.0` here is a
/// recurring ~100 m-scale bug. Paired with [`EE`], the Krasovsky eccentricity.
pub(crate) const GCJ_A: f64 = 6_378_245.0;
/// Eccentricity squared `e²` of the Krasovsky 1940 ellipsoid (`2f − f²`).
pub(crate) const EE: f64 = 0.006_693_421_622_965_943;
/// Baidu's magic angular constant, `π · 3000 / 180`.
///
/// The canonical BD-09 constant. Plain `π` (used by several stale ports) is a
/// bug that introduces a systematic ~2 m error; do not use it.
pub(crate) const X_PI: f64 = std::f64::consts::PI * 3000.0 / 180.0;
/// Baidu's BD-09 latitude offset, in degrees (the `+0.0060` nudge).
pub(crate) const BD_DLAT: f64 = 0.006_0;
/// Baidu's BD-09 longitude offset, in degrees (the `+0.0065` nudge).
pub(crate) const BD_DLON: f64 = 0.006_5;

/// Bounding-box gate: every China conversion is the identity outside it.
///
/// Box: `72.004 ≤ lon ≤ 137.8347`, `0.8293 ≤ lat ≤ 55.8271`. This is the
/// de-facto-standard rectangle used by every mainstream port (eviltransform,
/// coordtransform, …), so our results match theirs at the border.
///
/// The hard rectangle creates a **documented discontinuity**: a point just
/// inside the box is offset, a point just outside is not. It is also coarse —
/// it includes ocean and neighboring countries and does not model the true,
/// jagged GCJ distortion boundary. A future refinement could replace it with a
/// precise polygon and special-case the territories where the real border
/// matters (Hong Kong / Macau, which use WGS-84-ish data, and the Xinjiang /
/// border regions where the rectangle over-reaches). Kept as a box here for
/// parity with the reference implementations and to avoid a large vendored
/// vertex table.
#[must_use]
pub fn out_of_china(lat: f64, lon: f64) -> bool {
    !(72.004..=137.8347).contains(&lon) || !(0.8293..=55.8271).contains(&lat)
}

/// A WGS-84 position (real GPS / OpenStreetMap), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Wgs84 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A GCJ-02 position (Google China, AutoNavi/高德, Tencent), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gcj02 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// A BD-09 position (Baidu Maps only), in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bd09 {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

macro_rules! impl_latlon {
    ($($t:ty => $crs:expr),*) => {$(
        impl $t {
            /// Construct from latitude/longitude in decimal degrees.
            #[must_use]
            pub fn new(lat: f64, lon: f64) -> Self { Self { lat, lon } }

            /// Validate that latitude and longitude are finite and in range.
            ///
            /// # Errors
            ///
            /// Returns [`Error::OutOfRange`] when latitude falls outside ±90°,
            /// longitude outside ±180°, or either is not finite.
            pub fn validate(&self) -> Result<()> {
                Coordinate::new(self.lat, self.lon, $crs).validate()
            }

            /// Convert to the canonical [`Coordinate`], tagged with this
            /// datum's [`Crs`].
            ///
            /// Exact and total. Height is left unset.
            #[must_use]
            pub fn to_coordinate(self) -> Coordinate {
                Coordinate::new(self.lat, self.lon, $crs)
            }

            /// Extract this datum from a canonical [`Coordinate`].
            ///
            /// Any height is dropped: GCJ-02 and BD-09 are 2D obfuscations
            /// with no vertical datum.
            ///
            /// # Errors
            ///
            /// Returns [`Error::CrsMismatch`] unless `coord.crs` matches this
            /// datum, so a datum is never silently laundered, plus whatever
            /// [`Coordinate::validate`] reports for an out-of-range input.
            pub fn try_from_coordinate(coord: Coordinate) -> Result<Self> {
                coord.validate()?;
                if coord.crs == $crs {
                    Ok(Self { lat: coord.lat, lon: coord.lon })
                } else {
                    Err(Error::CrsMismatch { expected: $crs, found: coord.crs })
                }
            }
        }

        impl crate::coord::LatLon for $t {
            fn lat(&self) -> f64 { self.lat }
            fn lon(&self) -> f64 { self.lon }
            fn crs(&self) -> Crs { $crs }
        }

        /// Thin sugar over the named `to_coordinate` method.
        impl From<$t> for Coordinate {
            fn from(p: $t) -> Self { p.to_coordinate() }
        }

        /// Thin sugar over the named `try_from_coordinate` method.
        impl TryFrom<Coordinate> for $t {
            type Error = Error;

            fn try_from(coord: Coordinate) -> Result<Self> {
                Self::try_from_coordinate(coord)
            }
        }
    )*};
}

// --- Bridges to the canonical `Coordinate` ---
//
// Datum newtype → `Coordinate` is exact and total: `to_coordinate` injects the
// correct [`Crs`] tag and leaves height unset. `Coordinate` → newtype is
// fallible: `try_from_coordinate` requires the coordinate's [`Crs`] to match,
// so a datum is never silently laundered.
//
// The named inherent methods are the access path that crosses FFI; the `From` /
// `TryFrom` impls above are one-line sugar over them (see AGENTS.md).
impl_latlon!(Wgs84 => Crs::Wgs84, Gcj02 => Crs::Gcj02, Bd09 => Crs::Bd09);

#[cfg(test)]
mod tests {
    use super::*;

    /// The named method and its `From` sugar must agree, and must tag the
    /// coordinate with this datum's CRS.
    #[test]
    fn to_coordinate_tags_the_datum_and_matches_its_sugar() {
        let cases = [
            (Wgs84::new(39.915, 116.404).to_coordinate(), Crs::Wgs84),
            (Gcj02::new(39.915, 116.404).to_coordinate(), Crs::Gcj02),
            (Bd09::new(39.915, 116.404).to_coordinate(), Crs::Bd09),
        ];
        for (coord, expected) in cases {
            assert_eq!(coord.crs, expected);
            assert_eq!(coord.lat, 39.915);
            assert_eq!(coord.lon, 116.404);
            assert!(coord.height.is_none(), "height must be left unset");
        }

        assert_eq!(
            Coordinate::from(Wgs84::new(1.0, 2.0)),
            Wgs84::new(1.0, 2.0).to_coordinate()
        );
        assert_eq!(
            Coordinate::from(Gcj02::new(1.0, 2.0)),
            Gcj02::new(1.0, 2.0).to_coordinate()
        );
        assert_eq!(
            Coordinate::from(Bd09::new(1.0, 2.0)),
            Bd09::new(1.0, 2.0).to_coordinate()
        );
    }

    /// Newtype → `Coordinate` → newtype must round-trip exactly.
    #[test]
    fn coordinate_round_trip_is_exact() {
        let w = Wgs84::new(39.915, 116.404);
        assert_eq!(Wgs84::try_from_coordinate(w.to_coordinate()).unwrap(), w);

        let g = Gcj02::new(39.915, 116.404);
        assert_eq!(Gcj02::try_from_coordinate(g.to_coordinate()).unwrap(), g);

        let b = Bd09::new(39.915, 116.404);
        assert_eq!(Bd09::try_from_coordinate(b.to_coordinate()).unwrap(), b);
    }

    /// A datum must never be silently laundered: every wrong-CRS extraction
    /// reports `CrsMismatch` naming both the expected and the found datum.
    #[test]
    fn extracting_the_wrong_datum_is_a_crs_mismatch() {
        let gcj = Coordinate::gcj02(39.915, 116.404);
        assert!(matches!(
            Wgs84::try_from_coordinate(gcj),
            Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                found: Crs::Gcj02
            })
        ));

        let bd = Coordinate::bd09(39.915, 116.404);
        assert!(matches!(
            Gcj02::try_from_coordinate(bd),
            Err(Error::CrsMismatch {
                expected: Crs::Gcj02,
                found: Crs::Bd09
            })
        ));

        let wgs = Coordinate::wgs84(39.915, 116.404);
        assert!(matches!(
            Bd09::try_from_coordinate(wgs),
            Err(Error::CrsMismatch {
                expected: Crs::Bd09,
                found: Crs::Wgs84
            })
        ));

        // A classic datum is rejected too, not just the sibling China datums.
        let nad27 = Coordinate::new(39.915, 116.404, Crs::Nad27);
        assert!(matches!(
            Wgs84::try_from_coordinate(nad27),
            Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                found: Crs::Nad27
            })
        ));
    }

    /// Extraction validates before it inspects the CRS, so an out-of-range
    /// coordinate fails on range rather than being reported as a mismatch.
    #[test]
    fn extraction_rejects_out_of_range_before_checking_crs() {
        let bad = Coordinate::new(91.0, 0.0, Crs::Gcj02);
        assert!(matches!(
            Wgs84::try_from_coordinate(bad),
            Err(Error::OutOfRange { .. })
        ));
    }

    /// `TryFrom` is sugar and must agree with the named method exactly.
    #[test]
    fn try_from_sugar_matches_the_named_method() {
        let coord = Coordinate::wgs84(39.915, 116.404);
        assert_eq!(
            Wgs84::try_from(coord).unwrap(),
            Wgs84::try_from_coordinate(coord).unwrap()
        );

        let mismatched = Coordinate::bd09(39.915, 116.404);
        assert_eq!(
            Wgs84::try_from(mismatched).unwrap_err().to_string(),
            Wgs84::try_from_coordinate(mismatched)
                .unwrap_err()
                .to_string()
        );
    }

    /// The datum newtypes validate their own ranges.
    #[test]
    fn newtype_validate_checks_range() {
        assert!(Wgs84::new(39.915, 116.404).validate().is_ok());
        assert!(matches!(
            Gcj02::new(0.0, 181.0).validate(),
            Err(Error::OutOfRange { .. })
        ));
        assert!(matches!(
            Bd09::new(f64::NAN, 0.0).validate(),
            Err(Error::OutOfRange { .. })
        ));
    }
}
