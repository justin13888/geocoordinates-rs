//! Central runtime conversion dispatch over [`Crs`].
//!
//! This is the *runtime* path: it dispatches on a [`Crs`] tag and returns an
//! [`Approx`] because some target systems (GCJ-02/BD-09 inverses) are only
//! reachable approximately. When the source and target are known at compile
//! time, prefer the typed newtype conversions (e.g. [`crate::Wgs84::to_gcj02`]),
//! which return bare types where no inversion is involved.
//!
//! Every modeled [`Crs`] pair has a route, so there is no reachability query:
//! the only failures are the domain edges [`convert`] documents.
//!
//! Conversions are per-coordinate; batch / vectorized conversion is left to the
//! caller (iterate, or parallelize with e.g. `rayon`) rather than offered as a
//! dedicated API.

use crate::approx::Approx;
use crate::china::{Bd09, Gcj02, Wgs84};
use crate::coord::{Coordinate, Crs, LatLon};
use crate::error::Result;
use crate::geodesy::datum::{DatumTransform, HubRoute};

/// Convert `coord` from its own [`Crs`] to `to`.
///
/// Routes through WGS-84 as the hub: China systems use the typed
/// [`china`](crate::china) conversions; classic datums (NAD27, Tokyo,
/// Pulkovo-1942) use the 7-parameter Helmert transforms from
/// [`geodesy::datum`](crate::geodesy::datum). The full EPSG/national-grid long
/// tail is delegated to the deferred `proj` feature (see
/// [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)).
///
/// The result is wrapped in [`Approx`] because several legs are approximate.
/// [`Approx::max_error_m`] is the sum of the legs' error estimates:
///
/// - the iterative GCJ-02 / BD-09 inverses report their own bound;
/// - the forward China offsets are exact and report `0.0`;
/// - each Helmert datum shift (NAD27, Tokyo, Pulkovo-1942, in either
///   direction) reports its parameters' published accuracy,
///   [`DatumTransform::accuracy_m`](crate::geodesy::DatumTransform::accuracy_m):
///   about 9.27 m for NAD27 and 28.72 m for Tokyo (the root-sum-square of
///   NIMA TR8350.2's per-axis figures, a statistical estimate rather than a
///   hard maximum), and `f64::INFINITY` for Pulkovo-1942, for which no
///   estimate is published (serialized as `null` under the `serde` feature;
///   see [`Approx`]). The computation itself is exact to round-off, and
///   the catalog is translation-only, so reversing a shift adds nothing.
///
/// The GCJ-02 legs never fail. A refined inverse that cannot converge (an
/// input in the gap the GCJ-02 box edge leaves) reports its residual in the
/// bound rather than failing.
///
/// # Errors
/// Every modeled [`Crs`] pair has a route and `coord` is valid by construction,
/// so this fails only where a leg's domain ends:
///
/// - [`Error::OutsideDomain`](crate::Error::OutsideDomain) (operation
///   `"BD-09"` or `"BD-09 inverse"`) when a BD-09 leg would carry a latitude
///   past a pole, within about 0.006° of one;
/// - [`Error::InvalidValue`](crate::Error::InvalidValue) (field `height`) when
///   `coord` carries an orthometric height and a leg is a Helmert datum shift,
///   which needs an ellipsoidal height (no geoid model is available).
///
/// ```
/// use geocoordinates::{Coordinate, Crs, convert::convert};
///
/// let gcj = Coordinate::gcj02(39.916_404, 116.410_244)?;
/// let wgs = convert(gcj, Crs::Wgs84)?;
/// assert_eq!(wgs.crs(), Crs::Wgs84);
/// assert_eq!(wgs.max_error_m(), 0.5); // the refined GCJ-02 inverse
///
/// // The BD-09 nudge would carry a point at the North Pole past it.
/// let pole = Coordinate::gcj02(90.0, 0.0)?;
/// assert!(matches!(
///     convert(pole, Crs::Bd09),
///     Err(geocoordinates::Error::OutsideDomain { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn convert(coord: Coordinate, to: Crs) -> Result<Approx<Coordinate>> {
    if coord.crs() == to {
        return Ok(Approx::new(coord, 0.0));
    }
    // GCJ-02 → BD-09 is an exact direct transform. Routing it through the WGS-84
    // hub would taint the result with the GCJ-02 inverse error, so take the
    // direct path and keep it exact.
    if coord.crs() == Crs::Gcj02 && to == Crs::Bd09 {
        let b = Gcj02::from_valid(coord.lat(), coord.lon()).try_to_bd09()?;
        return Ok(Approx::new(carry(coord, b), 0.0));
    }
    // Everything else routes through WGS-84, accumulating the per-leg bounds.
    let hub = to_wgs84(coord)?;
    let hub_error = hub.max_error_m();
    let target = from_wgs84(hub.into_inner(), to)?;
    Ok(Approx::new(
        target.into_inner(),
        hub_error + target.max_error_m(),
    ))
}

/// Convert `coord` to a WGS-84 [`Coordinate`], carrying the per-leg error bound.
fn to_wgs84(coord: Coordinate) -> Result<Approx<Coordinate>> {
    Ok(match HubRoute::of(coord.crs()) {
        HubRoute::Wgs84 => Approx::new(coord, 0.0),
        HubRoute::Gcj02 => {
            let w = Gcj02::from_valid(coord.lat(), coord.lon()).to_wgs84_refined();
            Approx::new(carry(coord, *w.value()), w.max_error_m())
        }
        HubRoute::Bd09 => {
            let w = Bd09::from_valid(coord.lat(), coord.lon()).try_to_wgs84_refined()?;
            Approx::new(carry(coord, *w.value()), w.max_error_m())
        }
        HubRoute::Helmert(dt) => Approx::new(dt.apply(coord)?, helmert_bound_m(dt)),
    })
}

/// Convert a WGS-84 [`Coordinate`] to `to`, carrying the per-leg error bound.
fn from_wgs84(wgs: Coordinate, to: Crs) -> Result<Approx<Coordinate>> {
    let w = Wgs84::from_valid(wgs.lat(), wgs.lon());
    Ok(match HubRoute::of(to) {
        HubRoute::Wgs84 => Approx::new(wgs, 0.0),
        HubRoute::Gcj02 => Approx::new(carry(wgs, w.to_gcj02()), 0.0),
        HubRoute::Bd09 => Approx::new(carry(wgs, w.try_to_bd09()?), 0.0),
        HubRoute::Helmert(dt) => {
            Approx::new(dt.inverse_first_order().apply(wgs)?, helmert_bound_m(dt))
        }
    })
}

/// A Helmert leg's error estimate: its parameters' published accuracy, or
/// infinity when none is published.
fn helmert_bound_m(dt: DatumTransform) -> f64 {
    dt.accuracy_m().unwrap_or(f64::INFINITY)
}

/// Rebuild `orig` at the datum position `p`, tagged with `p`'s CRS and
/// carrying `orig`'s height (the China lat/lon transforms are 2-D; height
/// passes through unchanged). Both are valid by construction.
fn carry(orig: Coordinate, p: impl LatLon) -> Coordinate {
    Coordinate::from_valid(p.lat(), p.lon(), orig.height(), p.crs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use crate::coord::Height;
    use crate::geodesy::haversine_distance;
    use crate::test_support::assert_close;

    // Reference triple from `test_support` (a single WGS-84 point and its exact
    // GCJ-02 / BD-09 images): (lat, lon).
    const WGS: (f64, f64) = (39.915, 116.404);
    const GCJ: (f64, f64) = (39.916_404_281_501_64, 116.410_244_499_169_38);
    const BD: (f64, f64) = (39.922_699_552_216_216, 116.416_627_243_787_33);

    #[test]
    fn same_crs_is_exact_identity() {
        let c = Coordinate::wgs84(WGS.0, WGS.1).unwrap();
        let r = convert(c, Crs::Wgs84).unwrap();
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs(), Crs::Wgs84);
        assert_close(r.lat(), WGS.0, 1e-12);
        assert_close(r.lon(), WGS.1, 1e-12);
    }

    #[test]
    fn wgs84_to_gcj02_is_exact() {
        let r = convert(Coordinate::wgs84(WGS.0, WGS.1).unwrap(), Crs::Gcj02).unwrap();
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs(), Crs::Gcj02);
        // Exactly the typed forward conversion.
        let g = Wgs84::new(WGS.0, WGS.1).unwrap().to_gcj02();
        assert_close(r.lat(), g.lat(), 1e-12);
        assert_close(r.lon(), g.lon(), 1e-12);
        assert_close(r.lat(), GCJ.0, 1e-9);
    }

    #[test]
    fn gcj02_to_wgs84_is_approximate() {
        let r = convert(Coordinate::gcj02(GCJ.0, GCJ.1).unwrap(), Crs::Wgs84).unwrap();
        // Bound is exactly the delegated refined-inverse bound (not re-derived).
        let direct = Gcj02::new(GCJ.0, GCJ.1).unwrap().to_wgs84_refined();
        assert!(r.max_error_m() > 0.0);
        assert_close(r.max_error_m(), direct.max_error_m(), 1e-12);
        assert_close(r.lat(), direct.lat(), 1e-12);
        // Recovers the original WGS-84 within the stated bound.
        let bound = r.max_error_m();
        crate::test_support::assert_within_meters(
            &r.into_inner(),
            &Coordinate::wgs84(WGS.0, WGS.1).unwrap(),
            bound,
        );
    }

    #[test]
    fn gcj02_to_bd09_stays_exact_via_direct_path() {
        let r = convert(Coordinate::gcj02(GCJ.0, GCJ.1).unwrap(), Crs::Bd09).unwrap();
        // The direct GCJ→BD path keeps it exact — no WGS-84 inverse error leaks in.
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs(), Crs::Bd09);
        let b = Gcj02::new(GCJ.0, GCJ.1).unwrap().try_to_bd09().unwrap();
        assert_close(r.lat(), b.lat(), 1e-12);
        assert_close(r.lon(), b.lon(), 1e-12);
        assert_close(r.lat(), BD.0, 1e-9);
    }

    #[test]
    fn bd09_to_wgs84_is_approximate() {
        let r = convert(Coordinate::bd09(BD.0, BD.1).unwrap(), Crs::Wgs84).unwrap();
        assert!(r.max_error_m() > 0.0);
        let bound = r.max_error_m();
        crate::test_support::assert_within_meters(
            &r.into_inner(),
            &Coordinate::wgs84(WGS.0, WGS.1).unwrap(),
            bound,
        );
    }

    /// NAD27 test point at 40°N 100°W with a zero ellipsoidal height.
    fn nad27_point() -> Coordinate {
        Coordinate::new(40.0, -100.0, Crs::Nad27)
            .unwrap()
            .with_height(Height::Ellipsoidal(0.0))
            .unwrap()
    }

    /// √(5² + 5² + 6²): TR8350.2's NAD27 CONUS mean-shift accuracy.
    const NAD27_ACCURACY_M: f64 = 9.273_618_495_495_704;
    /// √(20² + 5² + 20²): TR8350.2's Tokyo mean-shift accuracy.
    const TOKYO_ACCURACY_M: f64 = 28.722_813_232_690_143;

    #[test]
    fn nad27_to_wgs84_reports_the_published_accuracy() {
        // Same independent reference as the datum milestone. The bound is the
        // parameters' published accuracy, not zero.
        let nad27 = nad27_point();
        let r = convert(nad27, Crs::Wgs84).unwrap();
        assert_eq!(r.max_error_m(), NAD27_ACCURACY_M);
        assert_eq!(r.crs(), Crs::Wgs84);
        assert_close(r.lat(), 40.000_009_482_759, 1e-8);
        assert_close(r.lon(), -100.000_417_622_218_8, 1e-8);
    }

    #[test]
    fn classic_datum_round_trips() {
        // The NAD27 catalog shift is translation-only, so the reverse leg's
        // negated parameters undo the forward leg and a round trip recovers the
        // input to round-off. Each leg still reports the parameters' accuracy.
        let nad27 = nad27_point();
        let w = convert(nad27, Crs::Wgs84).unwrap().into_inner();
        let back = convert(w, Crs::Nad27).unwrap();
        assert_eq!(back.max_error_m(), NAD27_ACCURACY_M);
        assert_eq!(back.crs(), Crs::Nad27);
        assert_close(back.lat(), 40.0, 1e-12);
        assert_close(back.lon(), -100.0, 1e-12);
    }

    #[test]
    fn datum_to_datum_sums_both_helmert_legs() {
        let r = convert(nad27_point(), Crs::Tokyo).unwrap();
        assert_eq!(r.crs(), Crs::Tokyo);
        assert_eq!(r.max_error_m(), NAD27_ACCURACY_M + TOKYO_ACCURACY_M);
    }

    /// TR8350.2 publishes no error estimate for the Pulkovo-1942 (Russia)
    /// shift, so any route through it has no finite bound.
    #[test]
    fn pulkovo_legs_have_no_finite_bound() {
        let pulkovo = Coordinate::new(55.75, 37.62, Crs::Pulkovo42).unwrap();
        assert_eq!(
            convert(pulkovo, Crs::Wgs84).unwrap().max_error_m(),
            f64::INFINITY
        );
        let wgs = Coordinate::wgs84(55.75, 37.62).unwrap();
        assert_eq!(
            convert(wgs, Crs::Pulkovo42).unwrap().max_error_m(),
            f64::INFINITY
        );
    }

    /// An unbounded Pulkovo-1942 result survives a JSON round trip: JSON has
    /// no infinity, so the bound travels as `null` and reads back as infinity.
    #[cfg(feature = "serde")]
    #[test]
    fn pulkovo_result_round_trips_through_json() {
        let wgs = Coordinate::wgs84(55.75, 37.62).unwrap();
        let r = convert(wgs, Crs::Pulkovo42).unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let back: Approx<Coordinate> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
        assert_eq!(back.max_error_m(), f64::INFINITY);
    }

    #[test]
    fn cross_datum_to_china_carries_the_helmert_accuracy() {
        // NAD27 → WGS-84 (Helmert) → GCJ-02 (typed forward, exact): the
        // composed bound is the Helmert leg's published accuracy alone.
        let nad27 = Coordinate::new(40.0, -100.0, Crs::Nad27).unwrap();
        let r = convert(nad27, Crs::Gcj02).unwrap();
        assert_eq!(r.max_error_m(), NAD27_ACCURACY_M);
        assert_eq!(r.crs(), Crs::Gcj02);
        // Equals the explicit two-step route.
        let dt = DatumTransform::catalog_to_wgs84(Crs::Nad27).unwrap();
        let w = dt.apply(nad27).unwrap();
        let g = Wgs84::new(w.lat(), w.lon()).unwrap().to_gcj02();
        assert_close(r.lat(), g.lat(), 1e-12);
        assert_close(r.lon(), g.lon(), 1e-12);
    }

    #[test]
    fn cross_china_to_datum_accumulates_the_bound() {
        // BD-09 → WGS-84 (approx) → Tokyo (Helmert): the composed bound is the
        // BD-09 inverse bound plus the Tokyo shift's published accuracy.
        let r = convert(Coordinate::bd09(BD.0, BD.1).unwrap(), Crs::Tokyo).unwrap();
        let w = Bd09::new(BD.0, BD.1)
            .unwrap()
            .try_to_wgs84_refined()
            .unwrap();
        let dt = DatumTransform::catalog_to_wgs84(Crs::Tokyo).unwrap();
        let tokyo = dt
            .inverse_first_order()
            .apply(w.value().to_coordinate())
            .unwrap();
        assert_eq!(r.crs(), Crs::Tokyo);
        assert_close(r.max_error_m(), w.max_error_m() + TOKYO_ACCURACY_M, 1e-12);
        assert_close(r.lat(), tokyo.lat(), 1e-9);
        assert_close(r.lon(), tokyo.lon(), 1e-9);
    }

    #[test]
    fn china_conversion_carries_height() {
        // China transforms are 2-D — an attached height passes through unchanged.
        let g = Coordinate::gcj02(GCJ.0, GCJ.1)
            .unwrap()
            .with_height(Height::Orthometric(123.0))
            .unwrap();
        for to in [Crs::Bd09, Crs::Wgs84] {
            let r = convert(g, to).unwrap();
            assert_eq!(r.height(), Some(Height::Orthometric(123.0)));
        }
        let b = Coordinate::bd09(BD.0, BD.1)
            .unwrap()
            .with_height(Height::Orthometric(7.0))
            .unwrap();
        let r = convert(b, Crs::Gcj02).unwrap();
        assert_eq!(r.height(), Some(Height::Orthometric(7.0)));
    }

    const ALL: [Crs; 6] = [
        Crs::Wgs84,
        Crs::Gcj02,
        Crs::Bd09,
        Crs::Nad27,
        Crs::Tokyo,
        Crs::Pulkovo42,
    ];

    /// Every modeled pair has a route: a mid-latitude point converts between
    /// all of them.
    #[test]
    fn every_pair_converts() {
        for from in ALL {
            let c = Coordinate::new(WGS.0, WGS.1, from).unwrap();
            for to in ALL {
                let r = convert(c, to).unwrap_or_else(|e| panic!("{from:?} -> {to:?}: {e}"));
                assert_eq!(r.crs(), to);
            }
        }
    }

    /// A BD-09 leg that would carry a latitude past a pole is reported as
    /// outside BD-09's domain, on the direct path and through the hub.
    #[test]
    fn bd09_legs_past_a_pole_are_outside_the_domain() {
        let north = |crs| Coordinate::new(90.0, 0.0, crs).unwrap();
        for from in [Crs::Gcj02, Crs::Wgs84] {
            assert!(matches!(
                convert(north(from), Crs::Bd09),
                Err(Error::OutsideDomain {
                    operation: "BD-09",
                    ..
                })
            ));
        }
        let south = Coordinate::bd09(-90.0, 0.0).unwrap();
        assert!(matches!(
            convert(south, Crs::Wgs84),
            Err(Error::OutsideDomain {
                operation: "BD-09 inverse",
                ..
            })
        ));
    }

    /// A Helmert leg needs an ellipsoidal height, so an orthometric one is an
    /// invalid value rather than a silent zero.
    #[test]
    fn orthometric_height_on_a_helmert_leg_is_invalid() {
        let g = Coordinate::gcj02(GCJ.0, GCJ.1)
            .unwrap()
            .with_height(Height::Orthometric(10.0))
            .unwrap();
        for to in [Crs::Tokyo, Crs::Nad27, Crs::Pulkovo42] {
            assert!(matches!(
                convert(g, to),
                Err(Error::InvalidValue {
                    field: "height",
                    ..
                })
            ));
        }
    }

    /// A refined inverse that cannot converge passes its honest, residual-
    /// carrying bound through `convert` unchanged.
    #[test]
    fn an_unconverged_inverse_bound_passes_through() {
        let edge = Coordinate::gcj02(40.0, 72.006).unwrap();
        let r = convert(edge, Crs::Wgs84).unwrap();
        let direct = Gcj02::new(40.0, 72.006).unwrap().to_wgs84_refined();
        assert!(r.max_error_m() > 100.0, "{}", r.max_error_m());
        assert_eq!(r.max_error_m(), direct.max_error_m());
    }

    /// A GCJ-02 point just outside the box that an inside WGS-84 point also
    /// maps to is returned unchanged, with a bound that covers the inside one.
    #[test]
    fn an_outside_gcj02_point_with_an_inside_preimage_is_not_exact() {
        let origin = Wgs84::new(55.824_77, 130.0).unwrap();
        let gcj = origin.to_gcj02();
        let r = convert(Coordinate::gcj02(gcj.lat(), gcj.lon()).unwrap(), Crs::Wgs84).unwrap();
        assert_eq!((r.lat(), r.lon()), (gcj.lat(), gcj.lon()));
        let off = haversine_distance(&Wgs84::new(r.lat(), r.lon()).unwrap(), &origin)
            .unwrap()
            .meters();
        assert!(
            off > 600.0 && off <= r.max_error_m(),
            "{off} m vs {}",
            r.max_error_m()
        );
    }
}
