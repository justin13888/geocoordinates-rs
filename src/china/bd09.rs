//! GCJ-02 ↔ BD-09 transforms, and WGS-84 ↔ BD-09 compositions.
//!
//! `gcj2bd` is an exact (empirical) forward nudge in polar coordinates using
//! Baidu's [`X_PI`](super::X_PI) constant. `bd2gcj` is only an approximate
//! inverse; for sub-meter round-trips it is wrapped in the same fixed-point
//! iteration as the GCJ inverse.

use super::{BD_DLAT, BD_DLON, Bd09, Gcj02, Wgs84, X_PI};
use crate::approx::Approx;
use crate::error::Result;

/// Amplitude of BD-09's radial warp term.
const BD_R_FACTOR: f64 = 0.00002;
/// Amplitude of BD-09's angular warp term.
const BD_THETA_FACTOR: f64 = 0.000003;
/// Fast BD-09 → GCJ-02 inverse error bound (meters): the closed-form is already
/// decimeter-accurate because Baidu's warp is gentle.
const FAST_BOUND_M: f64 = 1.0;
/// Refined BD-09 → GCJ-02 error bound (meters) after fixed-point iteration.
const REFINED_BOUND_M: f64 = 0.5;
/// Convergence threshold for the refined inverse, in degrees.
const REFINED_EPS_DEG: f64 = 1e-9;
/// Iteration cap for the refined inverse.
const REFINED_MAX_ITERS: u32 = 100;

impl Gcj02 {
    /// GCJ-02 → BD-09. **Exact** forward nudge.
    pub fn try_to_bd09(self) -> Result<Bd09> {
        self.validate()?;
        let x = self.lon;
        let y = self.lat;
        let z = (x * x + y * y).sqrt() + BD_R_FACTOR * (y * X_PI).sin();
        let theta = y.atan2(x) + BD_THETA_FACTOR * (x * X_PI).cos();
        Ok(Bd09::new(
            z * theta.sin() + BD_DLAT,
            z * theta.cos() + BD_DLON,
        ))
    }
}

impl TryFrom<Gcj02> for Bd09 {
    type Error = crate::Error;

    /// Exact forward nudge.
    fn try_from(gcj: Gcj02) -> Result<Self> {
        gcj.try_to_bd09()
    }
}

impl Bd09 {
    /// BD-09 → GCJ-02, fast single-step inverse (closed-form, decimeter-level).
    pub fn try_to_gcj02_fast(self) -> Result<Approx<Gcj02>> {
        self.validate()?;
        let x = self.lon - BD_DLON;
        let y = self.lat - BD_DLAT;
        let z = (x * x + y * y).sqrt() - BD_R_FACTOR * (y * X_PI).sin();
        let theta = y.atan2(x) - BD_THETA_FACTOR * (x * X_PI).cos();
        Ok(Approx::new(
            Gcj02::new(z * theta.sin(), z * theta.cos()),
            FAST_BOUND_M,
        ))
    }

    /// BD-09 → GCJ-02, refined fixed-point inverse (sub-meter).
    ///
    /// Tightens [`to_gcj02_fast`](Self::to_gcj02_fast) by iterating against the
    /// exact forward [`Gcj02::to_bd09`].
    pub fn try_to_gcj02_refined(self) -> Result<Approx<Gcj02>> {
        self.validate()?;
        let mut gcj = self.try_to_gcj02_fast()?.into_inner();
        for _ in 0..REFINED_MAX_ITERS {
            let cur = gcj.try_to_bd09()?;
            let d_lat = self.lat - cur.lat;
            let d_lon = self.lon - cur.lon;
            gcj = Gcj02::new(gcj.lat + d_lat, gcj.lon + d_lon);
            if d_lat.abs() < REFINED_EPS_DEG && d_lon.abs() < REFINED_EPS_DEG {
                break;
            }
        }
        Ok(Approx::new(gcj, REFINED_BOUND_M))
    }
}

// --- WGS-84 ↔ BD-09 compositions ---

impl Wgs84 {
    /// WGS-84 → BD-09. **Exact** composition `gcj2bd(wgs2gcj(x))`.
    pub fn try_to_bd09(self) -> Result<Bd09> {
        self.try_to_gcj02()?.try_to_bd09()
    }
}

impl TryFrom<Wgs84> for Bd09 {
    type Error = crate::Error;

    /// Exact composition through GCJ-02.
    fn try_from(wgs: Wgs84) -> Result<Self> {
        wgs.try_to_bd09()
    }
}

impl Bd09 {
    /// BD-09 → WGS-84, refined composition through GCJ-02 (**approximate**).
    ///
    /// Chains the two refined inverses, summing their error bounds via
    /// `Approx::and_then`.
    pub fn try_to_wgs84_refined(self) -> Result<Approx<Wgs84>> {
        let gcj = self.try_to_gcj02_refined()?;
        let max_error_m = gcj.max_error_m();
        let wgs = gcj.into_inner().try_to_wgs84_refined()?;
        Ok(Approx::new(
            wgs.into_inner(),
            max_error_m + wgs.max_error_m(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{DATUM_VECTORS, assert_within_meters};

    #[test]
    fn wgs84_to_bd09_matches_references() {
        for v in DATUM_VECTORS {
            if let Some(bd) = v.bd09() {
                assert_within_meters(&v.wgs84().try_to_bd09().unwrap(), &bd, 0.2);
            }
        }
    }

    /// Our forward GCJ→BD and fast inverse BD→GCJ are the same closed forms as
    /// coordtransform-rs, so they reproduce its exact values.
    #[test]
    fn gcj02_to_bd09_matches_coordtransform() {
        use crate::test_support::coordtransform::{GCJ_TO_BD, INPUT};
        let got = Gcj02::new(INPUT.0, INPUT.1).try_to_bd09().unwrap();
        assert_within_meters(&got, &Bd09::new(GCJ_TO_BD.0, GCJ_TO_BD.1), 0.05);
    }

    #[test]
    fn bd09_to_gcj02_fast_matches_coordtransform() {
        use crate::test_support::coordtransform::{BD_TO_GCJ, INPUT};
        let got = Bd09::new(INPUT.0, INPUT.1).try_to_gcj02_fast().unwrap();
        assert_within_meters(got.value(), &Gcj02::new(BD_TO_GCJ.0, BD_TO_GCJ.1), 0.05);
    }

    /// GCJ-02 → BD-09 is exact; the refined inverse recovers it to sub-meter.
    #[test]
    fn gcj02_bd09_refined_round_trip() {
        for v in DATUM_VECTORS {
            let gcj = v.gcj02();
            let back = gcj.try_to_bd09().unwrap().try_to_gcj02_refined().unwrap();
            assert_within_meters(back.value(), &gcj, 0.5);
        }
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
            assert_within_meters(back.value(), &v.wgs84(), 0.5);
            assert!((back.max_error_m() - 1.0).abs() < 1e-9);
        }
    }
}
