//! WGS-84 ↔ GCJ-02 offset math.
//!
//! The forward offset (WGS-84 → GCJ-02) is the canonical, de-facto-standard
//! empirical polynomial + sinusoids — exact within the published algorithm. The
//! inverse (GCJ-02 → WGS-84) has no closed form:
//!
//! - [`Gcj02::to_wgs84_fast`] subtracts the offset computed at the wrong point
//!   (under 6 m away from the box edge), and measures its bound by running
//!   the forward offset once on the result.
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
//! (the input unchanged) and widen `max_error` to cover the inside one, so
//! the bound holds whichever point the caller started from.

use super::{
    EE, GCJ_A, Gcj02, Refined, Wgs84, near_china_box, outside_china_box, refine, residual_m,
};
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
    if outside_china_box(lat, lon) {
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

/// The fast inverse's bound for the candidate `candidate` of the inside input
/// `target`, measured by running the forward map once on the candidate.
///
/// Where the offset is smooth, the candidate's forward image lands within
/// about its own error of `target` (the offset's gradient is about a percent),
/// and the box sweep holds that error under [`FAST_BOUND_M`]; a residual that
/// large is the sweep's case and keeps that bound. A larger residual means the
/// step crossed the discontinuous box edge, and the bound is [`FAST_BOUND_M`]
/// plus that residual, as the refined inverse reports an unconverged run.
fn fast_bound_m(target: (f64, f64), candidate: (f64, f64)) -> f64 {
    let (i_lat, i_lon) = wgs_to_gcj(candidate.0, candidate.1);
    let residual = gap_m(target.0, target.1, i_lat, i_lon);
    if residual <= FAST_BOUND_M {
        FAST_BOUND_M
    } else {
        FAST_BOUND_M + residual
    }
}

/// Meters from `(lat, lon)` to `(other_lat, other_lon)`, never understated.
fn gap_m(lat: f64, lon: f64, other_lat: f64, other_lon: f64) -> f64 {
    residual_m(lat, other_lat - lat, other_lon - lon)
}

impl Wgs84 {
    /// Whether this point lies outside the China box, where
    /// [`to_gcj02`](Self::to_gcj02) is the identity. The BD-09 offset is not
    /// gated by it; Baidu's nudge applies worldwide.
    ///
    /// The box is `72.004 ≤ lon ≤ 137.8347`, `0.8293 ≤ lat ≤ 55.8271` (closed),
    /// the de-facto-standard rectangle every mainstream port (eviltransform,
    /// coordtransform, …) uses, so results match theirs at the border. It is
    /// coarse: it takes in ocean and neighboring countries, and its hard edge
    /// is a **documented discontinuity** (a point just inside is offset, one
    /// just outside is not).
    ///
    /// ```
    /// use geocoordinates::china::Wgs84;
    ///
    /// assert!(!Wgs84::new(39.915, 116.404)?.is_outside_china()); // Beijing
    /// assert!(Wgs84::new(51.5, -0.12)?.is_outside_china()); // London
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn is_outside_china(&self) -> bool {
        outside_china_box(self.lat(), self.lon())
    }

    /// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
    ///
    /// Infallible: the offset is applied only inside the China box, where it
    /// moves a point by under a kilometer, so the result is always valid.
    ///
    /// ```
    /// use geocoordinates::china::Wgs84;
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
    /// Whether this point lies outside the China box (the same closed
    /// rectangle as [`Wgs84::is_outside_china`]), where both inverses return
    /// it unchanged. Within about 0.011° past the box edge an inside WGS-84
    /// point can also land here; the inverses widen `max_error` to cover it.
    ///
    /// ```
    /// use geocoordinates::china::Gcj02;
    ///
    /// assert!(!Gcj02::new(39.916_404, 116.410_244)?.is_outside_china()); // Beijing
    /// assert!(Gcj02::new(-33.86, 151.21)?.is_outside_china()); // Sydney
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn is_outside_china(&self) -> bool {
        outside_china_box(self.lat(), self.lon())
    }

    /// GCJ-02 → WGS-84, fast single-step inverse (**approximate**, within
    /// 6 m away from the China box edge).
    ///
    /// Subtracts the offset evaluated **at the GCJ point** (the wrong point —
    /// the true offset is defined at the unknown WGS point), which leaves an
    /// error of up to about 5 m inside the box. The candidate is then run
    /// through the forward offset once, and the bound is measured from how far
    /// its image lands from `self`: within 6 m (everywhere the offset is
    /// smooth) `max_error` is 6 m, the worst case a sweep of the box measures.
    /// Within about a kilometer of the box edge, where the offset is
    /// discontinuous, the step can cross the edge; `max_error` is then 6 m
    /// plus that residual, so it is never a bound the step did not reach.
    /// [`to_wgs84_refined`](Self::to_wgs84_refined) is tighter there.
    ///
    /// Outside the box the input is returned unchanged. Far from the box that
    /// is exact (`max_error` 0). Within about 0.011° past its edge an inside
    /// WGS-84 point may also land on the input; when the single-step
    /// candidate for it falls inside the box, `max_error` is the distance to
    /// that candidate plus 6 m, so it covers both preimages.
    ///
    /// ```
    /// use geocoordinates::china::Gcj02;
    ///
    /// let wgs = Gcj02::new(39.916_404, 116.410_244)?.to_wgs84_fast();
    /// assert_eq!(wgs.max_error().meters(), 6.0);
    ///
    /// // At the box's west edge the step crosses it: the bound says so.
    /// let edge = Gcj02::new(40.0, 72.006)?.to_wgs84_fast();
    /// assert!(edge.max_error().meters() > 100.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_wgs84_fast(self) -> Approx<Wgs84> {
        let (lat, lon) = (self.lat(), self.lon());
        if outside_china_box(lat, lon) {
            let identity = Wgs84::from_valid(lat, lon);
            if !near_china_box(lat, lon, EDGE_BAND_DEG) {
                return Approx::from_valid(identity, 0.0);
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
            return Approx::from_valid(identity, bound);
        }
        let (d_lat, d_lon) = delta(lat, lon);
        let (c_lat, c_lon) = (lat - d_lat, lon - d_lon);
        Approx::from_valid(
            Wgs84::from_valid(c_lat, c_lon),
            fast_bound_m((lat, lon), (c_lat, c_lon)),
        )
    }

    /// GCJ-02 → WGS-84, refined fixed-point inverse (**approximate**).
    ///
    /// Inside the box, iterates `wgs += target − wgs.to_gcj02()` until the
    /// forward image of the estimate matches `self`. A converged run reports
    /// `max_error` of 0.5 m. An input with no exact preimage (a point in the
    /// gap the discontinuous box edge leaves) never converges; it reports
    /// 0.5 m plus the distance between its result's forward image and `self`,
    /// so the bound is never one it did not reach.
    ///
    /// Outside the box the input is its own preimage and is returned
    /// unchanged. Far from the box that is exact (`max_error` 0). Within
    /// about 0.011° past its edge, an inside WGS-84 point may also land on the
    /// input; when one does, `max_error` is the distance to it plus its own
    /// refined bound, so it covers whichever of the two points the caller
    /// started from.
    ///
    /// ```
    /// use geocoordinates::china::Gcj02;
    ///
    /// let wgs = Gcj02::new(39.916_404, 116.410_244)?.to_wgs84_refined();
    /// assert_eq!(wgs.max_error().meters(), 0.5);
    /// assert!((wgs.value().lon() - 116.404).abs() < 1e-5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_wgs84_refined(self) -> Approx<Wgs84> {
        let (lat, lon) = (self.lat(), self.lon());
        if outside_china_box(lat, lon) {
            let bound = inside_preimage(lat, lon)
                .map_or(0.0, |r| gap_m(lat, lon, r.lat, r.lon) + r.max_error_m);
            return Approx::from_valid(Wgs84::from_valid(lat, lon), bound);
        }
        let target = (self.lat(), self.lon());
        let r = refine(target, target, wgs_to_gcj);
        Approx::from_valid(Wgs84::from_valid(r.lat, r.lon), r.max_error_m)
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
            assert_eq!(got.max_error().meters(), FAST_BOUND_M);
            assert_within_meters(got.value(), &v.wgs84(), got.max_error().meters());
        }
    }

    #[test]
    fn gcj02_to_wgs84_refined_within_half_meter() {
        for v in DATUM_VECTORS {
            let got = v.gcj02().to_wgs84_refined();
            assert_eq!(got.max_error().meters(), REFINED_BOUND_M);
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
            assert_eq!(
                (back.value().lat(), back.value().lon()),
                (london.lat(), london.lon())
            );
            assert_eq!(back.max_error().meters(), 0.0);
        }
    }

    /// C3: at the box edge the fixed point does not converge, and the refined
    /// inverse must not claim a bound it did not reach. Its result's forward
    /// image must land within the reported `max_error` of the input, and the
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
            residual <= got.max_error().meters(),
            "residual {residual} m exceeds the claimed {} m",
            got.max_error().meters()
        );
        let excess = got.max_error().meters() - REFINED_BOUND_M;
        assert!(
            excess <= residual * 1.01,
            "excess {excess} m overstates the residual {residual} m"
        );
    }

    /// C3 (#172): at the west edge the fast step carries the input across the
    /// box edge (to about 72.002°E, where the offset is the identity), and its
    /// forward image lands about 340 m from the input. The bound must cover
    /// that residual rather than claim the interior's 6 m.
    #[test]
    fn fast_bound_covers_the_residual_at_the_box_edge() {
        let target = Gcj02::new(40.0, 72.006).unwrap();
        let got = target.to_wgs84_fast();
        assert!(got.value().is_outside_china());
        let image = got.value().to_gcj02();
        let residual = haversine_distance(&image, &target).unwrap().meters();
        assert!(residual > 300.0, "{residual} m");
        let bound = got.max_error().meters();
        assert!(residual <= bound, "residual {residual} m exceeds {bound} m");
        let excess = bound - FAST_BOUND_M;
        assert!(
            excess <= residual * 1.01,
            "excess {excess} m overstates the residual {residual} m"
        );
        // The refined inverse cannot do better than its own residual here;
        // the fast bound covers the distance to its result too.
        let refined = target.to_wgs84_refined();
        assert_within_meters(
            got.value(),
            refined.value(),
            bound + refined.max_error().meters(),
        );
    }

    /// Sweep inside inputs within 0.02° of each box edge, every 0.0005°: the
    /// fast inverse's forward image always lands within its bound of the
    /// input, and an input that is the image of an inside point gets that
    /// point back within the bound.
    #[test]
    fn fast_bound_covers_its_residual_near_every_edge() {
        let (s, n, w, e) = (0.8293, 55.8271, 72.004, 137.8347);
        let mut widened = 0;
        for i in 0..=40_u32 {
            let d = f64::from(i) * 0.0005;
            for j in 0..=200_u32 {
                let t = f64::from(j) / 200.0;
                let (lat, lon) = (s + t * (n - s), w + t * (e - w));
                for (la, lo) in [(n - d, lon), (s + d, lon), (lat, w + d), (lat, e - d)] {
                    let gcj = Gcj02::new(la, lo).unwrap();
                    let got = gcj.to_wgs84_fast();
                    let bound = got.max_error().meters();
                    let image = got.value().to_gcj02();
                    assert_within_meters(&image, &gcj, bound);
                    if bound > FAST_BOUND_M {
                        widened += 1;
                    }
                    let origin = Wgs84::new(la, lo).unwrap();
                    let carried = origin.to_gcj02();
                    if !carried.is_outside_china() {
                        let back = carried.to_wgs84_fast();
                        assert_within_meters(back.value(), &origin, back.max_error().meters());
                    }
                }
            }
        }
        assert!(widened > 100, "only {widened} edge inputs crossed the edge");
    }

    /// D1: an inside WGS-84 point the offset carries just outside the box
    /// shares its GCJ-02 image with the outside point that maps to itself.
    /// Both inverses return the outside point and must widen the bound to
    /// cover the inside one.
    #[test]
    fn outside_input_with_an_inside_preimage_covers_both() {
        let origin = Wgs84::new(55.824_77, 130.0).unwrap();
        let gcj = origin.to_gcj02();
        assert!(gcj.is_outside_china(), "{gcj:?}");
        for got in [gcj.to_wgs84_fast(), gcj.to_wgs84_refined()] {
            assert_eq!(
                (got.value().lat(), got.value().lon()),
                (gcj.lat(), gcj.lon())
            );
            let off = haversine_distance(got.value(), &origin).unwrap().meters();
            assert!(off > 600.0, "{off} m");
            assert!(
                off <= got.max_error().meters(),
                "{off} m from the origin exceeds the claimed {} m",
                got.max_error().meters()
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
        let refined = gcj.to_wgs84_refined().max_error().meters();
        assert!(
            (refined - (gap + REFINED_BOUND_M)).abs() < 0.01,
            "refined {refined} m vs gap {gap} m"
        );
        let fast = gcj.to_wgs84_fast().max_error().meters();
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
                if !gcj.is_outside_china() {
                    continue;
                }
                carried += 1;
                for got in [gcj.to_wgs84_fast(), gcj.to_wgs84_refined()] {
                    assert_within_meters(got.value(), &wgs, got.max_error().meters());
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
                    assert_eq!(got.max_error().meters(), 0.0, "({la}, {lo})");
                }
            }
        }
        assert!(carried > 100, "only {carried} edge points left the box");
    }

    /// Honest-bound sweep: every 0.5° inside the box, the fast and refined
    /// inverses land within their own `max_error` of the true WGS-84 point.
    #[test]
    fn inverse_bounds_hold_across_the_box() {
        let mut worst_fast = 0.0_f64;
        for (lat, lon) in china_box_grid() {
            let wgs = Wgs84::new(lat, lon).unwrap();
            let gcj = wgs.to_gcj02();
            for got in [gcj.to_wgs84_fast(), gcj.to_wgs84_refined()] {
                assert_within_meters(got.value(), &wgs, got.max_error().meters());
            }
            let fast = haversine_distance(gcj.to_wgs84_fast().value(), &wgs).unwrap();
            worst_fast = worst_fast.max(fast.meters());
            // Away from the edge the measured residual stays within the sweep's bound.
            if near_china_box(lat, lon, -0.1) {
                assert_eq!(gcj.to_wgs84_fast().max_error().meters(), FAST_BOUND_M);
            }
            assert_eq!(gcj.to_wgs84_refined().max_error().meters(), REFINED_BOUND_M);
        }
        assert!(
            worst_fast > 0.5 * FAST_BOUND_M,
            "the fast bound is loose: {worst_fast} m"
        );
    }
}
