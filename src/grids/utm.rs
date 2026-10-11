//! UTM (Universal Transverse Mercator) and UPS (Universal Polar Stereographic).
//!
//! Both projections are deterministic and invertible to well under a
//! millimeter, so they return bare types rather than [`Approx`](crate::Approx)
//! — but they are **not** exact. Constructing from a coordinate can fail (UTM
//! is undefined at the poles, where UPS is used instead), so the lat/lon → grid
//! direction uses [`TryFrom`].
//!
//! The transverse Mercator forward/inverse use the Karney–Krüger `n`-series
//! truncated at 4th order: sub-millimeter throughout a UTM zone (a round trip
//! is stable to a few micrometers). The UPS forward is the closed-form
//! conformal polar stereographic projection; its inverse recovers latitude
//! from the conformal latitude with a series truncated at `e⁶`, accurate to
//! about 0.05 mm.

use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// UTM scale factor on the central meridian — the minimum point scale factor
/// anywhere in the projection (k grows away from the central meridian).
pub(crate) const K0: f64 = 0.9996;
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
        coord.validate()?;
        if coord.crs != crate::Crs::Wgs84 {
            return Err(Error::CrsMismatch {
                expected: crate::Crs::Wgs84,
                found: coord.crs,
            });
        }
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

    /// UTM → geodetic WGS-84 coordinate, by the 4th-order Karney–Krüger
    /// inverse series (sub-millimeter within the UTM domain; not exact).
    pub fn try_to_coordinate(self) -> Result<Coordinate> {
        if !(1..=60).contains(&self.zone)
            || !self.easting.is_finite()
            || !(100_000.0..=900_000.0).contains(&self.easting)
            || !self.northing.is_finite()
            || !(0.0..=10_000_000.0).contains(&self.northing)
        {
            return Err(Error::InvalidValue {
                field: "UTM",
                detail: "zone/easting/northing are outside the UTM domain".into(),
            });
        }
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
        let result = Coordinate::wgs84(lat.to_degrees(), lon.to_degrees());
        result.validate()?;
        Ok(result)
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
        coord.validate()?;
        if coord.crs != crate::Crs::Wgs84 {
            return Err(Error::CrsMismatch {
                expected: crate::Crs::Wgs84,
                found: coord.crs,
            });
        }
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

    /// UPS → geodetic WGS-84 coordinate, by the inverse polar stereographic
    /// projection. Latitude comes from a conformal-latitude series truncated at
    /// `e⁶`, accurate to about 0.05 mm; not exact.
    pub fn try_to_coordinate(self) -> Result<Coordinate> {
        if !self.easting.is_finite()
            || !(0.0..=4_000_000.0).contains(&self.easting)
            || !self.northing.is_finite()
            || !(0.0..=4_000_000.0).contains(&self.northing)
        {
            return Err(Error::InvalidValue {
                field: "UPS",
                detail: "easting/northing are outside the UPS domain".into(),
            });
        }
        let north = self.hemisphere == Hemisphere::North;
        let (lat, lon) = ups_inverse(self.easting, self.northing, north);
        let result = Coordinate::wgs84(lat, lon);
        result.validate()?;
        Ok(result)
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
/// UPS central scale factor — the point scale factor at the pole, and the
/// minimum anywhere in the projection (k grows away from the pole).
pub(crate) const UPS_K0: f64 = 0.994;

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
mod tests;
