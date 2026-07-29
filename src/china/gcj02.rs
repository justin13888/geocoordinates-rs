//! WGS-84 ↔ GCJ-02 offset math.
//!
//! The forward offset (WGS-84 → GCJ-02) is the canonical, de-facto-standard
//! empirical polynomial + sinusoids — exact within the published algorithm. The
//! inverse (GCJ-02 → WGS-84) has no closed form:
//!
//! - [`Gcj02::to_wgs84_fast`] subtracts the offset computed at the wrong point
//!   (~1–2 m error).
//! - [`Gcj02::to_wgs84_refined`] uses fixed-point iteration
//!   (`wgs += target − wgs2gcj(wgs)`), converging to < 0.5 m. Preferred over the
//!   30-iteration binary search used by older ports.

use super::{EE, GCJ_A, Gcj02, Wgs84, out_of_china};
use crate::approx::Approx;
use crate::error::Result;

use core::f64::consts::PI;

/// Fast-inverse error bound (meters): subtracting the offset at the GCJ point
/// rather than the unknown WGS point leaves a sub-gradient residual.
const FAST_BOUND_M: f64 = 5.0;
/// Refined-inverse error bound (meters): the fixed-point iteration inverts the
/// published forward algorithm to well under this.
const REFINED_BOUND_M: f64 = 0.5;
/// Convergence threshold for the refined inverse, in degrees (~1 cm).
const REFINED_EPS_DEG: f64 = 1e-9;
/// Iteration cap for the refined inverse (it converges in a handful of steps).
const REFINED_MAX_ITERS: u32 = 100;

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

impl Wgs84 {
    /// WGS-84 → GCJ-02. **Exact** forward offset (identity outside China).
    pub fn try_to_gcj02(self) -> Result<Gcj02> {
        self.validate()?;
        if out_of_china(self.lat, self.lon) {
            return Ok(Gcj02::new(self.lat, self.lon));
        }
        let (d_lat, d_lon) = delta(self.lat, self.lon);
        Ok(Gcj02::new(self.lat + d_lat, self.lon + d_lon))
    }
}

impl TryFrom<Wgs84> for Gcj02 {
    type Error = crate::Error;

    /// Exact forward offset.
    fn try_from(wgs: Wgs84) -> Result<Self> {
        wgs.try_to_gcj02()
    }
}

impl Gcj02 {
    /// GCJ-02 → WGS-84, fast single-step inverse (~1–2 m error).
    ///
    /// Subtracts the offset evaluated **at the GCJ point** (the wrong point —
    /// the true offset is defined at the unknown WGS point), which is exact
    /// outside China and leaves a small residual inside it.
    pub fn try_to_wgs84_fast(self) -> Result<Approx<Wgs84>> {
        self.validate()?;
        if out_of_china(self.lat, self.lon) {
            return Ok(Approx::new(Wgs84::new(self.lat, self.lon), 0.0));
        }
        let (d_lat, d_lon) = delta(self.lat, self.lon);
        Ok(Approx::new(
            Wgs84::new(self.lat - d_lat, self.lon - d_lon),
            FAST_BOUND_M,
        ))
    }

    /// GCJ-02 → WGS-84, refined fixed-point inverse (< 0.5 m error).
    ///
    /// Iterates `wgs += target − wgs.try_to_gcj02()` until the forward image of the
    /// estimate matches `self`, converging on the WGS point whose offset lands
    /// exactly on `self`.
    pub fn try_to_wgs84_refined(self) -> Result<Approx<Wgs84>> {
        self.validate()?;
        if out_of_china(self.lat, self.lon) {
            return Ok(Approx::new(Wgs84::new(self.lat, self.lon), 0.0));
        }
        let mut wgs = Wgs84::new(self.lat, self.lon);
        for _ in 0..REFINED_MAX_ITERS {
            let cur = wgs.try_to_gcj02()?;
            let d_lat = self.lat - cur.lat;
            let d_lon = self.lon - cur.lon;
            wgs = Wgs84::new(wgs.lat + d_lat, wgs.lon + d_lon);
            if d_lat.abs() < REFINED_EPS_DEG && d_lon.abs() < REFINED_EPS_DEG {
                break;
            }
        }
        Ok(Approx::new(wgs, REFINED_BOUND_M))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{DATUM_VECTORS, assert_within_meters};

    #[test]
    fn wgs84_to_gcj02_matches_references() {
        for v in DATUM_VECTORS {
            assert_within_meters(&v.wgs84().try_to_gcj02().unwrap(), &v.gcj02(), 0.2);
        }
    }

    #[test]
    fn gcj02_to_wgs84_fast_within_5m() {
        for v in DATUM_VECTORS {
            assert_within_meters(
                v.gcj02().try_to_wgs84_fast().unwrap().value(),
                &v.wgs84(),
                5.0,
            );
        }
    }

    #[test]
    fn gcj02_to_wgs84_refined_within_half_meter() {
        for v in DATUM_VECTORS {
            assert_within_meters(
                v.gcj02().try_to_wgs84_refined().unwrap().value(),
                &v.wgs84(),
                0.5,
            );
        }
    }

    /// Our fast inverse is exactly coordtransform-rs's `gcj02_to_wgs84`
    /// (subtract the offset at the GCJ point), so it reproduces its value.
    #[test]
    fn gcj02_to_wgs84_fast_matches_coordtransform() {
        use crate::test_support::coordtransform::{GCJ_TO_WGS_FAST, INPUT};
        let got = Gcj02::new(INPUT.0, INPUT.1).try_to_wgs84_fast().unwrap();
        assert_within_meters(
            got.value(),
            &Wgs84::new(GCJ_TO_WGS_FAST.0, GCJ_TO_WGS_FAST.1),
            0.05,
        );
    }

    #[test]
    fn outside_china_is_identity() {
        let london = Wgs84::new(51.5074, -0.1278);
        let g = london.try_to_gcj02().unwrap();
        assert_eq!((g.lat, g.lon), (london.lat, london.lon));

        let back = Gcj02::new(london.lat, london.lon)
            .try_to_wgs84_refined()
            .unwrap();
        assert_eq!((back.lat, back.lon), (london.lat, london.lon));
        assert_eq!(back.max_error_m(), 0.0);
    }

    #[test]
    fn invalid_coordinates_are_rejected() {
        assert!(Wgs84::new(f64::NAN, 116.4).try_to_gcj02().is_err());
        assert!(Gcj02::new(91.0, 116.4).try_to_wgs84_refined().is_err());
    }
}
