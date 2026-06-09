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
    // TODO(impl): Krasovsky-1940, Airy-1830 (OSGB), Bessel-1841, Clarke-1866, …

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
