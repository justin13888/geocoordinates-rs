//! Grid systems: Plus Code, geohash, and Maidenhead encodings, the UTM / UPS
//! projections, and MGRS.

use geocoordinates as gc;

use crate::core::approx_coordinate;
use crate::{ApproxCoordinate, Coordinate, GeoError};

// --- Grid systems (Plus Code, geohash, Maidenhead) ---

/// Encode a coordinate to an Open Location Code at a valid length.
#[uniffi::export]
pub fn plus_code_encode(coord: Coordinate, length: u8) -> Result<String, GeoError> {
    gc::grids::PlusCode::encode(coord.into(), usize::from(length))
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
    gc::grids::Geohash::encode(coord.into(), usize::from(length))
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
    gc::grids::Maidenhead::encode(coord.into(), usize::from(pairs))
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
/// [`gc::grids::utm::Hemisphere`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum UtmHemisphere {
    /// Northern hemisphere.
    North,
    /// Southern hemisphere.
    South,
}

impl From<gc::grids::utm::Hemisphere> for UtmHemisphere {
    fn from(h: gc::grids::utm::Hemisphere) -> Self {
        match h {
            gc::grids::utm::Hemisphere::North => UtmHemisphere::North,
            gc::grids::utm::Hemisphere::South => UtmHemisphere::South,
        }
    }
}
impl From<UtmHemisphere> for gc::grids::utm::Hemisphere {
    fn from(h: UtmHemisphere) -> Self {
        match h {
            UtmHemisphere::North => gc::grids::utm::Hemisphere::North,
            UtmHemisphere::South => gc::grids::utm::Hemisphere::South,
        }
    }
}

/// A UTM coordinate — mirror of [`gc::Utm`](gc::grids::Utm).
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
            zone: u.zone,
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}
impl From<Utm> for gc::grids::Utm {
    fn from(u: Utm) -> Self {
        gc::grids::Utm {
            zone: u.zone,
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}
impl From<gc::grids::Ups> for Ups {
    fn from(u: gc::grids::Ups) -> Self {
        Ups {
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}
impl From<Ups> for gc::grids::Ups {
    fn from(u: Ups) -> Self {
        gc::grids::Ups {
            hemisphere: u.hemisphere.into(),
            easting: u.easting,
            northing: u.northing,
        }
    }
}

/// Geodetic → UTM. Errors in the polar regions (use UPS there).
#[uniffi::export]
pub fn utm_from_coordinate(coord: Coordinate) -> Result<Utm, GeoError> {
    gc::grids::Utm::try_from_coordinate(coord.into())
        .map(Into::into)
        .map_err(GeoError::from)
}

/// UTM → geodetic WGS-84 coordinate, by the 4th-order Karney–Krüger inverse
/// series (sub-millimeter within the UTM domain; not exact).
#[uniffi::export]
pub fn utm_to_coordinate(utm: Utm) -> Result<Coordinate, GeoError> {
    gc::grids::Utm::from(utm)
        .try_to_coordinate()
        .map(Into::into)
        .map_err(Into::into)
}

/// Geodetic → UPS. Errors outside the polar regions (use UTM there).
#[uniffi::export]
pub fn ups_from_coordinate(coord: Coordinate) -> Result<Ups, GeoError> {
    gc::grids::Ups::try_from_coordinate(coord.into())
        .map(Into::into)
        .map_err(GeoError::from)
}

/// UPS → geodetic WGS-84 coordinate, by the inverse polar stereographic
/// projection with a latitude series truncated at `e⁶` (about 0.05 mm; not
/// exact).
#[uniffi::export]
pub fn ups_to_coordinate(ups: Ups) -> Result<Coordinate, GeoError> {
    gc::grids::Ups::from(ups)
        .try_to_coordinate()
        .map(Into::into)
        .map_err(Into::into)
}

/// Encode a coordinate to an MGRS string at the given precision in meters
/// (1 m … 100 km, as a power of ten).
#[uniffi::export]
pub fn mgrs_from_coordinate(coord: Coordinate, precision_m: u32) -> Result<String, GeoError> {
    gc::grids::Mgrs::try_from_coordinate(coord.into(), precision_m)
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
