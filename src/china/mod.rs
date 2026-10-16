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
//! | WGS-84 → GCJ-02 | exact forward offset, infallible | [`Wgs84::to_gcj02`] / [`From`] |
//! | GCJ-02 → BD-09 | exact (empirical) forward | [`Gcj02::try_to_bd09`] / [`TryFrom`] |
//! | WGS-84 → BD-09 | exact composition | [`Wgs84::try_to_bd09`] / [`TryFrom`] |
//! | GCJ-02 → WGS-84 | **approximate** inverse, infallible | [`Gcj02::to_wgs84_fast`] / [`Gcj02::to_wgs84_refined`] → [`Approx`](crate::Approx) |
//! | BD-09 → GCJ-02 | **approximate** inverse | [`Bd09::try_to_gcj02_fast`] / [`Bd09::try_to_gcj02_refined`] → [`Approx`](crate::Approx) |
//! | BD-09 → WGS-84 | **approximate** composition | [`Bd09::try_to_wgs84_refined`] → [`Approx`](crate::Approx) |
//!
//! Every type here is valid by construction: [`Wgs84::new`], [`Gcj02::new`],
//! and [`Bd09::new`] reject a non-finite or out-of-range position, so the
//! conversions never re-validate their input.
//!
//! The WGS-84 ↔ GCJ-02 offset is the identity outside China (see
//! [`out_of_china`]), which creates a documented discontinuity at the border.
//! The BD-09 offset is **not** gated: like every reference port, Baidu's nudge
//! applies worldwide. Its longitude is wrapped into `[-180, 180)` at the
//! antimeridian, and a latitude it would push past a pole is reported as
//! [`Error::OutsideDomain`], so those conversions stay fallible.
//!
//! ```
//! use geocoordinates::china::{Gcj02, Wgs84};
//!
//! let wgs = Wgs84::new(39.915, 116.404)?;
//! let gcj: Gcj02 = wgs.to_gcj02(); // exact, infallible
//! let back = gcj.to_wgs84_refined(); // approximate, with an honest bound
//! assert!(back.max_error().meters() <= 0.5);
//! # Ok::<(), geocoordinates::Error>(())
//! ```

mod baidu_mercator;
mod bd09;
mod gcj02;

pub use baidu_mercator::BaiduMercator;

use crate::angle::wrap_longitude;
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

/// Bounding-box gate: the WGS-84 ↔ GCJ-02 offset is the identity outside it.
///
/// The BD-09 offset is not gated by it; Baidu's nudge applies worldwide.
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
///
/// ```
/// use geocoordinates::china::out_of_china;
///
/// assert!(!out_of_china(39.915, 116.404)); // Beijing
/// assert!(out_of_china(51.5, -0.12)); // London
/// ```
#[must_use]
pub fn out_of_china(lat: f64, lon: f64) -> bool {
    !near_china_box(lat, lon, 0.0)
}

/// Whether `(lat, lon)` lies inside the [`out_of_china`] box grown by
/// `margin_deg` on every side (`0.0` is the box itself).
pub(crate) fn near_china_box(lat: f64, lon: f64, margin_deg: f64) -> bool {
    (72.004 - margin_deg..=137.8347 + margin_deg).contains(&lon)
        && (0.8293 - margin_deg..=55.8271 + margin_deg).contains(&lat)
}

/// A WGS-84 position (real GPS / OpenStreetMap), in decimal degrees.
///
/// Valid by construction: build one with [`Wgs84::new`] or
/// [`Wgs84::try_from_coordinate`]. Latitude is in `[-90, 90]` and longitude in
/// `[-180, 180]`, both finite.
///
/// ```
/// use geocoordinates::china::Wgs84;
///
/// let wgs = Wgs84::new(39.915, 116.404)?;
/// assert_eq!((wgs.lat(), wgs.lon()), (39.915, 116.404));
/// assert!(Wgs84::new(90.5, 116.404).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "LatLonRecord"))]
pub struct Wgs84 {
    lat: f64,
    lon: f64,
}

/// A GCJ-02 position (Google China, AutoNavi/高德, Tencent), in decimal degrees.
///
/// Valid by construction: build one with [`Gcj02::new`] or
/// [`Gcj02::try_from_coordinate`]. Latitude is in `[-90, 90]` and longitude in
/// `[-180, 180]`, both finite.
///
/// ```
/// use geocoordinates::china::Gcj02;
///
/// let gcj = Gcj02::new(39.916_404, 116.410_244)?;
/// assert_eq!(gcj.lat(), 39.916_404);
/// assert!(Gcj02::new(39.9, f64::NAN).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "LatLonRecord"))]
pub struct Gcj02 {
    lat: f64,
    lon: f64,
}

/// A BD-09 position (Baidu Maps only), in decimal degrees.
///
/// Valid by construction: build one with [`Bd09::new`] or
/// [`Bd09::try_from_coordinate`]. Latitude is in `[-90, 90]` and longitude in
/// `[-180, 180]`, both finite.
///
/// ```
/// use geocoordinates::china::Bd09;
///
/// let bd = Bd09::new(39.922_700, 116.416_627)?;
/// assert_eq!(bd.lon(), 116.416_627);
/// assert!(Bd09::new(39.9, 180.5).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "LatLonRecord"))]
pub struct Bd09 {
    lat: f64,
    lon: f64,
}

/// Unvalidated serde shape shared by the three datum newtypes; deserialization
/// goes through each type's `new`.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct LatLonRecord {
    lat: f64,
    lon: f64,
}

macro_rules! impl_latlon {
    ($($t:ident => $crs:expr),*) => {$(
        impl $t {
            #[doc = concat!("Construct a [`", stringify!($t), "`] from latitude and longitude in decimal degrees.")]
            ///
            /// The range check is **closed** (longitude `180` is accepted) and
            /// does not normalize.
            ///
            /// # Errors
            ///
            /// Returns [`Error::OutOfRange`] when latitude is outside
            /// `[-90, 90]`, longitude is outside `[-180, 180]`, or either is not
            /// finite.
            ///
            /// ```
            #[doc = concat!("use geocoordinates::china::", stringify!($t), ";")]
            ///
            #[doc = concat!("let p = ", stringify!($t), "::new(39.9, 180.0)?; // closed range")]
            /// assert_eq!(p.lon(), 180.0);
            #[doc = concat!("assert!(", stringify!($t), "::new(-90.1, 0.0).is_err());")]
            /// # Ok::<(), geocoordinates::Error>(())
            /// ```
            pub fn new(lat: f64, lon: f64) -> Result<Self> {
                Coordinate::new(lat, lon, $crs).map(|c| Self { lat: c.lat(), lon: c.lon() })
            }

            /// Build from components the caller has already proven valid.
            ///
            /// For the conversions whose math keeps the result in range. Debug
            /// builds assert it.
            pub(crate) fn from_valid(lat: f64, lon: f64) -> Self {
                debug_assert!(
                    (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon),
                    "{}::from_valid out of range: lat={lat}, lon={lon}",
                    stringify!($t),
                );
                Self { lat, lon }
            }

            /// Latitude in decimal degrees, in `[-90, 90]`.
            ///
            /// ```
            #[doc = concat!("assert_eq!(geocoordinates::china::", stringify!($t), "::new(39.9, 116.4)?.lat(), 39.9);")]
            /// # Ok::<(), geocoordinates::Error>(())
            /// ```
            #[must_use]
            pub fn lat(&self) -> f64 {
                self.lat
            }

            /// Longitude in decimal degrees, in `[-180, 180]`.
            ///
            /// ```
            #[doc = concat!("assert_eq!(geocoordinates::china::", stringify!($t), "::new(39.9, 116.4)?.lon(), 116.4);")]
            /// # Ok::<(), geocoordinates::Error>(())
            /// ```
            #[must_use]
            pub fn lon(&self) -> f64 {
                self.lon
            }

            /// Convert to the canonical [`Coordinate`], tagged with this
            /// datum's [`Crs`].
            ///
            /// Exact and infallible. Height is left unset.
            ///
            /// ```
            /// use geocoordinates::Crs;
            #[doc = concat!("use geocoordinates::china::", stringify!($t), ";")]
            ///
            #[doc = concat!("let c = ", stringify!($t), "::new(39.9, 116.4)?.to_coordinate();")]
            #[doc = concat!("assert_eq!(c.crs(), Crs::", stringify!($t), ");")]
            /// assert_eq!((c.lat(), c.lon(), c.height()), (39.9, 116.4, None));
            /// # Ok::<(), geocoordinates::Error>(())
            /// ```
            #[must_use]
            pub fn to_coordinate(self) -> Coordinate {
                Coordinate::from_valid(self.lat, self.lon, None, $crs)
            }

            /// Extract this datum from a canonical [`Coordinate`].
            ///
            /// Any height is dropped: GCJ-02 and BD-09 are 2D obfuscations
            /// with no vertical datum.
            ///
            /// # Errors
            ///
            /// Returns [`Error::CrsMismatch`] unless `coord.crs()` matches this
            /// datum, so a datum is never silently laundered.
            ///
            /// ```
            /// use geocoordinates::{Coordinate, Crs, Error};
            #[doc = concat!("use geocoordinates::china::", stringify!($t), ";")]
            ///
            #[doc = concat!("let c = Coordinate::new(39.9, 116.4, Crs::", stringify!($t), ")?;")]
            #[doc = concat!("assert_eq!(", stringify!($t), "::try_from_coordinate(c)?.lat(), 39.9);")]
            /// let nad27 = Coordinate::new(39.9, 116.4, Crs::Nad27)?;
            /// assert!(matches!(
            #[doc = concat!("    ", stringify!($t), "::try_from_coordinate(nad27),")]
            ///     Err(Error::CrsMismatch { .. })
            /// ));
            /// # Ok::<(), geocoordinates::Error>(())
            /// ```
            pub fn try_from_coordinate(coord: Coordinate) -> Result<Self> {
                if coord.crs() == $crs {
                    Ok(Self { lat: coord.lat(), lon: coord.lon() })
                } else {
                    Err(Error::CrsMismatch { expected: $crs, found: coord.crs() })
                }
            }
        }

        impl crate::coord::sealed::Sealed for $t {}

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

        #[cfg(feature = "serde")]
        impl TryFrom<LatLonRecord> for $t {
            type Error = Error;

            fn try_from(r: LatLonRecord) -> Result<Self> {
                Self::new(r.lat, r.lon)
            }
        }
    )*};
}

// --- Bridges to the canonical `Coordinate` ---
//
// Datum newtype → `Coordinate` is exact and infallible: both are valid by
// construction, and `to_coordinate` injects the correct [`Crs`] tag and leaves
// height unset. `Coordinate` → newtype is fallible: `try_from_coordinate`
// requires the coordinate's [`Crs`] to match, so a datum is never silently
// laundered.
//
// The named inherent methods are the access path that crosses FFI; the
// `From` / `TryFrom` impls above are one-line sugar over them (see AGENTS.md).
impl_latlon!(Wgs84 => Crs::Wgs84, Gcj02 => Crs::Gcj02, Bd09 => Crs::Bd09);

// --- Refined (fixed-point) inverses, shared by GCJ-02 and BD-09 ---

/// Convergence threshold of the refined inverses, in degrees (≈ 0.1 mm): the
/// forward image of the estimate must land this close to the input.
const REFINED_EPS_DEG: f64 = 1e-9;
/// Iteration cap of the refined inverses. A converging iteration needs a
/// handful of steps; the cap only stops one that cannot converge (an input
/// with no exact preimage, such as a point in the gap the GCJ-02 box edge
/// leaves).
const REFINED_MAX_ITERS: u32 = 100;
/// Error bound (meters) a refined inverse reports when it converged. The
/// forward map is within about one percent of the identity, so a forward
/// mismatch under [`REFINED_EPS_DEG`] leaves an error far below this.
pub(crate) const REFINED_BOUND_M: f64 = 0.5;
/// Meters per radian of arc, an upper bound on every radius of curvature of
/// the WGS-84 and Krasovsky 1940 ellipsoids (the largest is the polar
/// `a/√(1 − e²)`, about 6 399 700 m on Krasovsky). Used to turn a residual in
/// degrees into a distance that never understates it.
const MAX_RADIUS_OF_CURVATURE_M: f64 = 6_400_000.0;

/// The estimate a refined inverse settled on, and the bound it earned.
pub(crate) struct Refined {
    /// Latitude of the estimate, in degrees (not yet range-checked).
    pub(crate) lat: f64,
    /// Longitude of the estimate, in degrees, wrapped into `[-180, 180)`.
    pub(crate) lon: f64,
    /// [`REFINED_BOUND_M`] if the iteration converged, otherwise that plus the
    /// distance between the estimate's forward image and the target.
    pub(crate) max_error_m: f64,
}

/// Invert `forward` at `target` by fixed-point iteration from `start`:
/// `estimate += target − forward(estimate)`, with longitude differences and
/// the estimate's longitude wrapped so the iteration works across the
/// antimeridian.
///
/// Never claims a bound it did not reach: a run that hits
/// [`REFINED_MAX_ITERS`] reports its remaining residual on top of
/// [`REFINED_BOUND_M`].
pub(crate) fn refine(
    target: (f64, f64),
    start: (f64, f64),
    forward: impl Fn(f64, f64) -> (f64, f64),
) -> Refined {
    let (mut lat, mut lon) = start;
    for _ in 0..REFINED_MAX_ITERS {
        let (d_lat, d_lon) = mismatch(target, forward(lat, lon));
        if has_converged(d_lat, d_lon) {
            return Refined {
                lat,
                lon,
                max_error_m: REFINED_BOUND_M,
            };
        }
        lat += d_lat;
        lon = wrap_lon(lon + d_lon);
    }
    let (d_lat, d_lon) = mismatch(target, forward(lat, lon));
    Refined {
        lat,
        lon,
        max_error_m: REFINED_BOUND_M + residual_m(target.0, d_lat, d_lon),
    }
}

/// `target − image` in degrees, with the longitude difference wrapped into
/// `[-180, 180)`.
fn mismatch(target: (f64, f64), image: (f64, f64)) -> (f64, f64) {
    (target.0 - image.0, wrap_lon(target.1 - image.1))
}

/// Wrap a longitude that left `[-180, 180]` into `[-180, 180)`, and pass one
/// already inside it through bit-for-bit (so `180` itself is kept).
pub(crate) fn wrap_lon(lon: f64) -> f64 {
    if (-180.0..=180.0).contains(&lon) {
        lon
    } else {
        wrap_longitude(lon)
    }
}

/// The one convergence test of the refined inverses: the forward mismatch,
/// measured as a single angular distance, is strictly below
/// [`REFINED_EPS_DEG`].
fn has_converged(d_lat: f64, d_lon: f64) -> bool {
    d_lat.hypot(d_lon) < REFINED_EPS_DEG
}

/// A mismatch of `(d_lat, d_lon)` degrees at latitude `lat`, as a ground
/// distance in meters that never understates the true one.
pub(crate) fn residual_m(lat: f64, d_lat: f64, d_lon: f64) -> f64 {
    d_lat.hypot(d_lon * lat.to_radians().cos()).to_radians() * MAX_RADIUS_OF_CURVATURE_M
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The named method and its `TryFrom` sugar must agree, and must tag the
    /// coordinate with this datum's CRS.
    #[test]
    fn to_coordinate_tags_the_datum_and_matches_its_sugar() {
        let cases = [
            (
                Wgs84::new(39.915, 116.404).unwrap().to_coordinate(),
                Crs::Wgs84,
            ),
            (
                Gcj02::new(39.915, 116.404).unwrap().to_coordinate(),
                Crs::Gcj02,
            ),
            (
                Bd09::new(39.915, 116.404).unwrap().to_coordinate(),
                Crs::Bd09,
            ),
        ];
        for (coord, expected) in cases {
            assert_eq!(coord.crs(), expected);
            assert_eq!(coord.lat(), 39.915);
            assert_eq!(coord.lon(), 116.404);
            assert!(coord.height().is_none(), "height must be left unset");
        }

        let w = Wgs84::new(1.0, 2.0).unwrap();
        assert_eq!(Coordinate::from(w), w.to_coordinate());
        let g = Gcj02::new(1.0, 2.0).unwrap();
        assert_eq!(Coordinate::from(g), g.to_coordinate());
        let b = Bd09::new(1.0, 2.0).unwrap();
        assert_eq!(Coordinate::from(b), b.to_coordinate());
    }

    /// Construction rejects a non-finite or out-of-range position, so no
    /// invalid newtype can exist.
    #[test]
    fn construction_rejects_out_of_range_positions() {
        assert!(matches!(
            Wgs84::new(91.0, 0.0),
            Err(Error::OutOfRange {
                lat: 91.0,
                lon: 0.0
            })
        ));
        assert!(Gcj02::new(0.0, 180.000_001).is_err());
        assert!(Bd09::new(0.0, f64::NAN).is_err());
        // The closed range keeps both poles and both antimeridian edges.
        for (lat, lon) in [(90.0, 180.0), (-90.0, -180.0)] {
            let w = Wgs84::new(lat, lon).unwrap();
            assert_eq!((w.lat(), w.lon()), (lat, lon));
        }
    }

    /// Newtype → `Coordinate` → newtype must round-trip exactly.
    #[test]
    fn coordinate_round_trip_is_exact() {
        let w = Wgs84::new(39.915, 116.404).unwrap();
        assert_eq!(Wgs84::try_from_coordinate(w.to_coordinate()).unwrap(), w);

        let g = Gcj02::new(39.915, 116.404).unwrap();
        assert_eq!(Gcj02::try_from_coordinate(g.to_coordinate()).unwrap(), g);

        let b = Bd09::new(39.915, 116.404).unwrap();
        assert_eq!(Bd09::try_from_coordinate(b.to_coordinate()).unwrap(), b);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_and_validates() {
        let g = Gcj02::new(39.915, 116.404).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(json, r#"{"lat":39.915,"lon":116.404}"#);
        assert_eq!(serde_json::from_str::<Gcj02>(&json).unwrap(), g);
        assert!(serde_json::from_str::<Wgs84>(r#"{"lat":91.0,"lon":0.0}"#).is_err());
        assert!(serde_json::from_str::<Gcj02>(r#"{"lat":0.0,"lon":-181.0}"#).is_err());
        assert!(serde_json::from_str::<Bd09>(r#"{"lat":-90.5,"lon":0.0}"#).is_err());
    }

    /// The single convergence test is strict, on the angular length of the
    /// mismatch, not on each component.
    #[test]
    fn convergence_is_a_strict_test_on_the_mismatch_length() {
        assert!(has_converged(0.0, 0.0));
        assert!(has_converged(0.0, REFINED_EPS_DEG * 0.999));
        assert!(!has_converged(REFINED_EPS_DEG, 0.0));
        assert!(!has_converged(0.0, -REFINED_EPS_DEG));
        // Each component alone is under the threshold, but the length is not.
        assert!(!has_converged(REFINED_EPS_DEG * 0.8, REFINED_EPS_DEG * 0.8));
    }

    /// A residual is turned into meters with the conservative radius, and the
    /// longitude component shrinks with the cosine of latitude.
    #[test]
    fn residual_never_understates_the_distance() {
        let one_deg = MAX_RADIUS_OF_CURVATURE_M * core::f64::consts::PI / 180.0;
        assert!((residual_m(0.0, 1.0, 0.0) - one_deg).abs() < 1e-6);
        assert!((residual_m(60.0, 0.0, 1.0) - one_deg / 2.0).abs() < 1e-6);
        assert!((residual_m(60.0, 0.0, -1.0) - one_deg / 2.0).abs() < 1e-6);
    }

    /// The refine driver wraps longitude across the antimeridian, and reports
    /// its residual when it cannot converge.
    #[test]
    fn refine_wraps_and_reports_an_unreached_bound() {
        // A pure eastward shift of 0.5° across the antimeridian.
        let shift = |lat: f64, lon: f64| (lat, wrap_lon(lon + 0.5));
        let r = refine((10.0, -179.8), (10.0, -179.8), shift);
        assert!((r.lon - 179.7).abs() < 1e-9, "lon {}", r.lon);
        assert_eq!(r.max_error_m, REFINED_BOUND_M);

        // A map whose image never reaches the target: the bound is the gap.
        let stuck = |lat: f64, _lon: f64| (lat, 0.0);
        let r = refine((0.0, 1.0), (0.0, 0.0), stuck);
        let expected = REFINED_BOUND_M + residual_m(0.0, 0.0, 1.0);
        assert!((r.max_error_m - expected).abs() < 1e-6, "{}", r.max_error_m);
    }

    #[test]
    fn wrap_lon_keeps_the_valid_range_bit_for_bit() {
        assert_eq!(wrap_lon(180.0), 180.0);
        assert_eq!(wrap_lon(-180.0), -180.0);
        assert_eq!(wrap_lon(-74.006), -74.006);
        assert!((wrap_lon(180.5) + 179.5).abs() < 1e-12);
        assert!((wrap_lon(-180.5) - 179.5).abs() < 1e-12);
    }

    /// A datum must never be silently laundered: every wrong-CRS extraction
    /// reports `CrsMismatch` naming both the expected and the found datum.
    #[test]
    fn extracting_the_wrong_datum_is_a_crs_mismatch() {
        let gcj = Coordinate::gcj02(39.915, 116.404).unwrap();
        assert!(matches!(
            Wgs84::try_from_coordinate(gcj),
            Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                found: Crs::Gcj02
            })
        ));

        let bd = Coordinate::bd09(39.915, 116.404).unwrap();
        assert!(matches!(
            Gcj02::try_from_coordinate(bd),
            Err(Error::CrsMismatch {
                expected: Crs::Gcj02,
                found: Crs::Bd09
            })
        ));

        let wgs = Coordinate::wgs84(39.915, 116.404).unwrap();
        assert!(matches!(
            Bd09::try_from_coordinate(wgs),
            Err(Error::CrsMismatch {
                expected: Crs::Bd09,
                found: Crs::Wgs84
            })
        ));

        // A classic datum is rejected too, not just the sibling China datums.
        let nad27 = Coordinate::new(39.915, 116.404, Crs::Nad27).unwrap();
        assert!(matches!(
            Wgs84::try_from_coordinate(nad27),
            Err(Error::CrsMismatch {
                expected: Crs::Wgs84,
                found: Crs::Nad27
            })
        ));
    }

    /// An out-of-range coordinate cannot reach extraction at all: it fails on
    /// range at construction rather than being reported as a mismatch.
    #[test]
    fn out_of_range_coordinate_fails_before_extraction() {
        assert!(matches!(
            Coordinate::new(91.0, 0.0, Crs::Gcj02).and_then(Wgs84::try_from_coordinate),
            Err(Error::OutOfRange { .. })
        ));
    }

    /// `TryFrom` is sugar and must agree with the named method exactly.
    #[test]
    fn try_from_sugar_matches_the_named_method() {
        let coord = Coordinate::wgs84(39.915, 116.404).unwrap();
        assert_eq!(
            Wgs84::try_from(coord).unwrap(),
            Wgs84::try_from_coordinate(coord).unwrap()
        );

        let mismatched = Coordinate::bd09(39.915, 116.404).unwrap();
        assert_eq!(
            Wgs84::try_from(mismatched).unwrap_err().to_string(),
            Wgs84::try_from_coordinate(mismatched)
                .unwrap_err()
                .to_string()
        );
    }
}
