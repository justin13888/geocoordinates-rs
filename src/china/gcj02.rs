//! WGS-84 ↔ GCJ-02 offset math.
//!
//! The forward offset (WGS-84 → GCJ-02) is the canonical, de-facto-standard
//! empirical polynomial + sinusoids — exact within the published algorithm. The
//! inverse (GCJ-02 → WGS-84) has no closed form:
//!
//! - [`Gcj02::to_wgs84_fast`] subtracts the offset computed at the wrong point
//!   (under 6 m away from the box edge).
//! - [`Gcj02::to_wgs84_refined`] uses fixed-point iteration
//!   (`wgs += target − wgs2gcj(wgs)`), converging to well under 0.5 m. Preferred
//!   over the 30-iteration binary search used by older ports.
//!
//! Both are infallible: inside the China box the offset moves a point by under
//! a kilometer, so no result can leave the valid latitude/longitude range.

use super::{EE, GCJ_A, Gcj02, Wgs84, out_of_china, refine};
use crate::approx::Approx;

use core::f64::consts::PI;

/// Fast-inverse error bound (meters): subtracting the offset at the GCJ point
/// rather than the unknown WGS point leaves a residual below this across the
/// box away from its edge. A 0.1° sweep of the box measures a worst case of
/// about 5.15 m, near its northern edge (55.2°N 131°E); the
/// `inverse_bounds_hold_across_the_box` test re-checks it on a 0.5° grid.
pub(crate) const FAST_BOUND_M: f64 = 6.0;

/// Raw GCJ offset polynomial in latitude, evaluated on `(lon − 105, lat − 35)`.
fn transform_lat(x: f64, y: f64) -> f64 {
    let mut ret = -100.0 + 2.0 * x + 3.0 * y + 0.2 * y * y + 0.1 * x * y + 0.2 * x.abs().sqrt();
    ret += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    ret += (20.0 * (y * PI).sin() + 40.0 * (y / 3.0 * PI).sin()) * 2.0 / 3.0;
    ret += (160.0 * (y / 12.0 * PI).sin() + 320.0 * (y * PI / 30.0).sin()) * 2.0 / 3.0;
    ret
}

/// Raw GCJ offset polynomial in longitude, evaluated on `(lon − 105, lat − 35)`.
fn transform_lon(x: f64, y: f64) -> f64 {
    let mut ret = 300.0 + x + 2.0 * y + 0.1 * x * x + 0.1 * x * y + 0.1 * x.abs().sqrt();
    ret += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    ret += (20.0 * (x * PI).sin() + 40.0 * (x / 3.0 * PI).sin()) * 2.0 / 3.0;
    ret += (150.0 * (x / 12.0 * PI).sin() + 300.0 * (x / 30.0 * PI).sin()) * 2.0 / 3.0;
    ret
}

/// Convert the raw polynomial offset at `(lat, lon)` to a `(dLat, dLon)` pair in
/// degrees, scaling by the Krasovsky radii of curvature at `lat`.
fn delta(lat: f64, lon: f64) -> (f64, f64) {
    let x = lon - 105.0;
    let y = lat - 35.0;
    let d_lat_raw = transform_lat(x, y);
    let d_lon_raw = transform_lon(x, y);

    let rad_lat = lat.to_radians();
    let magic = 1.0 - EE * rad_lat.sin().powi(2);
    let sqrt_magic = magic.sqrt();

    let d_lat = (d_lat_raw * 180.0) / ((GCJ_A * (1.0 - EE)) / (magic * sqrt_magic) * PI);
    let d_lon = (d_lon_raw * 180.0) / (GCJ_A / sqrt_magic * rad_lat.cos() * PI);
    (d_lat, d_lon)
}

/// The WGS-84 → GCJ-02 offset on raw degrees: the identity outside the China
/// box, the published offset inside it.
fn wgs_to_gcj(lat: f64, lon: f64) -> (f64, f64) {
    if out_of_china(lat, lon) {
        return (lat, lon);
    }
    let (d_lat, d_lon) = delta(lat, lon);
    (lat + d_lat, lon + d_lon)
}

impl Wgs84 {
    /// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
    ///
    /// Infallible: the offset is applied only inside the China box, where it
    /// moves a point by under a kilometer, so the result is always valid.
    ///
    /// ```
    /// use geocoordinates::Wgs84;
    ///
    /// let gcj = Wgs84::new(39.915, 116.404)?.to_gcj02();
    /// assert!((gcj.lon() - 116.410_244).abs() < 1e-6);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_gcj02(self) -> Gcj02 {
        let (lat, lon) = wgs_to_gcj(self.lat(), self.lon());
        Gcj02::from_valid(lat, lon)
    }
}

impl From<Wgs84> for Gcj02 {
    /// Exact forward offset. Thin sugar over [`Wgs84::to_gcj02`].
    fn from(wgs: Wgs84) -> Self {
        wgs.to_gcj02()
    }
}

impl Gcj02 {
    /// GCJ-02 → WGS-84, fast single-step inverse (**approximate**, within
    /// 6 m away from the China box edge).
    ///
    /// Subtracts the offset evaluated **at the GCJ point** (the wrong point —
    /// the true offset is defined at the unknown WGS point), which is exact
    /// outside China and leaves a residual of up to about 5 m inside it. The
    /// stated bound comes from a sweep of the whole box; within about a
    /// kilometer of the box edge, where the offset is discontinuous, only
    /// [`to_wgs84_refined`](Self::to_wgs84_refined) reports an honest bound.
    ///
    /// ```
    /// use geocoordinates::Gcj02;
    ///
    /// let wgs = Gcj02::new(39.916_404, 116.410_244)?.to_wgs84_fast();
    /// assert_eq!(wgs.max_error_m(), 6.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_wgs84_fast(self) -> Approx<Wgs84> {
        if out_of_china(self.lat(), self.lon()) {
            return Approx::new(Wgs84::from_valid(self.lat(), self.lon()), 0.0);
        }
        let (d_lat, d_lon) = delta(self.lat(), self.lon());
        Approx::new(
            Wgs84::from_valid(self.lat() - d_lat, self.lon() - d_lon),
            FAST_BOUND_M,
        )
    }

    /// GCJ-02 → WGS-84, refined fixed-point inverse (**approximate**).
    ///
    /// Iterates `wgs += target − wgs.to_gcj02()` until the forward image of the
    /// estimate matches `self`, converging on the WGS point whose offset lands
    /// exactly on `self`. A converged run reports `max_error_m` of 0.5 m. An
    /// input with no exact preimage (a point in the gap the discontinuous box
    /// edge leaves) never converges; it reports 0.5 m plus the distance
    /// between its result's forward image and `self`, so the bound is never
    /// one it did not reach.
    ///
    /// ```
    /// use geocoordinates::Gcj02;
    ///
    /// let wgs = Gcj02::new(39.916_404, 116.410_244)?.to_wgs84_refined();
    /// assert_eq!(wgs.max_error_m(), 0.5);
    /// assert!((wgs.lon() - 116.404).abs() < 1e-5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_wgs84_refined(self) -> Approx<Wgs84> {
        if out_of_china(self.lat(), self.lon()) {
            return Approx::new(Wgs84::from_valid(self.lat(), self.lon()), 0.0);
        }
        let target = (self.lat(), self.lon());
        let r = refine(target, target, wgs_to_gcj);
        Approx::new(Wgs84::from_valid(r.lat, r.lon), r.max_error_m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::china::REFINED_BOUND_M;
    use crate::geodesy::haversine_distance;
    use crate::test_support::{DATUM_VECTORS, assert_within_meters, china_box_grid};

    #[test]
    fn wgs84_to_gcj02_matches_references() {
        for v in DATUM_VECTORS {
            assert_within_meters(&v.wgs84().to_gcj02(), &v.gcj02(), 0.2);
        }
    }

    #[test]
    fn from_sugar_matches_the_named_method() {
        let w = Wgs84::new(39.915, 116.404).unwrap();
        assert_eq!(Gcj02::from(w), w.to_gcj02());
    }

    #[test]
    fn gcj02_to_wgs84_fast_within_its_bound() {
        for v in DATUM_VECTORS {
            let got = v.gcj02().to_wgs84_fast();
            assert_eq!(got.max_error_m(), FAST_BOUND_M);
            assert_within_meters(got.value(), &v.wgs84(), got.max_error_m());
        }
    }

    #[test]
    fn gcj02_to_wgs84_refined_within_half_meter() {
        for v in DATUM_VECTORS {
            let got = v.gcj02().to_wgs84_refined();
            assert_eq!(got.max_error_m(), REFINED_BOUND_M);
            assert_within_meters(got.value(), &v.wgs84(), 0.01);
        }
    }

    /// Our fast inverse is exactly coordtransform-rs's `gcj02_to_wgs84`
    /// (subtract the offset at the GCJ point), so it reproduces its value.
    #[test]
    fn gcj02_to_wgs84_fast_matches_coordtransform() {
        use crate::test_support::coordtransform::{GCJ_TO_WGS_FAST, INPUT};
        let got = Gcj02::new(INPUT.0, INPUT.1).unwrap().to_wgs84_fast();
        assert_within_meters(
            got.value(),
            &Wgs84::new(GCJ_TO_WGS_FAST.0, GCJ_TO_WGS_FAST.1).unwrap(),
            0.05,
        );
    }

    #[test]
    fn outside_china_is_identity() {
        let london = Wgs84::new(51.5074, -0.1278).unwrap();
        let g = london.to_gcj02();
        assert_eq!((g.lat(), g.lon()), (london.lat(), london.lon()));

        let gcj = Gcj02::new(london.lat(), london.lon()).unwrap();
        for back in [gcj.to_wgs84_refined(), gcj.to_wgs84_fast()] {
            assert_eq!((back.lat(), back.lon()), (london.lat(), london.lon()));
            assert_eq!(back.max_error_m(), 0.0);
        }
    }

    /// C3: at the box edge the fixed point does not converge, and the refined
    /// inverse must not claim a bound it did not reach. Its result's forward
    /// image must land within the reported `max_error_m` of the input, and the
    /// reported excess over the converged bound must be that residual (to the
    /// spread between the haversine sphere and the conservative radius).
    #[test]
    fn refined_bound_covers_the_residual_at_the_box_edge() {
        let target = Gcj02::new(40.0, 72.006).unwrap();
        let got = target.to_wgs84_refined();
        let image = got.value().to_gcj02();
        let residual = haversine_distance(&image, &target).unwrap().meters();
        assert!(residual > 100.0, "the edge must not converge: {residual} m");
        assert!(
            residual <= got.max_error_m(),
            "residual {residual} m exceeds the claimed {} m",
            got.max_error_m()
        );
        let excess = got.max_error_m() - REFINED_BOUND_M;
        assert!(
            excess <= residual * 1.01,
            "excess {excess} m overstates the residual {residual} m"
        );
    }

    /// Honest-bound sweep: every 0.5° inside the box, the fast and refined
    /// inverses land within their own `max_error_m` of the true WGS-84 point.
    #[test]
    fn inverse_bounds_hold_across_the_box() {
        let mut worst_fast = 0.0_f64;
        for (lat, lon) in china_box_grid() {
            let wgs = Wgs84::new(lat, lon).unwrap();
            let gcj = wgs.to_gcj02();
            for got in [gcj.to_wgs84_fast(), gcj.to_wgs84_refined()] {
                assert_within_meters(got.value(), &wgs, got.max_error_m());
            }
            let fast = haversine_distance(gcj.to_wgs84_fast().value(), &wgs).unwrap();
            worst_fast = worst_fast.max(fast.meters());
            assert_eq!(gcj.to_wgs84_refined().max_error_m(), REFINED_BOUND_M);
        }
        assert!(
            worst_fast > 0.5 * FAST_BOUND_M,
            "the fast bound is loose: {worst_fast} m"
        );
    }
}
