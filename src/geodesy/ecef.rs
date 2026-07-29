//! Earth-Centered, Earth-Fixed (ECEF) geocentric coordinates.
//!
//! ECEF is the bridge format for almost every datum transformation. The
//! geodetic ↔ ECEF conversion is closed-form and treated as **exact** (the
//! inverse uses Bowring's well-converged formula). Named fallible methods
//! validate the ellipsoid, numeric inputs, and ellipsoidal height semantics.

use super::ellipsoid::Ellipsoid;
use crate::coord::{Coordinate, Crs, Height};
use crate::error::{Error, Result};

fn height_meters(coord: Coordinate) -> Result<f64> {
    match coord.height {
        Some(Height::Ellipsoidal(h)) => Ok(h),
        Some(Height::Orthometric(_)) => Err(Error::InvalidValue {
            field: "height",
            detail: "ECEF conversion requires ellipsoidal height".into(),
        }),
        None => Ok(0.0),
    }
}

/// A geocentric ECEF position in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ecef {
    /// X axis (meters), through the prime meridian at the equator.
    pub x: f64,
    /// Y axis (meters), 90° east at the equator.
    pub y: f64,
    /// Z axis (meters), through the north pole.
    pub z: f64,
}

impl Ecef {
    /// Construct from X/Y/Z in meters.
    #[must_use]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// ECEF → geodetic [`Coordinate`] (lat/lon/height) on the given ellipsoid
    /// (exact, Bowring closed-form inverse).
    ///
    /// ECEF is datum-agnostic, so `crs` explicitly tags the output with the
    /// reference system to which `ellipsoid` belongs.
    pub fn try_to_coordinate(self, ellipsoid: Ellipsoid, crs: Crs) -> Result<Coordinate> {
        ellipsoid.validate()?;
        if !self.x.is_finite() || !self.y.is_finite() || !self.z.is_finite() {
            return Err(Error::InvalidValue {
                field: "ECEF",
                detail: "x, y, and z must be finite".into(),
            });
        }
        let a = ellipsoid.semi_major_m;
        let b = ellipsoid.semi_minor_m();
        let e2 = ellipsoid.eccentricity_sq();
        let ep2 = (a * a - b * b) / (b * b); // second eccentricity squared
        let p = (self.x * self.x + self.y * self.y).sqrt();
        let lon = self.y.atan2(self.x).to_degrees();

        // Bowring's closed-form latitude.
        let theta = (self.z * a).atan2(p * b);
        let (sin_t, cos_t) = (theta.sin(), theta.cos());
        let lat = (self.z + ep2 * b * sin_t.powi(3)).atan2(p - e2 * a * cos_t.powi(3));
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        // Height from the branchless identity h = p·cosφ + z·sinφ − a·√(1 − e²sin²φ),
        // which (unlike p/cosφ − N) is well-defined at the poles too.
        let w = (1.0 - e2 * sin_lat * sin_lat).sqrt();
        let h = p * cos_lat + self.z * sin_lat - a * w;

        let result =
            Coordinate::new(lat.to_degrees(), lon, crs).with_height(Height::Ellipsoidal(h));
        result.validate()?;
        Ok(result)
    }

    /// Geodetic [`Coordinate`] → ECEF on the given ellipsoid (exact, closed
    /// form). Missing height means zero ellipsoidal height; orthometric height
    /// is rejected because no geoid model is available.
    pub fn try_from_coordinate(coord: Coordinate, ellipsoid: Ellipsoid) -> Result<Self> {
        coord.validate()?;
        ellipsoid.validate()?;
        let phi = coord.lat.to_radians();
        let lambda = coord.lon.to_radians();
        let h = height_meters(coord)?;
        let a = ellipsoid.semi_major_m;
        let e2 = ellipsoid.eccentricity_sq();
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        let n = a / (1.0 - e2 * sin_phi * sin_phi).sqrt();
        Ok(Ecef {
            x: (n + h) * cos_phi * lambda.cos(),
            y: (n + h) * cos_phi * lambda.sin(),
            z: (n * (1.0 - e2) + h) * sin_phi,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    const WGS84_A: f64 = 6_378_137.0;
    const WGS84_B: f64 = 6_356_752.314_245_18;

    #[test]
    fn forward_reference_points() {
        let e = Ellipsoid::WGS84;
        let at = |lat, lon| Ecef::try_from_coordinate(Coordinate::wgs84(lat, lon), e).unwrap();
        let p = at(0.0, 0.0);
        assert_close(p.x, WGS84_A, 1e-3);
        assert_close(p.y, 0.0, 1e-3);
        assert_close(p.z, 0.0, 1e-3);
        let p = at(0.0, 90.0);
        assert_close(p.x, 0.0, 1e-3);
        assert_close(p.y, WGS84_A, 1e-3);
        let p = at(90.0, 0.0);
        assert_close(p.x, 0.0, 1e-3);
        assert_close(p.z, WGS84_B, 1e-3);
    }

    #[test]
    fn round_trip_geodetic() {
        let e = Ellipsoid::WGS84;
        for (lat, lon, h) in [
            (40.7128, -74.006, 100.0),
            (-33.8688, 151.2093, 0.0),
            (51.5074, -0.1278, 1500.0),
            (90.0, 0.0, 0.0), // north pole (polar-axis height branch)
            (-89.9, 179.9, 50.0),
        ] {
            let c = Coordinate::wgs84(lat, lon).with_height(Height::Ellipsoidal(h));
            let back = Ecef::try_from_coordinate(c, e)
                .unwrap()
                .try_to_coordinate(e, Crs::Wgs84)
                .unwrap();
            assert_close(back.lat, lat, 1e-9);
            // Longitude is undefined at the exact pole; skip it there.
            if lat.abs() < 90.0 {
                assert_close(back.lon, lon, 1e-9);
            }
            match back.height {
                Some(Height::Ellipsoidal(hb)) => assert_close(hb, h, 1e-4),
                other => panic!("expected ellipsoidal height, got {other:?}"),
            }
        }
    }

    #[test]
    fn invalid_inputs_and_orthometric_heights_are_rejected() {
        assert!(
            Ecef::try_from_coordinate(
                Coordinate::wgs84(0.0, 0.0).with_height(Height::Orthometric(1.0)),
                Ellipsoid::WGS84,
            )
            .is_err()
        );
        assert!(
            Ecef::new(f64::NAN, 0.0, 0.0)
                .try_to_coordinate(Ellipsoid::WGS84, Crs::Wgs84)
                .is_err()
        );
    }
}
