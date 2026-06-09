//! GCJ-02 ↔ BD-09 transforms, and WGS-84 ↔ BD-09 compositions.
//!
//! `gcj2bd` is an exact (empirical) forward nudge in polar coordinates using
//! Baidu's [`X_PI`](super::X_PI) constant. `bd2gcj` is only an approximate
//! inverse; for sub-meter round-trips it is wrapped in the same fixed-point
//! iteration as the GCJ inverse.

use super::{Bd09, Gcj02, Wgs84};
use crate::approx::Approx;

impl Gcj02 {
    /// GCJ-02 → BD-09. **Exact** forward nudge.
    #[must_use]
    pub fn to_bd09(self) -> Bd09 {
        todo!(
            "z = hypot + 0.00002*sin(y*X_PI); theta = atan2 + 0.000003*cos(x*X_PI); +0.0065/+0.006"
        )
    }
}

impl From<Gcj02> for Bd09 {
    /// Exact forward nudge.
    fn from(gcj: Gcj02) -> Self {
        gcj.to_bd09()
    }
}

impl Bd09 {
    /// BD-09 → GCJ-02, fast single-step inverse.
    #[must_use]
    pub fn to_gcj02_fast(self) -> Approx<Gcj02> {
        todo!("subtract 0.0065/0.006 then negate the nudges")
    }

    /// BD-09 → GCJ-02, refined fixed-point inverse (sub-meter).
    #[must_use]
    pub fn to_gcj02_refined(self) -> Approx<Gcj02> {
        todo!("iterate to tighten the bd2gcj inverse")
    }
}

// --- WGS-84 ↔ BD-09 compositions ---

impl Wgs84 {
    /// WGS-84 → BD-09. **Exact** composition `gcj2bd(wgs2gcj(x))`.
    #[must_use]
    pub fn to_bd09(self) -> Bd09 {
        self.to_gcj02().to_bd09()
    }
}

impl From<Wgs84> for Bd09 {
    /// Exact composition through GCJ-02.
    fn from(wgs: Wgs84) -> Self {
        wgs.to_bd09()
    }
}

impl Bd09 {
    /// BD-09 → WGS-84, refined composition through GCJ-02 (**approximate**).
    #[must_use]
    pub fn to_wgs84_refined(self) -> Approx<Wgs84> {
        todo!("compose bd.to_gcj02_refined() then gcj.to_wgs84_refined(); combine error bounds")
    }
}
