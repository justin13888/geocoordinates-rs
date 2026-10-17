//! UTM (Universal Transverse Mercator) and UPS (Universal Polar Stereographic).
//!
//! Both projections are deterministic and invertible to well under a
//! millimeter, so they return bare types rather than [`Approx`](crate::Approx)
//! — but they are **not** exact.
//!
//! [`Utm`] and [`Ups`] are valid by construction: [`Utm::new`] / [`Ups::new`]
//! reject a zone, easting, or northing outside the projection's domain, so the
//! grid → lat/lon direction ([`Utm::to_coordinate`], [`Ups::to_coordinate`]) is
//! infallible and always yields a wrapped, in-range longitude. The lat/lon →
//! grid direction can fail (UTM is undefined at the poles, where UPS is used
//! instead), so it returns [`Result`] with [`Error::OutsideDomain`].
//!
//! The transverse Mercator forward/inverse use the Karney–Krüger `n`-series
//! truncated at 4th order: sub-millimeter throughout a UTM zone (a round trip
//! is stable to a few micrometers). The UPS forward is the closed-form
//! conformal polar stereographic projection; its inverse recovers latitude
//! from the conformal latitude with a series truncated at `e⁶`, accurate to
//! about 0.05 mm.

use crate::angle::{clamp_latitude, wrap_longitude};
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::geodesy::Ellipsoid;
use crate::geodesy::series::KrugerSeries;
use crate::grids::require_wgs84;

mod ups;

pub(crate) use ups::UPS_K0;
pub use ups::Ups;

/// UTM scale factor on the central meridian — the minimum point scale factor
/// anywhere in the projection (k grows away from the central meridian).
pub(crate) const K0: f64 = 0.9996;
/// UTM false easting (meters).
const FALSE_EASTING: f64 = 500_000.0;
/// UTM false northing applied in the southern hemisphere (meters).
pub(crate) const FALSE_NORTHING_SOUTH: f64 = 10_000_000.0;

// --- WGS-84 transverse Mercator (Karney–Krüger n-series) constants ---
const A_AXIS: f64 = Ellipsoid::WGS84.semi_major().meters();
const F: f64 = Ellipsoid::WGS84.flattening();
/// The WGS-84 Krüger series (α, β, δ, rectifying radius), shared with the
/// ellipsoidal rhumb lines.
const SERIES: KrugerSeries = KrugerSeries::WGS84;
/// Rectifying radius `A`, in meters.
const RECT_A: f64 = SERIES.rect_a;

/// Smallest valid UTM easting (meters).
const MIN_EASTING: f64 = 100_000.0;
/// Largest valid UTM easting (meters).
const MAX_EASTING: f64 = 900_000.0;
/// Grid northing of the pole measured from the equator: the quarter meridian
/// `A·π/2` scaled by `K0` (≈ 9 997 964.94 m). A northern northing beyond it (or
/// a southern one below `FALSE_NORTHING_SOUTH` minus it) would fold over the
/// pole onto the far meridian.
const POLE_NORTHING: f64 = K0 * RECT_A * core::f64::consts::FRAC_PI_2;

/// Northern or southern hemisphere of a [`Utm`] or [`Ups`] coordinate.
///
/// Distinct from [`angle::Hemisphere`](crate::angle::Hemisphere), which names
/// one of four cardinal directions on a single angle.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::grids::{Utm, UtmHemisphere};
///
/// let sydney = Utm::try_from_coordinate(Coordinate::wgs84(-33.8688, 151.2093)?)?;
/// assert_eq!(sydney.hemisphere(), UtmHemisphere::South);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UtmHemisphere {
    /// Northern hemisphere.
    North,
    /// Southern hemisphere.
    South,
}

/// A UTM coordinate: zone, hemisphere, easting, and northing.
///
/// Valid by construction — build one with [`Utm::new`] or
/// [`Utm::try_from_coordinate`]. The domain is zone `1..=60`, easting
/// `100 000..=900 000` m, and a northing that does not fold over the pole:
/// from 0 up to the pole's grid northing (≈ 9 997 965 m) in the north, and from
/// 10 000 000 m minus that (≈ 2 035 m) up to 10 000 000 m in the south.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::grids::Utm;
///
/// let nyc = Coordinate::wgs84(40.7128, -74.006)?;
/// let back = Utm::try_from_coordinate(nyc)?.to_coordinate();
/// assert!((back.lat() - 40.7128).abs() < 1e-9 && (back.lon() + 74.006).abs() < 1e-9);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "UtmRecord"))]
pub struct Utm {
    zone: u8,
    hemisphere: UtmHemisphere,
    easting: f64,
    northing: f64,
}

/// Unvalidated serde shape of [`Utm`]; deserialization goes through
/// [`Utm::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct UtmRecord {
    zone: u8,
    hemisphere: UtmHemisphere,
    easting: f64,
    northing: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<UtmRecord> for Utm {
    type Error = Error;

    fn try_from(r: UtmRecord) -> Result<Self> {
        Utm::new(r.zone, r.hemisphere, r.easting, r.northing)
    }
}

/// `InvalidValue` for `field` unless `value` lies in `lo..=hi` (which also
/// rejects NaN and the infinities).
fn check_range(field: &'static str, value: f64, lo: f64, hi: f64) -> Result<()> {
    if (lo..=hi).contains(&value) {
        Ok(())
    } else {
        Err(Error::InvalidValue {
            field,
            detail: format!("{value} is outside {lo}..={hi} m"),
        })
    }
}

/// `InvalidValue { field: "zone" }` unless `zone` is in `1..=60`.
fn check_zone(zone: u8) -> Result<()> {
    if (1..=60).contains(&zone) {
        Ok(())
    } else {
        Err(Error::InvalidValue {
            field: "zone",
            detail: format!("UTM zone {zone} is outside 1..=60"),
        })
    }
}

/// The UTM longitude zone for a WGS-84 coordinate, applying the Norway and
/// Svalbard exceptions.
///
/// ```
/// use geocoordinates::grids::utm_zone_for;
/// use geocoordinates::{Coordinate, Error};
///
/// assert_eq!(utm_zone_for(&Coordinate::wgs84(40.7128, -74.006)?)?, 18);
/// assert_eq!(utm_zone_for(&Coordinate::wgs84(60.0, 5.0)?)?, 32); // Norway exception
/// let pole = Coordinate::wgs84(85.0, 0.0)?;
/// assert!(matches!(utm_zone_for(&pole), Err(Error::OutsideDomain { .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
///
/// # Errors
/// [`Error::CrsMismatch`] for a non-WGS-84 coordinate;
/// [`Error::OutsideDomain`] (operation `"UTM"`) for a latitude outside the
/// UTM band `[-80, 84)` — 84°N itself is UPS territory.
pub fn utm_zone_for(coord: &Coordinate) -> Result<u8> {
    require_wgs84(coord)?;
    let (lat, lon) = (coord.lat(), coord.lon());
    if !(-80.0..84.0).contains(&lat) {
        return Err(Error::OutsideDomain {
            operation: "UTM",
            lat,
            lon,
        });
    }
    Ok(zone_in_band(lat, lon))
}

/// [`utm_zone_for`] on a latitude already inside the UTM band.
fn zone_in_band(lat: f64, lon: f64) -> u8 {
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

/// The longitudes `[west, east)` that [`zone_in_band`] assigns to `zone` at a
/// UTM-band latitude, the Norway and Svalbard exceptions included; `None`
/// where the Svalbard exception drops the zone (32, 34 and 36 above 72°N).
/// Zone 60 also takes 180° itself.
pub(crate) fn zone_span(zone: u8, lat: f64) -> Option<(f64, f64)> {
    let west = f64::from(zone) * 6.0 - 186.0;
    if (56.0..64.0).contains(&lat) {
        match zone {
            31 => return Some((0.0, 3.0)),
            32 => return Some((3.0, 12.0)),
            _ => {}
        }
    }
    if (72.0..84.0).contains(&lat) {
        match zone {
            31 => return Some((0.0, 9.0)),
            32 | 34 | 36 => return None,
            33 => return Some((9.0, 21.0)),
            35 => return Some((21.0, 33.0)),
            37 => return Some((33.0, 42.0)),
            _ => {}
        }
    }
    Some((west, west + 6.0))
}

/// The central meridian (degrees) of a UTM zone.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::grids::utm_central_meridian_deg;
///
/// assert_eq!(utm_central_meridian_deg(18)?, -75.0);
/// assert!(matches!(utm_central_meridian_deg(61), Err(Error::InvalidValue { .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
///
/// # Errors
/// [`Error::InvalidValue`] (field `"zone"`) for a zone outside `1..=60`.
pub fn utm_central_meridian_deg(zone: u8) -> Result<f64> {
    check_zone(zone)?;
    Ok(central_meridian(zone))
}

/// [`utm_central_meridian_deg`] for a zone already known to be in `1..=60`.
pub(crate) fn central_meridian(zone: u8) -> f64 {
    f64::from(zone) * 6.0 - 183.0
}

impl Utm {
    /// Build a UTM coordinate from its parts, validating each against the UTM
    /// domain.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::grids::{Utm, UtmHemisphere};
    ///
    /// let utm = Utm::new(18, UtmHemisphere::North, 583_959.37, 4_507_351.0)?;
    /// assert!((utm.to_coordinate().lat() - 40.7128).abs() < 1e-6);
    /// let err = Utm::new(18, UtmHemisphere::North, 950_000.0, 4_507_351.0);
    /// assert!(matches!(err, Err(Error::InvalidValue { field: "easting", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// [`Error::InvalidValue`] naming the offending field: `"zone"` outside
    /// `1..=60`; `"easting"` outside `100 000..=900 000` m; `"northing"`
    /// outside 0 to ≈ 9 997 965 m (north) or ≈ 2 035 to 10 000 000 m
    /// (south) — beyond those the inverse would fold over the pole. NaN and
    /// the infinities are rejected.
    pub fn new(zone: u8, hemisphere: UtmHemisphere, easting: f64, northing: f64) -> Result<Utm> {
        check_zone(zone)?;
        check_range("easting", easting, MIN_EASTING, MAX_EASTING)?;
        let (lo, hi) = match hemisphere {
            UtmHemisphere::North => (0.0, POLE_NORTHING),
            UtmHemisphere::South => (FALSE_NORTHING_SOUTH - POLE_NORTHING, FALSE_NORTHING_SOUTH),
        };
        check_range("northing", northing, lo, hi)?;
        Ok(Utm {
            zone,
            hemisphere,
            easting,
            northing,
        })
    }

    /// Longitude zone number, `1..=60`.
    ///
    /// ```
    /// use geocoordinates::grids::{Utm, UtmHemisphere};
    ///
    /// assert_eq!(Utm::new(33, UtmHemisphere::North, 500_000.0, 0.0)?.zone(), 33);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn zone(&self) -> u8 {
        self.zone
    }

    /// Hemisphere band.
    ///
    /// ```
    /// use geocoordinates::grids::{Utm, UtmHemisphere};
    ///
    /// let utm = Utm::new(56, UtmHemisphere::South, 334_000.0, 6_252_000.0)?;
    /// assert_eq!(utm.hemisphere(), UtmHemisphere::South);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn hemisphere(&self) -> UtmHemisphere {
        self.hemisphere
    }

    /// Easting in meters (500 000 m false easting applied).
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::grids::Utm;
    ///
    /// // A point on zone 18's central meridian (75°W) sits at the false easting.
    /// let utm = Utm::try_from_coordinate(Coordinate::wgs84(40.0, -75.0)?)?;
    /// assert!((utm.easting_m() - 500_000.0).abs() < 1e-6);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn easting_m(&self) -> f64 {
        self.easting
    }

    /// Northing in meters (10 000 000 m false northing applied in the south).
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::grids::Utm;
    ///
    /// // Just south of the equator, the false northing keeps it positive.
    /// let utm = Utm::try_from_coordinate(Coordinate::wgs84(-0.001, -75.0)?)?;
    /// assert!((9_999_800.0..10_000_000.0).contains(&utm.northing_m()));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn northing_m(&self) -> f64 {
        self.northing
    }

    /// Geodetic WGS-84 → UTM. Fails in the polar regions (use [`Ups`] there).
    ///
    /// The canonical inherent form behind [`TryFrom<Coordinate>`](Utm).
    ///
    /// ```
    /// use geocoordinates::grids::{Utm, UtmHemisphere};
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let utm = Utm::try_from_coordinate(Coordinate::wgs84(40.7128, -74.006)?)?;
    /// assert_eq!((utm.zone(), utm.hemisphere()), (18, UtmHemisphere::North));
    /// assert!((utm.easting_m() - 583_959.372).abs() < 1e-3);
    /// assert!((utm.northing_m() - 4_507_350.998).abs() < 1e-3);
    /// let pole = Utm::try_from_coordinate(Coordinate::wgs84(84.0, 0.0)?);
    /// assert!(matches!(pole, Err(Error::OutsideDomain { operation: "UTM", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// [`Error::CrsMismatch`] for a non-WGS-84 coordinate;
    /// [`Error::OutsideDomain`] (operation `"UTM"`) for latitudes outside the
    /// UTM band (south of 80°S, or at or north of 84°N).
    pub fn try_from_coordinate(coord: Coordinate) -> Result<Utm> {
        let zone = utm_zone_for(&coord)?;
        let (lat, lon) = (coord.lat(), coord.lon());
        let lon0 = central_meridian(zone);
        let phi = lat.to_radians();
        let d_lon = (lon - lon0).to_radians();

        let t = SERIES.conformal_tan(phi);
        let xi_p = t.atan2(d_lon.cos());
        let eta_p = (d_lon.sin() / (1.0 + t * t).sqrt()).atanh();

        let (xi, eta) = SERIES.tm_from_conformal(xi_p, eta_p);

        let easting = FALSE_EASTING + K0 * RECT_A * eta;
        let mut northing = K0 * RECT_A * xi;
        let hemisphere = if lat < 0.0 {
            northing += FALSE_NORTHING_SOUTH;
            UtmHemisphere::South
        } else {
            UtmHemisphere::North
        };
        // The UTM band keeps every forward result inside the `Utm::new`
        // domain (|Δλ| ≤ 6° only where cos φ ≤ 0.56, and |φ| < 84°).
        Ok(Utm {
            zone,
            hemisphere,
            easting,
            northing,
        })
    }

    /// UTM → geodetic WGS-84 coordinate, by the 4th-order Karney–Krüger
    /// inverse series (sub-millimeter within the UTM domain; not exact).
    ///
    /// Infallible: the domain was validated at construction. The longitude is
    /// wrapped into `[-180, 180)`, so a zone 1 easting west of the
    /// antimeridian decodes to about +179.9° rather than −180.1°.
    ///
    /// ```
    /// use geocoordinates::grids::{Utm, UtmHemisphere};
    ///
    /// // Zone 1's western edge reaches past the antimeridian; it wraps east.
    /// let west = Utm::new(1, UtmHemisphere::North, 100_000.0, 0.0)?.to_coordinate();
    /// assert!(west.lon() > 179.0 && west.lat().abs() < 1e-9);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_coordinate(&self) -> Coordinate {
        let lon0 = central_meridian(self.zone);
        let n0 = match self.hemisphere {
            UtmHemisphere::North => 0.0,
            UtmHemisphere::South => FALSE_NORTHING_SOUTH,
        };
        let xi = (self.northing - n0) / (K0 * RECT_A);
        let eta = (self.easting - FALSE_EASTING) / (K0 * RECT_A);

        let (xi_p, eta_p) = SERIES.conformal_from_tm(xi, eta);

        let chi = (xi_p.sin() / eta_p.cosh()).asin();
        let lat = SERIES.geodetic_from_conformal(chi);
        let lon = lon0 + eta_p.sinh().atan2(xi_p.cos()).to_degrees();
        Coordinate::from_valid(
            clamp_latitude(lat.to_degrees()),
            wrap_longitude(lon),
            None,
            Crs::Wgs84,
        )
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

#[cfg(test)]
mod tests;
