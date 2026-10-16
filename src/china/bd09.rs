//! GCJ-02 ↔ BD-09 transforms, and WGS-84 ↔ BD-09 compositions.
//!
//! `gcj2bd` is an exact (empirical) forward nudge in polar coordinates using
//! Baidu's [`X_PI`](super::X_PI) constant. `bd2gcj` is only an approximate
//! inverse; for sub-meter round-trips it is wrapped in the same fixed-point
//! iteration as the GCJ inverse.
//!
//! The nudge is **not** gated to China: like every reference port, it applies
//! worldwide. It treats latitude and longitude as plane coordinates, so near
//! the antimeridian it would carry a longitude past ±180° — the result is
//! wrapped into `[-180, 180)` — and near a pole it would carry a latitude past
//! ±90°, which is reported as [`Error::OutsideDomain`]. That is why the BD-09
//! conversions stay fallible while the GCJ-02 ones are infallible.

use super::{BD_DLAT, BD_DLON, Bd09, Gcj02, Wgs84, X_PI, refine, wrap_lon};
use crate::approx::Approx;
use crate::error::{Error, Result};

/// Amplitude of BD-09's radial warp term.
const BD_R_FACTOR: f64 = 0.00002;
/// Amplitude of BD-09's angular warp term.
const BD_THETA_FACTOR: f64 = 0.000003;
/// Fast BD-09 → GCJ-02 inverse error bound (meters) away from the
/// antimeridian: the closed form undoes the translation exactly and the gentle
/// warp to within this. A worldwide 0.25° sweep measures a worst case of about
/// 0.41 m (near the poles, where the planar radius is largest) and 0.21 m
/// inside the China box.
pub(crate) const FAST_BOUND_M: f64 = 0.5;
/// Fast-inverse error bound (meters) within [`ANTIMERIDIAN_BAND_DEG`] of the
/// antimeridian. Baidu's warp treats longitude as a plane coordinate, so it
/// turns points just east and just west of ±180° by opposite angles, about
/// 120 m apart in latitude; the closed form cannot tell which side the
/// preimage is on. A 0.0001° sweep of the band measures about 120.2 m.
pub(crate) const ANTIMERIDIAN_FAST_BOUND_M: f64 = 125.0;
/// Half-width (degrees of BD-09 longitude) of the band around the
/// antimeridian where [`ANTIMERIDIAN_FAST_BOUND_M`] applies.
const ANTIMERIDIAN_BAND_DEG: f64 = 0.02;

/// The fast inverse's bound at BD-09 longitude `lon`.
fn fast_bound_m(lon: f64) -> f64 {
    if lon.abs() > 180.0 - ANTIMERIDIAN_BAND_DEG {
        ANTIMERIDIAN_FAST_BOUND_M
    } else {
        FAST_BOUND_M
    }
}

/// The GCJ-02 → BD-09 nudge on raw degrees, longitude wrapped. The latitude is
/// not range-checked.
fn gcj_to_bd(lat: f64, lon: f64) -> (f64, f64) {
    let (x, y) = (lon, lat);
    let z = (x * x + y * y).sqrt() + BD_R_FACTOR * (y * X_PI).sin();
    let theta = y.atan2(x) + BD_THETA_FACTOR * (x * X_PI).cos();
    (
        z * theta.sin() + BD_DLAT,
        wrap_lon(z * theta.cos() + BD_DLON),
    )
}

/// The closed-form BD-09 → GCJ-02 inverse on raw degrees, longitude wrapped.
/// The latitude is not range-checked.
fn bd_to_gcj(lat: f64, lon: f64) -> (f64, f64) {
    let x = lon - BD_DLON;
    let y = lat - BD_DLAT;
    let z = (x * x + y * y).sqrt() - BD_R_FACTOR * (y * X_PI).sin();
    let theta = y.atan2(x) - BD_THETA_FACTOR * (x * X_PI).cos();
    (z * theta.sin(), wrap_lon(z * theta.cos()))
}

/// Range-check a computed latitude, naming `operation` and the input point
/// when it left `[-90, 90]`.
fn within_poles(lat: f64, operation: &'static str, input: (f64, f64)) -> Result<()> {
    if (-90.0..=90.0).contains(&lat) {
        Ok(())
    } else {
        Err(Error::OutsideDomain {
            operation,
            lat: input.0,
            lon: input.1,
        })
    }
}

impl Gcj02 {
    /// GCJ-02 → BD-09. **Exact** forward nudge, applied worldwide.
    ///
    /// The result's longitude is wrapped into `[-180, 180)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OutsideDomain`] (operation `"BD-09"`) when the nudge
    /// would carry the latitude past a pole, which happens within about
    /// 0.006° of the North Pole.
    ///
    /// ```
    /// use geocoordinates::china::Gcj02;
    ///
    /// let bd = Gcj02::new(0.0, 180.0)?.try_to_bd09()?;
    /// assert!((bd.lon() + 179.9935).abs() < 1e-3); // wrapped
    /// assert!(Gcj02::new(90.0, 0.0)?.try_to_bd09().is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_bd09(self) -> Result<Bd09> {
        let (lat, lon) = gcj_to_bd(self.lat(), self.lon());
        within_poles(lat, "BD-09", (self.lat(), self.lon()))?;
        Ok(Bd09::from_valid(lat, lon))
    }
}

impl TryFrom<Gcj02> for Bd09 {
    type Error = Error;

    /// Exact forward nudge. Thin sugar over [`Gcj02::try_to_bd09`].
    fn try_from(gcj: Gcj02) -> Result<Self> {
        gcj.try_to_bd09()
    }
}

impl Bd09 {
    /// BD-09 → GCJ-02, fast single-step inverse (closed form,
    /// **approximate**).
    ///
    /// Within 0.5 m everywhere except within 0.02° of the antimeridian, where
    /// Baidu's planar warp is discontinuous and the bound is 125 m; use
    /// [`try_to_gcj02_refined`](Self::try_to_gcj02_refined) there. The
    /// result's longitude is wrapped into `[-180, 180)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OutsideDomain`] (operation `"BD-09 inverse"`) when the
    /// preimage would lie past a pole, which happens within about 0.006° of
    /// the South Pole: no GCJ-02 point nudges there.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::china::Bd09;
    ///
    /// let gcj = Bd09::new(39.922_700, 116.416_627)?.try_to_gcj02_fast()?;
    /// assert_eq!(gcj.max_error().meters(), 0.5);
    /// assert!((gcj.value().lon() - 116.410_244).abs() < 1e-5);
    /// let south_pole = Bd09::new(-90.0, 0.0)?.try_to_gcj02_fast();
    /// assert!(matches!(south_pole, Err(Error::OutsideDomain { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_gcj02_fast(self) -> Result<Approx<Gcj02>> {
        let (lat, lon) = bd_to_gcj(self.lat(), self.lon());
        within_poles(lat, "BD-09 inverse", (self.lat(), self.lon()))?;
        Ok(Approx::from_valid(
            Gcj02::from_valid(lat, lon),
            fast_bound_m(self.lon()),
        ))
    }

    /// BD-09 → GCJ-02, refined fixed-point inverse (**approximate**).
    ///
    /// Tightens [`try_to_gcj02_fast`](Self::try_to_gcj02_fast) by iterating
    /// against the exact forward [`Gcj02::try_to_bd09`], across the
    /// antimeridian too. A converged run reports `max_error` of 0.5 m; one
    /// that cannot converge reports 0.5 m plus the distance between its
    /// result's forward image and `self`. The bound is on that forward image:
    /// within about 0.01° of the antimeridian the nudge is not one-to-one, and
    /// the result is one of the preimages, not necessarily the one a caller
    /// started from.
    ///
    /// # Errors
    ///
    /// As [`try_to_gcj02_fast`](Self::try_to_gcj02_fast).
    ///
    /// ```
    /// use geocoordinates::china::Bd09;
    ///
    /// let gcj = Bd09::new(0.0, -180.0)?.try_to_gcj02_refined()?;
    /// assert!((gcj.value().lon() - 179.9935).abs() < 1e-3); // wrapped
    /// assert_eq!(gcj.max_error().meters(), 0.5);
    /// assert!(Bd09::new(-90.0, 0.0)?.try_to_gcj02_refined().is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_gcj02_refined(self) -> Result<Approx<Gcj02>> {
        let target = (self.lat(), self.lon());
        let r = refine(target, bd_to_gcj(target.0, target.1), gcj_to_bd);
        within_poles(r.lat, "BD-09 inverse", target)?;
        Ok(Approx::from_valid(
            Gcj02::from_valid(r.lat, r.lon),
            r.max_error_m,
        ))
    }
}

// --- WGS-84 ↔ BD-09 compositions ---

impl Wgs84 {
    /// WGS-84 → BD-09. **Exact** composition `gcj2bd(wgs2gcj(x))`.
    ///
    /// # Errors
    ///
    /// As [`Gcj02::try_to_bd09`].
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::china::Wgs84;
    ///
    /// let bd = Wgs84::new(39.915, 116.404)?.try_to_bd09()?;
    /// assert!((bd.lon() - 116.416_627).abs() < 1e-5);
    /// let north_pole = Wgs84::new(90.0, 0.0)?.try_to_bd09();
    /// assert!(matches!(north_pole, Err(Error::OutsideDomain { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_bd09(self) -> Result<Bd09> {
        self.to_gcj02().try_to_bd09()
    }
}

impl TryFrom<Wgs84> for Bd09 {
    type Error = Error;

    /// Exact composition through GCJ-02. Thin sugar over
    /// [`Wgs84::try_to_bd09`].
    fn try_from(wgs: Wgs84) -> Result<Self> {
        wgs.try_to_bd09()
    }
}

impl Bd09 {
    /// BD-09 → WGS-84, refined composition through GCJ-02 (**approximate**).
    ///
    /// Chains the two refined inverses and sums their `max_error` bounds.
    ///
    /// # Errors
    ///
    /// As [`try_to_gcj02_refined`](Self::try_to_gcj02_refined).
    ///
    /// ```
    /// use geocoordinates::china::Bd09;
    ///
    /// let wgs = Bd09::new(39.922_700, 116.416_627)?.try_to_wgs84_refined()?;
    /// assert_eq!(wgs.max_error().meters(), 1.0); // 0.5 m per refined leg
    /// assert!((wgs.value().lat() - 39.915).abs() < 1e-5);
    /// assert!(Bd09::new(-90.0, 0.0)?.try_to_wgs84_refined().is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn try_to_wgs84_refined(self) -> Result<Approx<Wgs84>> {
        let gcj = self.try_to_gcj02_refined()?;
        let wgs = gcj.value().to_wgs84_refined();
        Ok(Approx::from_valid(
            *wgs.value(),
            gcj.max_error().meters() + wgs.max_error().meters(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::china::REFINED_BOUND_M;
    use crate::geodesy::haversine_distance;
    use crate::test_support::{DATUM_VECTORS, assert_within_meters, china_box_grid};

    fn bd(lat: f64, lon: f64) -> Bd09 {
        Bd09::new(lat, lon).unwrap()
    }

    fn gcj(lat: f64, lon: f64) -> Gcj02 {
        Gcj02::new(lat, lon).unwrap()
    }

    #[test]
    fn wgs84_to_bd09_matches_references() {
        for v in DATUM_VECTORS {
            if let Some(expected) = v.bd09() {
                let got = v.wgs84().try_to_bd09().unwrap();
                assert_within_meters(&got, &expected, 0.2);
                assert_eq!(Bd09::try_from(v.wgs84()).unwrap(), got);
            }
        }
    }

    /// Our forward GCJ→BD and fast inverse BD→GCJ are the same closed forms as
    /// coordtransform-rs, so they reproduce its exact values.
    #[test]
    fn gcj02_to_bd09_matches_coordtransform() {
        use crate::test_support::coordtransform::{GCJ_TO_BD, INPUT};
        let got = gcj(INPUT.0, INPUT.1).try_to_bd09().unwrap();
        assert_within_meters(&got, &bd(GCJ_TO_BD.0, GCJ_TO_BD.1), 0.05);
        assert_eq!(Bd09::try_from(gcj(INPUT.0, INPUT.1)).unwrap(), got);
    }

    #[test]
    fn bd09_to_gcj02_fast_matches_coordtransform() {
        use crate::test_support::coordtransform::{BD_TO_GCJ, INPUT};
        let got = bd(INPUT.0, INPUT.1).try_to_gcj02_fast().unwrap();
        assert_within_meters(got.value(), &gcj(BD_TO_GCJ.0, BD_TO_GCJ.1), 0.05);
    }

    /// GCJ-02 → BD-09 is exact; the refined inverse recovers it to sub-meter.
    #[test]
    fn gcj02_bd09_refined_round_trip() {
        for v in DATUM_VECTORS {
            let g = v.gcj02();
            let back = g.try_to_bd09().unwrap().try_to_gcj02_refined().unwrap();
            assert_eq!(back.max_error().meters(), REFINED_BOUND_M);
            assert_within_meters(back.value(), &g, 0.01);
        }
    }

    /// C2: the worldwide BD-09 offset must wrap longitude at the antimeridian
    /// rather than return a longitude past 180°.
    #[test]
    fn forward_wraps_longitude_at_the_antimeridian() {
        let b = gcj(0.0, 180.0).try_to_bd09().unwrap();
        assert!((-180.0..180.0).contains(&b.lon()), "lon {}", b.lon());
        assert!((b.lon() + 179.9935).abs() < 1e-3, "lon {}", b.lon());
    }

    /// C2: a latitude the offset pushes past the pole is outside BD-09's
    /// domain, not a coordinate.
    #[test]
    fn forward_rejects_a_latitude_pushed_past_the_pole() {
        assert!(matches!(
            gcj(90.0, 0.0).try_to_bd09(),
            Err(Error::OutsideDomain {
                operation: "BD-09",
                lat: 90.0,
                lon: 0.0
            })
        ));
        assert!(Wgs84::new(90.0, 0.0).unwrap().try_to_bd09().is_err());
        // Just short of the pole the nudge still lands inside the range.
        assert!(gcj(89.99, 0.0).try_to_bd09().is_ok());
    }

    /// The inverse of a point near the South Pole has no GCJ-02 preimage.
    #[test]
    fn inverse_rejects_a_preimage_past_the_pole() {
        let south = bd(-90.0, 0.0);
        for got in [south.try_to_gcj02_fast(), south.try_to_gcj02_refined()] {
            assert!(matches!(
                got,
                Err(Error::OutsideDomain {
                    operation: "BD-09 inverse",
                    lat: -90.0,
                    lon: 0.0
                })
            ));
        }
        assert!(south.try_to_wgs84_refined().is_err());
        assert!(bd(-89.99, 0.0).try_to_gcj02_refined().is_ok());
    }

    /// C2: a valid BD-09 point at the antimeridian has a GCJ-02 preimage, and
    /// both inverses find it on the far side of the antimeridian.
    #[test]
    fn inverses_handle_the_antimeridian() {
        let target = bd(0.0, -180.0);
        let refined = target.try_to_gcj02_refined().unwrap();
        assert!(
            (refined.value().lon() - 179.9935).abs() < 1e-3,
            "lon {}",
            refined.value().lon()
        );
        assert_eq!(refined.max_error().meters(), REFINED_BOUND_M);
        let image = refined.value().try_to_bd09().unwrap();
        assert_within_meters(&image, &target, 0.01);

        let fast = target.try_to_gcj02_fast().unwrap();
        assert_eq!(fast.max_error().meters(), ANTIMERIDIAN_FAST_BOUND_M);
        assert_within_meters(fast.value(), refined.value(), fast.max_error().meters());
    }

    #[test]
    fn fast_bound_widens_only_inside_the_antimeridian_band() {
        assert_eq!(fast_bound_m(116.4), FAST_BOUND_M);
        assert_eq!(fast_bound_m(180.0 - ANTIMERIDIAN_BAND_DEG), FAST_BOUND_M);
        assert_eq!(fast_bound_m(179.99), ANTIMERIDIAN_FAST_BOUND_M);
        assert_eq!(fast_bound_m(-179.99), ANTIMERIDIAN_FAST_BOUND_M);
    }

    /// WGS-84 → BD-09 (exact) → refined inverse recovers the origin, and the
    /// composed error bound accumulates both refined steps.
    #[test]
    fn bd09_to_wgs84_refined_round_trip() {
        for v in DATUM_VECTORS {
            let back = v
                .wgs84()
                .try_to_bd09()
                .unwrap()
                .try_to_wgs84_refined()
                .unwrap();
            assert_within_meters(back.value(), &v.wgs84(), 0.01);
            assert_eq!(back.max_error().meters(), 2.0 * REFINED_BOUND_M);
        }
    }

    /// Honest-bound sweep: every 0.5° inside the box, the fast and refined
    /// inverses land within their own `max_error` of the true GCJ-02 point.
    #[test]
    fn inverse_bounds_hold_across_the_box() {
        let mut worst_fast = 0.0_f64;
        for (lat, lon) in china_box_grid() {
            let g = gcj(lat, lon);
            let b = g.try_to_bd09().unwrap();
            let fast = b.try_to_gcj02_fast().unwrap();
            let refined = b.try_to_gcj02_refined().unwrap();
            assert_within_meters(fast.value(), &g, fast.max_error().meters());
            assert_within_meters(refined.value(), &g, refined.max_error().meters());
            assert_eq!(refined.max_error().meters(), REFINED_BOUND_M);
            worst_fast = worst_fast.max(haversine_distance(fast.value(), &g).unwrap().meters());
        }
        // The bound is set by the worldwide worst case (about 0.41 m, near the
        // poles); inside the box the worst is about 0.21 m.
        assert!(
            worst_fast > 0.4 * FAST_BOUND_M,
            "the fast bound is loose: {worst_fast} m"
        );
    }

    /// Worldwide, every 1° and through a fine antimeridian band: the forward
    /// nudge yields a valid BD-09 point, the fast inverse lands within its own
    /// bound of the origin, and the refined inverse's forward image lands
    /// within its own bound of the input.
    #[test]
    fn worldwide_inverses_stay_valid_and_honest() {
        let band = (0..=200).flat_map(|k| {
            let d = f64::from(k) * 1e-4;
            [180.0 - d, -180.0 + d]
        });
        let lons: Vec<f64> = (-180..=180).map(f64::from).chain(band).collect();
        for i in -89..=89 {
            for &lon in &lons {
                let g = gcj(f64::from(i), lon);
                let b = g.try_to_bd09().unwrap();
                let fast = b.try_to_gcj02_fast().unwrap();
                assert_within_meters(fast.value(), &g, fast.max_error().meters());
                let back = b.try_to_gcj02_refined().unwrap();
                let image = back.value().try_to_bd09().unwrap();
                assert_within_meters(&image, &b, back.max_error().meters());
            }
        }
    }
}
