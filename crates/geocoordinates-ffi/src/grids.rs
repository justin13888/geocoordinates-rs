//! Grid systems: Plus Code, geohash, and Maidenhead encodings, the UTM / UPS
//! projections, and MGRS.

use geocoordinates as gc;

use crate::core::approx_coordinate;
use crate::{ApproxCoordinate, Coordinate, GeoError};

// --- Grid systems (Plus Code, geohash, Maidenhead) ---

/// Encode a coordinate to an Open Location Code at a valid length.
#[uniffi::export]
pub fn plus_code_encode(coord: Coordinate, length: u8) -> Result<String, GeoError> {
    gc::grids::PlusCode::encode(coord.try_into()?, usize::from(length))
        .map(|code| code.as_str().to_string())
        .map_err(Into::into)
}

/// Decode an Open Location Code to its WGS-84 cell center, with the cell
/// half-diagonal error bound.
///
/// # Errors
/// Returns a [`GeoError`] for a malformed or short code.
#[uniffi::export]
pub fn plus_code_decode(code: String) -> Result<ApproxCoordinate, GeoError> {
    let pc = gc::grids::PlusCode::try_from(code.as_str()).map_err(GeoError::from)?;
    Ok(approx_coordinate(pc.decode()))
}

/// Encode a coordinate to a geohash of the given character length.
#[uniffi::export]
pub fn geohash_encode(coord: Coordinate, length: u8) -> Result<String, GeoError> {
    gc::grids::Geohash::encode(coord.try_into()?, usize::from(length))
        .map(|code| code.as_str().to_string())
        .map_err(Into::into)
}

/// Decode a geohash to its WGS-84 cell center, with the cell half-diagonal
/// error bound.
///
/// # Errors
/// Returns a [`GeoError`] for non-base-32 input.
#[uniffi::export]
pub fn geohash_decode(code: String) -> Result<ApproxCoordinate, GeoError> {
    let gh = gc::grids::Geohash::try_from(code.as_str()).map_err(GeoError::from)?;
    Ok(approx_coordinate(gh.decode()))
}

/// Encode a coordinate to a Maidenhead locator of the given number of pairs
/// (1–3).
#[uniffi::export]
pub fn maidenhead_encode(coord: Coordinate, pairs: u8) -> Result<String, GeoError> {
    gc::grids::Maidenhead::encode(coord.try_into()?, usize::from(pairs))
        .map(|code| code.as_str().to_string())
        .map_err(Into::into)
}

/// Decode a Maidenhead locator to its WGS-84 grid-square center, with the
/// square half-diagonal error bound.
///
/// # Errors
/// Returns a [`GeoError`] for a malformed locator.
#[uniffi::export]
pub fn maidenhead_decode(code: String) -> Result<ApproxCoordinate, GeoError> {
    let mh = gc::grids::Maidenhead::try_from(code.as_str()).map_err(GeoError::from)?;
    Ok(approx_coordinate(mh.decode()))
}

// ===========================================================================
// UTM / UPS projections and MGRS
// ===========================================================================

/// Northern or southern band for a UTM/UPS coordinate — mirror of
/// [`gc::grids::UtmHemisphere`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum UtmHemisphere {
    /// Northern hemisphere.
    North,
    /// Southern hemisphere.
    South,
}

impl From<gc::grids::UtmHemisphere> for UtmHemisphere {
    fn from(h: gc::grids::UtmHemisphere) -> Self {
        match h {
            gc::grids::UtmHemisphere::North => UtmHemisphere::North,
            gc::grids::UtmHemisphere::South => UtmHemisphere::South,
        }
    }
}
impl From<UtmHemisphere> for gc::grids::UtmHemisphere {
    fn from(h: UtmHemisphere) -> Self {
        match h {
            UtmHemisphere::North => gc::grids::UtmHemisphere::North,
            UtmHemisphere::South => gc::grids::UtmHemisphere::South,
        }
    }
}

/// A UTM coordinate — mirror of [`gc::Utm`](gc::grids::Utm).
///
/// A record crosses the boundary unvalidated; every export that accepts one
/// validates it as [`gc::grids::Utm::new`] does (zone `1..=60`, easting
/// `100 000..=900 000` m, a northing that does not fold over the pole).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Utm {
    /// Longitude zone number, 1–60.
    pub zone: u8,
    /// Hemisphere band.
    pub hemisphere: UtmHemisphere,
    /// Easting in meters (false-easting applied).
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

/// A UPS (polar) coordinate — mirror of [`gc::Ups`](gc::grids::Ups).
///
/// A record crosses the boundary unvalidated; every export that accepts one
/// validates it as [`gc::grids::Ups::new`] does (easting and northing each in
/// `0..=4 000 000` m).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ups {
    /// North or south polar zone.
    pub hemisphere: UtmHemisphere,
    /// Easting in meters.
    pub easting: f64,
    /// Northing in meters.
    pub northing: f64,
}

impl From<gc::grids::Utm> for Utm {
    fn from(u: gc::grids::Utm) -> Self {
        Utm {
            zone: u.zone(),
            hemisphere: u.hemisphere().into(),
            easting: u.easting(),
            northing: u.northing(),
        }
    }
}
impl TryFrom<Utm> for gc::grids::Utm {
    type Error = GeoError;

    fn try_from(u: Utm) -> Result<Self, GeoError> {
        gc::grids::Utm::new(u.zone, u.hemisphere.into(), u.easting, u.northing)
            .map_err(GeoError::from)
    }
}
impl From<gc::grids::Ups> for Ups {
    fn from(u: gc::grids::Ups) -> Self {
        Ups {
            hemisphere: u.hemisphere().into(),
            easting: u.easting(),
            northing: u.northing(),
        }
    }
}
impl TryFrom<Ups> for gc::grids::Ups {
    type Error = GeoError;

    fn try_from(u: Ups) -> Result<Self, GeoError> {
        gc::grids::Ups::new(u.hemisphere.into(), u.easting, u.northing).map_err(GeoError::from)
    }
}

/// Geodetic → UTM. Errors in the polar regions (use UPS there).
///
/// # Errors
/// `GeoError::OutsideDomain` (operation `"UTM"`) outside the 80°S–84°N band;
/// `GeoError::CrsMismatch` for a non-WGS-84 coordinate.
#[uniffi::export]
pub fn utm_from_coordinate(coord: Coordinate) -> Result<Utm, GeoError> {
    gc::grids::Utm::try_from_coordinate(coord.try_into()?)
        .map(Into::into)
        .map_err(GeoError::from)
}

/// UTM → geodetic WGS-84 coordinate, by the 4th-order Karney–Krüger inverse
/// series (sub-millimeter within the UTM domain; not exact). The longitude is
/// wrapped into `[-180, 180)`.
///
/// # Errors
/// `GeoError::InvalidValue` naming the field (`zone`, `easting`, or
/// `northing`) when the record is outside the UTM domain.
#[uniffi::export]
pub fn utm_to_coordinate(utm: Utm) -> Result<Coordinate, GeoError> {
    let utm = gc::grids::Utm::try_from(utm)?;
    Ok(utm.to_coordinate().into())
}

/// The UTM longitude zone for a WGS-84 coordinate, applying the Norway and
/// Svalbard exceptions.
///
/// # Errors
/// `GeoError::OutsideDomain` (operation `"UTM"`) outside `[-80, 84)` latitude;
/// `GeoError::CrsMismatch` for a non-WGS-84 coordinate.
#[uniffi::export]
pub fn utm_zone_for(coord: Coordinate) -> Result<u8, GeoError> {
    gc::grids::utm::zone_for(&coord.try_into()?).map_err(GeoError::from)
}

/// The central meridian (degrees) of a UTM zone.
///
/// # Errors
/// `GeoError::InvalidValue` (field `zone`) for a zone outside `1..=60`.
#[uniffi::export]
pub fn utm_central_meridian_deg(zone: u8) -> Result<f64, GeoError> {
    gc::grids::utm::central_meridian_deg(zone).map_err(GeoError::from)
}

/// Geodetic → UPS. Errors outside the polar regions (use UTM there).
///
/// # Errors
/// `GeoError::OutsideDomain` (operation `"UPS"`) strictly between 80°S and
/// 84°N; `GeoError::CrsMismatch` for a non-WGS-84 coordinate.
#[uniffi::export]
pub fn ups_from_coordinate(coord: Coordinate) -> Result<Ups, GeoError> {
    gc::grids::Ups::try_from_coordinate(coord.try_into()?)
        .map(Into::into)
        .map_err(GeoError::from)
}

/// UPS → geodetic WGS-84 coordinate, by the inverse polar stereographic
/// projection with a latitude series truncated at `e⁶` (about 0.05 mm; not
/// exact). The longitude is wrapped into `[-180, 180)`.
///
/// # Errors
/// `GeoError::InvalidValue` naming the field (`easting` or `northing`) when
/// the record is outside the UPS domain.
#[uniffi::export]
pub fn ups_to_coordinate(ups: Ups) -> Result<Coordinate, GeoError> {
    let ups = gc::grids::Ups::try_from(ups)?;
    Ok(ups.to_coordinate().into())
}

/// Encode a coordinate to an MGRS string at the given precision in meters
/// (1 m … 100 km, as a power of ten).
#[uniffi::export]
pub fn mgrs_from_coordinate(coord: Coordinate, precision_m: u32) -> Result<String, GeoError> {
    gc::grids::Mgrs::try_from_coordinate(coord.try_into()?, precision_m)
        .map(|mgrs| mgrs.as_str().to_string())
        .map_err(Into::into)
}

/// Decode an MGRS string to the center of its square, with the half-square
/// error bound.
///
/// # Errors
/// `GeoError` when the string is not a valid MGRS reference.
#[uniffi::export]
pub fn mgrs_to_coordinate(mgrs: String) -> Result<ApproxCoordinate, GeoError> {
    let parsed = gc::grids::Mgrs::try_from(mgrs.as_str()).map_err(GeoError::from)?;
    Ok(approx_coordinate(parsed.to_coordinate()))
}

/// The precision in meters implied by an MGRS string's digit count.
///
/// # Errors
/// `GeoError` when the string is not a valid MGRS reference.
#[uniffi::export]
pub fn mgrs_precision_m(mgrs: String) -> Result<u32, GeoError> {
    gc::grids::Mgrs::try_from(mgrs.as_str())
        .map(|m| m.precision_m())
        .map_err(GeoError::from)
}
