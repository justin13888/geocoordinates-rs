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
        todo!("1.0 / inverse_flattening")
    }

    /// Semi-minor axis `b`, in meters.
    #[must_use]
    pub fn semi_minor_m(&self) -> f64 {
        todo!("a * (1 - f)")
    }

    /// First eccentricity squared `e²`.
    #[must_use]
    pub fn eccentricity_sq(&self) -> f64 {
        todo!("f * (2 - f)")
    }
}
