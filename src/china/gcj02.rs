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
//!
//! The forward map is not one-to-one at the box edge. A WGS-84 point just
//! inside the box is offset by up to about 0.011°, which can carry it just
//! outside, where an outside WGS-84 point maps to itself. A GCJ-02 point in
//! that band therefore has two preimages. Both inverses return the outside one
//! (the input unchanged) and widen `max_error_m` to cover the inside one, so
//! the bound holds whichever point the caller started from.

use super::{EE, GCJ_A, Gcj02, Refined, Wgs84, near_china_box, out_of_china, refine, residual_m};
use crate::approx::Approx;

use core::f64::consts::PI;

/// How far (degrees) past the China box the forward offset can carry an
/// inside point. The largest offset component over the box is about 0.0108°
/// (a 2000-step sweep of its edges and interior); this doubles it.
const EDGE_BAND_DEG: f64 = 0.02;

/// Slack (degrees) on the box when deciding whether a fast candidate stands
/// for an inside preimage: the fast candidate lies within [`FAST_BOUND_M`] of
/// the true preimage, and 1e-4° is at least 6 m of longitude up to the box's
/// northern edge.
const FAST_CANDIDATE_SLACK_DEG: f64 = 1e-4;

/// Slack (degrees, about a centimeter) on the box when deciding whether a
/// refined candidate is an inside preimage, so a preimage on the edge itself
/// is not lost to rounding. Erring outward only widens a bound.
const REFINED_SLACK_DEG: f64 = 1e-7;

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
    offset(lat, lon)
}

/// The GCJ-02 offset with no box gate: the smooth map whose fixed point an
/// inside preimage is.
fn offset(lat: f64, lon: f64) -> (f64, f64) {
    let (d_lat, d_lon) = delta(lat, lon);
    (lat + d_lat, lon + d_lon)
}

/// For a GCJ-02 point outside the box, the WGS-84 point inside the box that
/// the forward offset also carries onto it, if there is one.
fn inside_preimage(lat: f64, lon: f64) -> Option<Refined> {
    if !near_china_box(lat, lon, EDGE_BAND_DEG) {
        return None;
    }
    let r = refine((lat, lon), (lat, lon), offset);
    near_china_box(r.lat, r.lon, REFINED_SLACK_DEG).then_some(r)
}

/// Meters from `(lat, lon)` to `(other_lat, other_lon)`, never understated.
fn gap_m(lat: f64, lon: f64, other_lat: f64, other_lon: f64) -> f64 {
    residual_m(lat, other_lat - lat, other_lon - lon)
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
    /// the true offset is defined at the unknown WGS point), which leaves a
    /// residual of up to about 5 m inside the box. The stated bound comes from
    /// a sweep of the whole box; within about a kilometer of the box edge,
    /// where the offset is discontinuous, only
    /// [`to_wgs84_refined`](Self::to_wgs84_refined) reports an honest bound.
    ///
    /// Outside the box the input is returned unchanged. Far from the box that
    /// is exact (`max_error_m` 0). Within about 0.011° past its edge an inside
    /// WGS-84 point may also land on the input; when the single-step
    /// candidate for it falls inside the box, `max_error_m` is the distance to
    /// that candidate plus 6 m, so it covers both preimages.
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
        let (lat, lon) = (self.lat(), self.lon());
        if out_of_china(lat, lon) {
            let identity = Wgs84::from_valid(lat, lon);
            if !near_china_box(lat, lon, EDGE_BAND_DEG) {
                return Approx::new(identity, 0.0);
            }
            let (c_lat, c_lon) = {
                let (d_lat, d_lon) = delta(lat, lon);
                (lat - d_lat, lon - d_lon)
            };
            let bound = if near_china_box(c_lat, c_lon, FAST_CANDIDATE_SLACK_DEG) {
                gap_m(lat, lon, c_lat, c_lon) + FAST_BOUND_M
            } else {
                0.0
            };
            return Approx::new(identity, bound);
        }
        let (d_lat, d_lon) = delta(self.lat(), self.lon());
        Approx::new(
            Wgs84::from_valid(self.lat() - d_lat, self.lon() - d_lon),
            FAST_BOUND_M,
        )
    }

    /// GCJ-02 → WGS-84, refined fixed-point inverse (**approximate**).
    ///
    /// Inside the box, iterates `wgs += target − wgs.to_gcj02()` until the
    /// forward image of the estimate matches `self`. A converged run reports
    /// `max_error_m` of 0.5 m. An input with no exact preimage (a point in the
    /// gap the discontinuous box edge leaves) never converges; it reports
    /// 0.5 m plus the distance between its result's forward image and `self`,
    /// so the bound is never one it did not reach.
    ///
    /// Outside the box the input is its own preimage and is returned
    /// unchanged. Far from the box that is exact (`max_error_m` 0). Within
    /// about 0.011° past its edge, an inside WGS-84 point may also land on the
    /// input; when one does, `max_error_m` is the distance to it plus its own
    /// refined bound, so it covers whichever of the two points the caller
    /// started from.
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
        let (lat, lon) = (self.lat(), self.lon());
        if out_of_china(lat, lon) {
            let bound = inside_preimage(lat, lon)
                .map_or(0.0, |r| gap_m(lat, lon, r.lat, r.lon) + r.max_error_m);
            return Approx::new(Wgs84::from_valid(lat, lon), bound);
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

    /// D1: an inside WGS-84 point the offset carries just outside the box
    /// shares its GCJ-02 image with the outside point that maps to itself.
    /// Both inverses return the outside point and must widen the bound to
    /// cover the inside one.
    #[test]
    fn outside_input_with_an_inside_preimage_covers_both() {
        let origin = Wgs84::new(55.824_77, 130.0).unwrap();
        let gcj = origin.to_gcj02();
        assert!(out_of_china(gcj.lat(), gcj.lon()), "{gcj:?}");
        for got in [gcj.to_wgs84_fast(), gcj.to_wgs84_refined()] {
            assert_eq!((got.lat(), got.lon()), (gcj.lat(), gcj.lon()));
            let off = haversine_distance(got.value(), &origin).unwrap().meters();
            assert!(off > 600.0, "{off} m");
            assert!(
                off <= got.max_error_m(),
                "{off} m from the origin exceeds the claimed {} m",
                got.max_error_m()
            );
        }
        // The widening is the gap to the inside preimage plus that inverse's
        // own bound, not anything looser: the refined preimage is the origin,
        // and the fast candidate lies within the fast bound of it.
        let gap = residual_m(
            gcj.lat(),
            origin.lat() - gcj.lat(),
            origin.lon() - gcj.lon(),
        );
        let refined = gcj.to_wgs84_refined().max_error_m();
        assert!(
            (refined - (gap + REFINED_BOUND_M)).abs() < 0.01,
            "refined {refined} m vs gap {gap} m"
        );
        let fast = gcj.to_wgs84_fast().max_error_m();
        assert!(
            (fast - (gap + FAST_BOUND_M)).abs() <= FAST_BOUND_M,
            "fast {fast} m vs gap {gap} m"
        );
    }

    /// Sweep each box edge from just inside: wherever the offset carries a
    /// point outside, both inverses cover it; a point past every image of the
    /// box (0.015° out, beyond the 0.0108° largest offset) stays exact.
    #[test]
    fn inverse_bounds_hold_along_the_box_edge() {
        let (s, n, w, e) = (0.8293, 55.8271, 72.004, 137.8347);
        let mut carried = 0;
        for i in 0..=400_u32 {
            let t = f64::from(i) / 400.0;
            let (lat, lon) = (s + t * (n - s), w + t * (e - w));
            let inside = [
                (n - 1e-4, lon),
                (s + 1e-4, lon),
                (lat, w + 1e-4),
                (lat, e - 1e-4),
            ];
            for (la, lo) in inside {
                let wgs = Wgs84::new(la, lo).unwrap();
                let gcj = wgs.to_gcj02();
                if !out_of_china(gcj.lat(), gcj.lon()) {
                    continue;
                }
                carried += 1;
                for got in [gcj.to_wgs84_fast(), gcj.to_wgs84_refined()] {
                    assert_within_meters(got.value(), &wgs, got.max_error_m());
                }
            }
            let far = [
                (n + 0.015, lon),
                (s - 0.015, lon),
                (lat, w - 0.015),
                (lat, e + 0.015),
            ];
            for (la, lo) in far {
                let gcj = Gcj02::new(la, lo).unwrap();
                for got in [gcj.to_wgs84_fast(), gcj.to_wgs84_refined()] {
                    assert_eq!(got.max_error_m(), 0.0, "({la}, {lo})");
                }
            }
        }
        assert!(carried > 100, "only {carried} edge points left the box");
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
