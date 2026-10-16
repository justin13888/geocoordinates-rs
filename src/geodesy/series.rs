//! Karney–Krüger `n`-series of an ellipsoid, shared by the transverse Mercator
//! projection ([`grids::utm`](crate::grids::utm)) and the ellipsoidal rhumb
//! lines ([`rhumb`](super::rhumb)).
//!
//! Every series is in the third flattening `n = f / (2 − f)`, truncated at
//! `n⁴` (Karney 2011, "Transverse Mercator with an accuracy of a few
//! nanometers", eqs. 35–36). For the ellipsoids this crate names, `n⁵ ≈ 10⁻¹⁴`,
//! so a truncated series is good to well under a micrometer of arc.
//!
//! The latitudes involved:
//!
//! - the **isometric latitude** `ψ = asinh(tan φ) − e·atanh(e·sin φ)`, the
//!   Mercator northing on a unit sphere, in which a rhumb line is straight;
//! - the **conformal latitude** `χ = gd(ψ) = atan(sinh ψ)`;
//! - the **rectifying latitude** `μ = M(φ) / A`, the meridian arc `M` from the
//!   equator in units of the rectifying radius `A`, so `A·π/2` is the quarter
//!   meridian.

use crate::geodesy::Ellipsoid;

/// The truncated Krüger series of one ellipsoid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct KrugerSeries {
    /// Third flattening `n = f / (2 − f)`.
    pub(crate) n: f64,
    /// Rectifying radius `A = a/(1+n)·(1 + n²/4 + n⁴/64)`, in meters.
    pub(crate) rect_a: f64,
    /// Conformal → rectifying coefficients α₁..α₄ (the transverse Mercator
    /// forward series).
    pub(crate) alpha: [f64; 4],
    /// Rectifying → conformal coefficients β₁..β₄ (the transverse Mercator
    /// inverse series).
    pub(crate) beta: [f64; 4],
    /// Conformal → geodetic latitude coefficients δ₁..δ₄.
    pub(crate) delta: [f64; 4],
}

impl KrugerSeries {
    /// The series of the WGS-84 ellipsoid, evaluated at compile time.
    pub(crate) const WGS84: KrugerSeries = KrugerSeries::new(Ellipsoid::WGS84);

    /// The series of `ellipsoid`.
    pub(crate) const fn new(ellipsoid: Ellipsoid) -> KrugerSeries {
        let f = ellipsoid.flattening();
        let n = f / (2.0 - f);
        let n2 = n * n;
        let n3 = n2 * n;
        let n4 = n3 * n;
        KrugerSeries {
            n,
            rect_a: (ellipsoid.semi_major().meters() / (1.0 + n)) * (1.0 + n2 / 4.0 + n4 / 64.0),
            alpha: [
                0.5 * n - 2.0 / 3.0 * n2 + 5.0 / 16.0 * n3 + 41.0 / 180.0 * n4,
                13.0 / 48.0 * n2 - 3.0 / 5.0 * n3 + 557.0 / 1440.0 * n4,
                61.0 / 240.0 * n3 - 103.0 / 140.0 * n4,
                49561.0 / 161_280.0 * n4,
            ],
            beta: [
                0.5 * n - 2.0 / 3.0 * n2 + 37.0 / 96.0 * n3 - 1.0 / 360.0 * n4,
                1.0 / 48.0 * n2 + 1.0 / 15.0 * n3 - 437.0 / 1440.0 * n4,
                17.0 / 480.0 * n3 - 37.0 / 840.0 * n4,
                4397.0 / 161_280.0 * n4,
            ],
            delta: [
                2.0 * n - 2.0 / 3.0 * n2 - 2.0 * n3 + 116.0 / 45.0 * n4,
                7.0 / 3.0 * n2 - 8.0 / 5.0 * n3 - 227.0 / 45.0 * n4,
                56.0 / 15.0 * n3 - 136.0 / 35.0 * n4,
                4279.0 / 630.0 * n4,
            ],
        }
    }

    /// First eccentricity `e = 2√n / (1 + n)`.
    pub(crate) fn eccentricity(&self) -> f64 {
        2.0 * self.n.sqrt() / (1.0 + self.n)
    }

    /// `tan χ = sinh ψ` of geodetic latitude `phi` (radians), in the form the
    /// transverse Mercator forward uses. Infinite at a pole.
    pub(crate) fn conformal_tan(&self, phi: f64) -> f64 {
        let e = self.eccentricity();
        (phi.sin().atanh() - e * (e * phi.sin()).atanh()).sinh()
    }

    /// Isometric latitude `ψ` of geodetic latitude `phi` (radians), finite and
    /// accurate up to a pole (`asinh(tan φ)` keeps its precision there, where
    /// `atanh(sin φ)` does not).
    pub(crate) fn isometric_latitude(&self, phi: f64) -> f64 {
        let e = self.eccentricity();
        phi.tan().asinh() - e * (e * phi.sin()).atanh()
    }

    /// Geodetic latitude of conformal latitude `chi` (radians).
    pub(crate) fn geodetic_from_conformal(&self, chi: f64) -> f64 {
        chi + sum_sin(&self.delta, chi)
    }

    /// Rectifying latitude `μ` of conformal latitude `chi` (radians).
    pub(crate) fn rectifying_from_conformal(&self, chi: f64) -> f64 {
        chi + sum_sin(&self.alpha, chi)
    }

    /// Conformal latitude of rectifying latitude `mu` (radians).
    pub(crate) fn conformal_from_rectifying(&self, mu: f64) -> f64 {
        mu - sum_sin(&self.beta, mu)
    }
}

/// `Σⱼ cⱼ · sin(2jx)` for `j = 1..=4`.
fn sum_sin(coefficients: &[f64; 4], x: f64) -> f64 {
    let mut sum = 0.0;
    for (j, c) in (1..=4u8).zip(coefficients) {
        sum += c * (2.0 * f64::from(j) * x).sin();
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;
    use core::f64::consts::FRAC_PI_2;

    #[test]
    fn wgs84_constants() {
        let s = KrugerSeries::WGS84;
        // Third flattening and rectifying radius of WGS-84 (Karney 2011, §2).
        assert_close(s.n, 0.001_679_220_386_383_705, 1e-18);
        assert_close(s.rect_a, 6_367_449.145_823_416, 1e-6);
        assert_close(s.eccentricity(), 0.081_819_190_842_621_49, 1e-15);
    }

    #[test]
    fn rectifying_and_conformal_latitudes_invert_each_other() {
        let s = KrugerSeries::new(Ellipsoid::BESSEL_1841);
        for deg in [-89.0, -45.0, -10.0, 0.0, 33.0, 60.0, 89.9] {
            let chi = f64::to_radians(deg);
            let mu = s.rectifying_from_conformal(chi);
            assert_close(s.conformal_from_rectifying(mu), chi, 1e-13);
        }
        // The quarter meridian sits at μ = χ = π/2.
        assert_close(s.rectifying_from_conformal(FRAC_PI_2), FRAC_PI_2, 1e-15);
    }

    #[test]
    fn isometric_latitude_matches_the_conformal_tangent() {
        let s = KrugerSeries::WGS84;
        for deg in [-80.0, -30.0, 0.0, 15.0, 45.0, 85.0] {
            let phi = f64::to_radians(deg);
            assert_close(s.isometric_latitude(phi).sinh(), s.conformal_tan(phi), 1e-9);
        }
        // 45°N on WGS-84: ψ = asinh(1) − e·atanh(e/√2).
        let e = s.eccentricity();
        assert_close(
            s.isometric_latitude(f64::to_radians(45.0)),
            1.0_f64.asinh() - e * (e * core::f64::consts::FRAC_1_SQRT_2).atanh(),
            1e-15,
        );
        // The geodetic latitude of a conformal latitude round-trips.
        let phi = f64::to_radians(52.0);
        let chi = s.conformal_tan(phi).atan();
        assert_close(s.geodetic_from_conformal(chi), phi, 1e-13);
    }
}
