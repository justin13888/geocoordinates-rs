//! UTM (Universal Transverse Mercator) and UPS (Universal Polar Stereographic).
//!
//! The projection is deterministic and invertible to high precision, so it is
//! **exact**. Constructing from a coordinate can fail (UTM is undefined at the
//! poles, where UPS is used instead), so the lat/lon → grid direction uses
//! [`TryFrom`].
//!
//! The transverse Mercator forward/inverse use the Karney–Krüger `n`-series to
//! 4th order — sub-millimeter throughout a UTM zone. UPS uses the exact
//! conformal polar stereographic projection.

use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// UTM scale factor on the central meridian.
const K0: f64 = 0.9996;
/// UTM false easting (meters).
const FALSE_EASTING: f64 = 500_000.0;
/// UTM false northing applied in the southern hemisphere (meters).
const FALSE_NORTHING_SOUTH: f64 = 10_000_000.0;

// --- WGS-84 transverse Mercator (Karney–Krüger n-series) constants ---
const A_AXIS: f64 = 6_378_137.0;
const INV_F: f64 = 298.257_223_563;
const F: f64 = 1.0 / INV_F;
/// Third flattening `n = f / (2 − f)`.
const N: f64 = F / (2.0 - F);
const N2: f64 = N * N;
const N3: f64 = N2 * N;
const N4: f64 = N3 * N;
/// Rectifying radius `A = a/(1+n)·(1 + n²/4 + n⁴/64)`, in meters.
const RECT_A: f64 = (A_AXIS / (1.0 + N)) * (1.0 + N2 / 4.0 + N4 / 64.0);
/// Krüger forward (geodetic → grid) coefficients α₁..α₄.
const ALPHA: [f64; 4] = [
    0.5 * N - 2.0 / 3.0 * N2 + 5.0 / 16.0 * N3 + 41.0 / 180.0 * N4,
    13.0 / 48.0 * N2 - 3.0 / 5.0 * N3 + 557.0 / 1440.0 * N4,
    61.0 / 240.0 * N3 - 103.0 / 140.0 * N4,
    49561.0 / 161_280.0 * N4,
];
/// Krüger inverse (grid → geodetic) coefficients β₁..β₄.
const BETA: [f64; 4] = [
    0.5 * N - 2.0 / 3.0 * N2 + 37.0 / 96.0 * N3 - 1.0 / 360.0 * N4,
    1.0 / 48.0 * N2 + 1.0 / 15.0 * N3 - 437.0 / 1440.0 * N4,
    17.0 / 480.0 * N3 - 37.0 / 840.0 * N4,
    4397.0 / 161_280.0 * N4,
];
/// Krüger inverse conformal → geodetic latitude coefficients δ₁..δ₄.
const DELTA: [f64; 4] = [
    2.0 * N - 2.0 / 3.0 * N2 - 2.0 * N3 + 116.0 / 45.0 * N4,
    7.0 / 3.0 * N2 - 8.0 / 5.0 * N3 - 227.0 / 45.0 * N4,
    56.0 / 15.0 * N3 - 136.0 / 35.0 * N4,
    4279.0 / 630.0 * N4,
];

/// Northern or southern hemisphere band for a UTM coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hemisphere {
    /// Northern hemisphere.
    North,
    /// Southern hemisphere.
    South,
}

/// A UTM coordinate: zone, hemisphere, easting, and northing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Utm {
    /// Longitude zone number, 1–60.
    pub zone: u8,
    /// Hemisphere band.
    pub hemisphere: Hemisphere,
    /// Easting in meters (false-easting applied).
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

/// A UPS coordinate for the polar regions where UTM is undefined.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ups {
    /// Whether this is the north or south polar zone.
    pub hemisphere: Hemisphere,
    /// Easting in meters.
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

/// The UTM longitude zone for a coordinate, applying the Norway and Svalbard
/// exceptions.
#[must_use]
pub fn zone_for(lat: f64, lon: f64) -> u8 {
    let base = (((lon + 180.0) / 6.0).floor() as i32).clamp(0, 59) as u8 + 1;
    // Norway: zone 32 is widened westward to 3°E across band V (56°–64°N).
    if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
        return 32;
    }
    // Svalbard: the odd zones 31/33/35/37 are widened (32/34/36 dropped) across
    // 72°–84°N, 0°–42°E.
    if (72.0..84.0).contains(&lat) && (0.0..42.0).contains(&lon) {
        return if lon < 9.0 {
            31
        } else if lon < 21.0 {
            33
        } else if lon < 33.0 {
            35
        } else {
            37
        };
    }
    base
}

/// The central meridian (degrees) of a UTM zone.
#[must_use]
pub fn central_meridian_deg(zone: u8) -> f64 {
    f64::from(zone) * 6.0 - 183.0
}

impl Utm {
    /// Geodetic WGS-84 → UTM. Fails in the polar regions (use [`Ups`] there).
    ///
    /// The canonical inherent form behind [`TryFrom<Coordinate>`](Utm).
    ///
    /// # Errors
    /// [`Error::OutOfRange`] for latitudes outside the UTM band (south of 80°S
    /// or north of 84°N).
    pub fn try_from_coordinate(coord: Coordinate) -> Result<Utm> {
        let (lat, lon) = (coord.lat, coord.lon);
        if !(-80.0..84.0).contains(&lat) {
            return Err(Error::OutOfRange { lat, lon });
        }
        let zone = zone_for(lat, lon);
        let lon0 = central_meridian_deg(zone);
        let phi = lat.to_radians();
        let d_lon = (lon - lon0).to_radians();

        let e_t = 2.0 * N.sqrt() / (1.0 + N);
        let t = (phi.sin().atanh() - e_t * (e_t * phi.sin()).atanh()).sinh();
        let xi_p = t.atan2(d_lon.cos());
        let eta_p = (d_lon.sin() / (1.0 + t * t).sqrt()).atanh();

        let mut xi = xi_p;
        let mut eta = eta_p;
        for j in 1..=4usize {
            let jf = j as f64;
            xi += ALPHA[j - 1] * (2.0 * jf * xi_p).sin() * (2.0 * jf * eta_p).cosh();
            eta += ALPHA[j - 1] * (2.0 * jf * xi_p).cos() * (2.0 * jf * eta_p).sinh();
        }

        let easting = FALSE_EASTING + K0 * RECT_A * eta;
        let mut northing = K0 * RECT_A * xi;
        let hemisphere = if lat < 0.0 {
            northing += FALSE_NORTHING_SOUTH;
            Hemisphere::South
        } else {
            Hemisphere::North
        };
        Ok(Utm {
            zone,
            hemisphere,
            easting,
            northing,
        })
    }

    /// UTM → geodetic WGS-84 coordinate (exact inverse projection).
    #[must_use]
    pub fn to_coordinate(self) -> Coordinate {
        let lon0 = central_meridian_deg(self.zone);
        let n0 = match self.hemisphere {
            Hemisphere::North => 0.0,
            Hemisphere::South => FALSE_NORTHING_SOUTH,
        };
        let xi = (self.northing - n0) / (K0 * RECT_A);
        let eta = (self.easting - FALSE_EASTING) / (K0 * RECT_A);

        let mut xi_p = xi;
        let mut eta_p = eta;
        for j in 1..=4usize {
            let jf = j as f64;
            xi_p -= BETA[j - 1] * (2.0 * jf * xi).sin() * (2.0 * jf * eta).cosh();
            eta_p -= BETA[j - 1] * (2.0 * jf * xi).cos() * (2.0 * jf * eta).sinh();
        }

        let chi = (xi_p.sin() / eta_p.cosh()).asin();
        let mut lat = chi;
        for j in 1..=4usize {
            let jf = j as f64;
            lat += DELTA[j - 1] * (2.0 * jf * chi).sin();
        }
        let lon = lon0.to_radians() + eta_p.sinh().atan2(xi_p.cos());
        Coordinate::wgs84(lat.to_degrees(), lon.to_degrees())
    }
}

impl TryFrom<Coordinate> for Utm {
    type Error = crate::Error;

    /// Geodetic → UTM. Fails in the polar regions (use [`Ups`] there). Thin
    /// sugar over [`Utm::try_from_coordinate`].
    fn try_from(coord: Coordinate) -> Result<Self> {
        Utm::try_from_coordinate(coord)
    }
}

impl Ups {
    /// Geodetic WGS-84 → UPS. The canonical inherent form behind
    /// [`TryFrom<Coordinate>`](Ups).
    ///
    /// # Errors
    /// [`Error::OutOfRange`] for latitudes outside the polar zones (between 80°S
    /// and 84°N, which are UTM territory).
    pub fn try_from_coordinate(coord: Coordinate) -> Result<Ups> {
        let (lat, lon) = (coord.lat, coord.lon);
        let north = lat >= 0.0;
        if (north && lat < 84.0) || (!north && lat > -80.0) {
            return Err(Error::OutOfRange { lat, lon });
        }
        let (easting, northing) = ups_forward(lat, lon, north);
        Ok(Ups {
            hemisphere: if north {
                Hemisphere::North
            } else {
                Hemisphere::South
            },
            easting,
            northing,
        })
    }

    /// UPS → geodetic WGS-84 coordinate (exact inverse polar stereographic).
    #[must_use]
    pub fn to_coordinate(self) -> Coordinate {
        let north = self.hemisphere == Hemisphere::North;
        let (lat, lon) = ups_inverse(self.easting, self.northing, north);
        Coordinate::wgs84(lat, lon)
    }
}

impl TryFrom<Coordinate> for Ups {
    type Error = crate::Error;

    /// Geodetic → UPS. Thin sugar over [`Ups::try_from_coordinate`].
    fn try_from(coord: Coordinate) -> Result<Self> {
        Ups::try_from_coordinate(coord)
    }
}

/// UPS false easting/northing (meters), shared by both polar zones.
const UPS_FALSE: f64 = 2_000_000.0;
/// UPS central scale factor.
const UPS_K0: f64 = 0.994;

/// First eccentricity `e` of WGS-84.
fn wgs84_e() -> f64 {
    (F * (2.0 - F)).sqrt()
}

/// Polar stereographic forward (geodetic → UPS easting/northing).
fn ups_forward(lat: f64, lon: f64, north: bool) -> (f64, f64) {
    let e = wgs84_e();
    let phi = lat.abs().to_radians();
    let lam = lon.to_radians();
    // Conformal radius from the pole (Snyder 21-33 / 21-34).
    let t = ((core::f64::consts::FRAC_PI_4 - phi / 2.0).tan())
        / ((1.0 - e * phi.sin()) / (1.0 + e * phi.sin())).powf(e / 2.0);
    let big_a = 2.0 * A_AXIS * UPS_K0 / ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt();
    let r = big_a * t;
    let (e_off, n_off) = if north {
        (r * lam.sin(), -r * lam.cos())
    } else {
        (r * lam.sin(), r * lam.cos())
    };
    (UPS_FALSE + e_off, UPS_FALSE + n_off)
}

/// Polar stereographic inverse (UPS easting/northing → geodetic lat/lon).
fn ups_inverse(easting: f64, northing: f64, north: bool) -> (f64, f64) {
    let e = wgs84_e();
    let x = easting - UPS_FALSE;
    let y = northing - UPS_FALSE;
    let big_a = 2.0 * A_AXIS * UPS_K0 / ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt();
    let r = (x * x + y * y).sqrt();
    let t = r / big_a;
    // Conformal latitude, then series back to geodetic (Snyder 3-5).
    let chi = core::f64::consts::FRAC_PI_2 - 2.0 * t.atan();
    let e2 = e * e;
    let e4 = e2 * e2;
    let e6 = e4 * e2;
    let lat_n = chi
        + (e2 / 2.0 + 5.0 * e4 / 24.0 + e6 / 12.0) * (2.0 * chi).sin()
        + (7.0 * e4 / 48.0 + 29.0 * e6 / 240.0) * (4.0 * chi).sin()
        + (7.0 * e6 / 120.0) * (6.0 * chi).sin();
    // North forward sets y = −r·cosλ, south sets y = +r·cosλ; invert each.
    let lon = if north { x.atan2(-y) } else { x.atan2(y) };
    let lat = if north { lat_n } else { -lat_n };
    (lat.to_degrees(), lon.to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    fn c(lat: f64, lon: f64) -> Coordinate {
        Coordinate::wgs84(lat, lon)
    }

    #[test]
    fn zone_selection_with_exceptions() {
        assert_eq!(zone_for(40.0, -75.0), 18); // central meridian −75°
        assert_eq!(zone_for(48.8584, 2.2945), 31);
        assert_eq!(zone_for(-33.8568, 151.2153), 56);
        // Norway: 5°E across band V belongs to the widened zone 32.
        assert_eq!(zone_for(60.0, 5.0), 32);
        assert_eq!(zone_for(60.0, 2.0), 31); // west of the widening stays 31
        // Svalbard: odd zones 31/33/37 widen, 32/34/36 vanish.
        assert_eq!(zone_for(78.0, 8.0), 31);
        assert_eq!(zone_for(78.0, 20.0), 33);
        assert_eq!(zone_for(72.5, 35.0), 37);
        // Antimeridian and prime-meridian extremes.
        assert_eq!(zone_for(0.0, -180.0), 1);
        assert_eq!(zone_for(0.0, 179.999), 60);
    }

    #[test]
    fn central_meridian_of_zones() {
        assert_close(central_meridian_deg(1), -177.0, 1e-12);
        assert_close(central_meridian_deg(18), -75.0, 1e-12);
        assert_close(central_meridian_deg(31), 3.0, 1e-12);
        assert_close(central_meridian_deg(60), 177.0, 1e-12);
    }

    #[test]
    fn utm_forward_reference_points() {
        // References from the `utm` Python library (sub-mm agreement).
        let u = Utm::try_from_coordinate(c(40.0, -75.0)).unwrap();
        assert_eq!((u.zone, u.hemisphere), (18, Hemisphere::North));
        assert_close(u.easting, 500_000.0, 1e-3); // on the central meridian
        assert_close(u.northing, 4_427_757.218_844_7, 1e-3);

        let u = Utm::try_from_coordinate(c(48.8584, 2.2945)).unwrap();
        assert_eq!((u.zone, u.hemisphere), (31, Hemisphere::North));
        assert_close(u.easting, 448_252.001_375_2, 1e-3);
        assert_close(u.northing, 5_411_954.910_279_2, 1e-3);

        let u = Utm::try_from_coordinate(c(-33.8568, 151.2153)).unwrap();
        assert_eq!((u.zone, u.hemisphere), (56, Hemisphere::South));
        assert_close(u.easting, 334_900.569_650_6, 1e-3);
        assert_close(u.northing, 6_252_288.752_863_7, 1e-3);
    }

    #[test]
    fn utm_round_trips_sub_millimeter() {
        for &(lat, lon) in &[
            (40.0, -75.0),
            (48.8584, 2.2945),
            (-33.8568, 151.2153),
            (60.0, 5.0),
            (-80.0 + 1e-6, -179.0),
            (83.9, 100.0),
        ] {
            let back = Utm::try_from_coordinate(c(lat, lon))
                .unwrap()
                .to_coordinate();
            assert_close(back.lat, lat, 1e-9);
            assert_close(back.lon, lon, 1e-9);
        }
    }

    #[test]
    fn utm_rejects_polar_latitudes() {
        assert!(Utm::try_from_coordinate(c(84.0, 10.0)).is_err()); // 84°N is UPS
        assert!(Utm::try_from_coordinate(c(-80.000_1, 10.0)).is_err());
        assert!(Utm::try_from_coordinate(c(83.999, 10.0)).is_ok());
    }

    #[test]
    fn ups_forward_reference_points() {
        // References from pyproj (EPSG:5041 north / 5042 south).
        let u = Ups::try_from_coordinate(c(85.0, 0.0)).unwrap();
        assert_eq!(u.hemisphere, Hemisphere::North);
        assert_close(u.easting, 2_000_000.0, 1e-3);
        assert_close(u.northing, 1_444_542.608_6, 1e-3);

        let u = Ups::try_from_coordinate(c(87.0, 45.0)).unwrap();
        assert_close(u.easting, 2_235_568.724_8, 1e-3);
        assert_close(u.northing, 1_764_431.275_2, 1e-3);

        let u = Ups::try_from_coordinate(c(-85.0, 30.0)).unwrap();
        assert_eq!(u.hemisphere, Hemisphere::South);
        assert_close(u.easting, 2_277_728.695_7, 1e-3);
        assert_close(u.northing, 2_481_040.211_7, 1e-3);
    }

    #[test]
    fn ups_pole_is_the_false_origin() {
        let np = Ups::try_from_coordinate(c(90.0, 0.0)).unwrap();
        assert_close(np.easting, 2_000_000.0, 1e-6);
        assert_close(np.northing, 2_000_000.0, 1e-6);
    }

    #[test]
    fn ups_round_trips() {
        for &(lat, lon) in &[
            (85.0, 0.0),
            (87.0, 45.0),
            (84.5, 170.0),
            (-85.0, 30.0),
            (-88.0, -120.0),
        ] {
            let back = Ups::try_from_coordinate(c(lat, lon))
                .unwrap()
                .to_coordinate();
            assert_close(back.lat, lat, 1e-9);
            assert_close(back.lon, lon, 1e-9);
        }
    }

    #[test]
    fn ups_rejects_non_polar_latitudes() {
        assert!(Ups::try_from_coordinate(c(50.0, 0.0)).is_err());
        assert!(Ups::try_from_coordinate(c(83.9, 0.0)).is_err()); // still UTM
        assert!(Ups::try_from_coordinate(c(-79.0, 0.0)).is_err());
        assert!(Ups::try_from_coordinate(c(84.0, 0.0)).is_ok());
        assert!(Ups::try_from_coordinate(c(-80.0, 0.0)).is_ok());
    }
}
