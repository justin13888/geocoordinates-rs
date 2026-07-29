//! Central runtime conversion dispatch over [`Crs`].
//!
//! This is the *runtime* path: it dispatches on a [`Crs`] tag and returns an
//! [`Approx`] because some target systems (GCJ-02/BD-09 inverses) are only
//! reachable approximately. When the source and target are known at compile
//! time, prefer the typed newtype conversions (e.g. [`crate::Wgs84`] →
//! [`crate::Gcj02`] via [`From`]), which return exact bare types where the math
//! is exact.
//!
//! Conversions are per-coordinate; batch / vectorized conversion is left to the
//! caller (iterate, or parallelize with e.g. `rayon`) rather than offered as a
//! dedicated API.

use crate::approx::Approx;
use crate::china::{Bd09, Gcj02, Wgs84};
use crate::coord::{Coordinate, Crs};
use crate::error::Result;
use crate::geodesy::datum::DatumTransform;

/// Convert `coord` from its own [`Crs`] to `to`.
///
/// Routes through WGS-84 as the hub: China systems use the typed
/// [`china`](crate::china) conversions; classic datums (NAD27, Tokyo,
/// Pulkovo-1942) use the 7-parameter Helmert transforms from
/// [`geodesy::datum`](crate::geodesy::datum). The full EPSG/national-grid long
/// tail is delegated to the optional `proj` feature.
///
/// The result is wrapped in [`Approx`] because the worst-case path (e.g.
/// BD-09 → WGS-84) is approximate; for exact paths — including Helmert datum
/// shifts — the reported [`Approx::max_error_m`] is `0.0`.
///
/// # Errors
/// Returns [`crate::Error::UnsupportedConversion`] if no route is known between
/// the two systems.
pub fn convert(coord: Coordinate, to: Crs) -> Result<Approx<Coordinate>> {
    coord.validate()?;
    if coord.crs == to {
        return Ok(Approx::new(coord, 0.0));
    }
    // GCJ-02 → BD-09 is an exact direct transform. Routing it through the WGS-84
    // hub would taint the result with the GCJ-02 inverse error, so take the
    // direct path and keep it exact.
    if coord.crs == Crs::Gcj02 && to == Crs::Bd09 {
        let b = Gcj02::new(coord.lat, coord.lon).try_to_bd09()?;
        return Ok(Approx::new(carry(coord, b.lat, b.lon, Crs::Bd09), 0.0));
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

/// Whether a conversion route exists between two reference systems.
///
/// True for every currently-modeled [`Crs`]: each reaches the WGS-84 hub (China
/// systems via the typed conversions, classic datums via Helmert). The
/// distinction exists for the optional `proj` long tail, whose datums are not
/// hub-reachable without that feature.
#[must_use]
pub fn can_convert(from: Crs, to: Crs) -> bool {
    hub_reachable(from) && hub_reachable(to)
}

/// Whether a reference system can reach the WGS-84 hub without the `proj`
/// feature. Exhaustive — a new [`Crs`] must declare its routability here.
fn hub_reachable(crs: Crs) -> bool {
    match crs {
        Crs::Wgs84 | Crs::Gcj02 | Crs::Bd09 | Crs::Nad27 | Crs::Tokyo | Crs::Pulkovo42 => true,
    }
}

/// Convert `coord` to a WGS-84 [`Coordinate`], carrying the per-leg error bound.
fn to_wgs84(coord: Coordinate) -> Result<Approx<Coordinate>> {
    Ok(match coord.crs {
        Crs::Wgs84 => Approx::new(coord, 0.0),
        Crs::Gcj02 => Gcj02::new(coord.lat, coord.lon)
            .try_to_wgs84_refined()?
            .map(|w| carry(coord, w.lat, w.lon, Crs::Wgs84)),
        Crs::Bd09 => Bd09::new(coord.lat, coord.lon)
            .try_to_wgs84_refined()?
            .map(|w| carry(coord, w.lat, w.lon, Crs::Wgs84)),
        Crs::Nad27 | Crs::Tokyo | Crs::Pulkovo42 => {
            let dt = DatumTransform::to_wgs84(coord.crs).expect("classic datum is catalogued");
            Approx::new(dt.transform(coord)?, 0.0)
        }
    })
}

/// Convert a WGS-84 [`Coordinate`] to `to`, carrying the per-leg error bound.
fn from_wgs84(wgs: Coordinate, to: Crs) -> Result<Approx<Coordinate>> {
    Ok(match to {
        Crs::Wgs84 => Approx::new(wgs, 0.0),
        Crs::Gcj02 => {
            let g = Wgs84::new(wgs.lat, wgs.lon).try_to_gcj02()?;
            Approx::new(carry(wgs, g.lat, g.lon, Crs::Gcj02), 0.0)
        }
        Crs::Bd09 => {
            let b = Wgs84::new(wgs.lat, wgs.lon).try_to_bd09()?;
            Approx::new(carry(wgs, b.lat, b.lon, Crs::Bd09), 0.0)
        }
        Crs::Nad27 | Crs::Tokyo | Crs::Pulkovo42 => {
            let dt = DatumTransform::to_wgs84(to).expect("classic datum is catalogued");
            Approx::new(dt.inverse().transform(wgs)?, 0.0)
        }
    })
}

/// Rebuild a coordinate with new lat/lon and CRS, carrying the original height
/// (the China lat/lon transforms are 2-D; height passes through unchanged).
fn carry(orig: Coordinate, lat: f64, lon: f64, crs: Crs) -> Coordinate {
    Coordinate {
        lat,
        lon,
        height: orig.height,
        crs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coord::Height;
    use crate::test_support::assert_close;

    // Reference triple from `test_support` (a single WGS-84 point and its exact
    // GCJ-02 / BD-09 images): (lat, lon).
    const WGS: (f64, f64) = (39.915, 116.404);
    const GCJ: (f64, f64) = (39.916_404_281_501_64, 116.410_244_499_169_38);
    const BD: (f64, f64) = (39.922_699_552_216_216, 116.416_627_243_787_33);

    #[test]
    fn same_crs_is_exact_identity() {
        let c = Coordinate::wgs84(WGS.0, WGS.1);
        let r = convert(c, Crs::Wgs84).unwrap();
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs, Crs::Wgs84);
        assert_close(r.lat, WGS.0, 1e-12);
        assert_close(r.lon, WGS.1, 1e-12);
    }

    #[test]
    fn wgs84_to_gcj02_is_exact() {
        let r = convert(Coordinate::wgs84(WGS.0, WGS.1), Crs::Gcj02).unwrap();
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs, Crs::Gcj02);
        // Exactly the typed forward conversion.
        let g = Wgs84::new(WGS.0, WGS.1).try_to_gcj02().unwrap();
        assert_close(r.lat, g.lat, 1e-12);
        assert_close(r.lon, g.lon, 1e-12);
        assert_close(r.lat, GCJ.0, 1e-9);
    }

    #[test]
    fn gcj02_to_wgs84_is_approximate() {
        let r = convert(Coordinate::gcj02(GCJ.0, GCJ.1), Crs::Wgs84).unwrap();
        // Bound is exactly the delegated refined-inverse bound (not re-derived).
        let direct = Gcj02::new(GCJ.0, GCJ.1).try_to_wgs84_refined().unwrap();
        assert!(r.max_error_m() > 0.0);
        assert_close(r.max_error_m(), direct.max_error_m(), 1e-12);
        assert_close(r.lat, direct.lat, 1e-12);
        // Recovers the original WGS-84 within the stated bound.
        let bound = r.max_error_m();
        crate::test_support::assert_within_meters(
            &r.into_inner(),
            &Coordinate::wgs84(WGS.0, WGS.1),
            bound,
        );
    }

    #[test]
    fn gcj02_to_bd09_stays_exact_via_direct_path() {
        let r = convert(Coordinate::gcj02(GCJ.0, GCJ.1), Crs::Bd09).unwrap();
        // The direct GCJ→BD path keeps it exact — no WGS-84 inverse error leaks in.
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs, Crs::Bd09);
        let b = Gcj02::new(GCJ.0, GCJ.1).try_to_bd09().unwrap();
        assert_close(r.lat, b.lat, 1e-12);
        assert_close(r.lon, b.lon, 1e-12);
        assert_close(r.lat, BD.0, 1e-9);
    }

    #[test]
    fn bd09_to_wgs84_is_approximate() {
        let r = convert(Coordinate::bd09(BD.0, BD.1), Crs::Wgs84).unwrap();
        assert!(r.max_error_m() > 0.0);
        let bound = r.max_error_m();
        crate::test_support::assert_within_meters(
            &r.into_inner(),
            &Coordinate::wgs84(WGS.0, WGS.1),
            bound,
        );
    }

    #[test]
    fn nad27_to_wgs84_is_exact() {
        // Same independent reference as the datum milestone.
        let nad27 = Coordinate::new(40.0, -100.0, Crs::Nad27).with_height(Height::Ellipsoidal(0.0));
        let r = convert(nad27, Crs::Wgs84).unwrap();
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs, Crs::Wgs84);
        assert_close(r.lat, 40.000_009_482_759, 1e-8);
        assert_close(r.lon, -100.000_417_622_218_8, 1e-8);
    }

    #[test]
    fn classic_datum_round_trips_exactly() {
        // NAD27 ↔ WGS-84 is exact both ways, so a round trip recovers the input.
        let nad27 = Coordinate::new(40.0, -100.0, Crs::Nad27).with_height(Height::Ellipsoidal(0.0));
        let w = convert(nad27, Crs::Wgs84).unwrap().into_inner();
        let back = convert(w, Crs::Nad27).unwrap();
        assert_eq!(back.max_error_m(), 0.0);
        assert_eq!(back.crs, Crs::Nad27);
        assert_close(back.lat, 40.0, 1e-9);
        assert_close(back.lon, -100.0, 1e-9);
    }

    #[test]
    fn cross_datum_to_china_is_exact() {
        // NAD27 → WGS-84 (Helmert, exact) → GCJ-02 (typed forward, exact): both
        // legs exact, so the composed bound is zero.
        let nad27 = Coordinate::new(40.0, -100.0, Crs::Nad27);
        let r = convert(nad27, Crs::Gcj02).unwrap();
        assert_eq!(r.max_error_m(), 0.0);
        assert_eq!(r.crs, Crs::Gcj02);
        // Equals the explicit two-step route.
        let dt = DatumTransform::to_wgs84(Crs::Nad27).unwrap();
        let w = dt.transform(nad27).unwrap();
        let g = Wgs84::new(w.lat, w.lon).try_to_gcj02().unwrap();
        assert_close(r.lat, g.lat, 1e-12);
        assert_close(r.lon, g.lon, 1e-12);
    }

    #[test]
    fn cross_china_to_datum_accumulates_the_bound() {
        // BD-09 → WGS-84 (approx) → Tokyo (Helmert, exact): the composed bound is
        // exactly the BD-09 inverse bound (the exact leg adds nothing).
        let r = convert(Coordinate::bd09(BD.0, BD.1), Crs::Tokyo).unwrap();
        let w = Bd09::new(BD.0, BD.1).try_to_wgs84_refined().unwrap();
        let dt = DatumTransform::to_wgs84(Crs::Tokyo).unwrap();
        let tokyo = dt
            .inverse()
            .transform(Coordinate::wgs84(w.lat, w.lon))
            .unwrap();
        assert_eq!(r.crs, Crs::Tokyo);
        assert_close(r.max_error_m(), w.max_error_m(), 1e-12);
        assert_close(r.lat, tokyo.lat, 1e-9);
        assert_close(r.lon, tokyo.lon, 1e-9);
    }

    #[test]
    fn china_conversion_carries_height() {
        // China transforms are 2-D — an attached height passes through unchanged.
        let g = Coordinate::gcj02(GCJ.0, GCJ.1).with_height(Height::Orthometric(123.0));
        let r = convert(g, Crs::Bd09).unwrap();
        assert_eq!(r.height, Some(Height::Orthometric(123.0)));
    }

    #[test]
    fn can_convert_spans_all_modeled_systems() {
        for from in [
            Crs::Wgs84,
            Crs::Gcj02,
            Crs::Bd09,
            Crs::Nad27,
            Crs::Tokyo,
            Crs::Pulkovo42,
        ] {
            for to in [
                Crs::Wgs84,
                Crs::Gcj02,
                Crs::Bd09,
                Crs::Nad27,
                Crs::Tokyo,
                Crs::Pulkovo42,
            ] {
                assert!(
                    can_convert(from, to),
                    "{from:?} -> {to:?} should be routable"
                );
            }
        }
    }
}
