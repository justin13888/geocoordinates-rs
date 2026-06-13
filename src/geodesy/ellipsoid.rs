//! Reference ellipsoid definitions.

/// A reference ellipsoid defined by its semi-major axis and flattening.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ellipsoid {
    /// Semi-major axis `a`, in meters.
    pub semi_major_m: f64,
    /// Inverse flattening `1/f`.
    pub inverse_flattening: f64,
}

impl Ellipsoid {
    /// The WGS-84 ellipsoid.
    pub const WGS84: Ellipsoid = Ellipsoid {
        semi_major_m: 6_378_137.0,
        inverse_flattening: 298.257_223_563,
    };

    /// The GRS80 ellipsoid (NAD83/ETRS89).
    pub const GRS80: Ellipsoid = Ellipsoid {
        semi_major_m: 6_378_137.0,
        inverse_flattening: 298.257_222_101,
    };

    /// The Krasovsky-1940 ellipsoid (Pulkovo-1942, SK-42, and derived datums).
    pub const KRASOVSKY_1940: Ellipsoid = Ellipsoid {
        semi_major_m: 6_378_245.0,
        inverse_flattening: 298.3,
    };

    /// The Airy-1830 ellipsoid (OSGB36 / Ordnance Survey of Great Britain).
    pub const AIRY_1830: Ellipsoid = Ellipsoid {
        semi_major_m: 6_377_563.396,
        inverse_flattening: 299.324_964_6,
    };

    /// The Bessel-1841 ellipsoid (Tokyo datum, older European datums).
    pub const BESSEL_1841: Ellipsoid = Ellipsoid {
        semi_major_m: 6_377_397.155,
        inverse_flattening: 299.152_812_8,
    };

    /// The Clarke-1866 ellipsoid (NAD27 / North American Datum 1927).
    pub const CLARKE_1866: Ellipsoid = Ellipsoid {
        semi_major_m: 6_378_206.4,
        inverse_flattening: 294.978_698_2,
    };

    /// Flattening `f`.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        1.0 / self.inverse_flattening
    }

    /// Semi-minor axis `b`, in meters.
    #[must_use]
    pub fn semi_minor_m(&self) -> f64 {
        self.semi_major_m * (1.0 - self.flattening())
    }

    /// First eccentricity squared `e² = f(2 − f)`.
    #[must_use]
    pub fn eccentricity_sq(&self) -> f64 {
        let f = self.flattening();
        f * (2.0 - f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    #[test]
    fn wgs84_derived_quantities() {
        let e = Ellipsoid::WGS84;
        assert_close(e.flattening(), 1.0 / 298.257_223_563, 1e-18);
        assert_close(e.semi_minor_m(), 6_356_752.314_245_18, 1e-6);
        assert_close(e.eccentricity_sq(), 0.006_694_379_990_141_32, 1e-15);
    }

    #[test]
    fn sphere_has_no_flattening() {
        // A sphere (inverse_flattening = ∞) has f = 0, b = a, e² = 0.
        let sphere = Ellipsoid {
            semi_major_m: 1000.0,
            inverse_flattening: f64::INFINITY,
        };
        assert_close(sphere.flattening(), 0.0, 0.0);
        assert_close(sphere.semi_minor_m(), 1000.0, 1e-12);
        assert_close(sphere.eccentricity_sq(), 0.0, 0.0);
    }
}
