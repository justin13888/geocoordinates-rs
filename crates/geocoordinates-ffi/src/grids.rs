//! Grid systems: Plus Code, geohash, and Maidenhead encodings, the UTM / UPS
//! projections, and MGRS.

use geocoordinates as gc;

use crate::core::approx_coordinate;
use crate::{ApproxCoordinate, Coordinate, GeoError};

// --- Grid systems (Plus Code, geohash, Maidenhead) ---

/// Encode a WGS-84 coordinate to a full Open Location Code of `length`
/// significant digits (2, 4, 6, 8, or 10–15).
///
/// # Errors
/// Returns [`GeoError::CrsMismatch`] for a non-WGS-84 coordinate and
/// [`GeoError::InvalidValue`] for any other `length`.
#[uniffi::export]
pub fn plus_code_encode(coord: Coordinate, length: u8) -> Result<String, GeoError> {
    gc::grids::PlusCode::encode(coord.try_into()?, length)
        .map(String::from)
        .map_err(Into::into)
}

/// Decode an Open Location Code to its WGS-84 cell center, bounded by the
/// largest geodesic distance from the center to a cell corner.
///
/// # Errors
/// Returns a [`GeoError`] for a malformed or short code.
#[uniffi::export]
pub fn plus_code_decode(code: String) -> Result<ApproxCoordinate, GeoError> {
    let pc = gc::grids::PlusCode::parse(&code).map_err(GeoError::from)?;
    Ok(approx_coordinate(pc.decode()))
}

/// Validate a full Open Location Code and return its canonical (trimmed,
/// uppercase) form — the FFI form of `PlusCode::parse` and
/// `Display`.
///
/// # Errors
/// Returns [`GeoError::InvalidGridRef`] for a malformed or short code.
#[uniffi::export]
pub fn plus_code_canonicalize(code: String) -> Result<String, GeoError> {
    gc::grids::PlusCode::parse(&code)
        .map(String::from)
        .map_err(Into::into)
}

/// Encode a WGS-84 coordinate to a geohash of `length` characters (1–12).
///
/// # Errors
/// Returns [`GeoError::CrsMismatch`] for a non-WGS-84 coordinate and
/// [`GeoError::InvalidValue`] for a `length` outside 1–12.
#[uniffi::export]
pub fn geohash_encode(coord: Coordinate, length: u8) -> Result<String, GeoError> {
    gc::grids::Geohash::encode(coord.try_into()?, length)
        .map(String::from)
        .map_err(Into::into)
}

/// Decode a geohash to its WGS-84 cell center, bounded by the largest
/// geodesic distance from the center to a cell corner.
///
/// # Errors
/// Returns a [`GeoError`] for empty, overlong, or non-base-32 input.
#[uniffi::export]
pub fn geohash_decode(code: String) -> Result<ApproxCoordinate, GeoError> {
    let gh = gc::grids::Geohash::parse(&code).map_err(GeoError::from)?;
    Ok(approx_coordinate(gh.decode()))
}

/// Validate a 1–12 character geohash and return its canonical (trimmed,
/// lowercase) form — the FFI form of `Geohash::parse` and
/// `Display`.
///
/// # Errors
/// Returns [`GeoError::InvalidGridRef`] for empty, overlong, or non-base-32
/// input.
#[uniffi::export]
pub fn geohash_canonicalize(code: String) -> Result<String, GeoError> {
    gc::grids::Geohash::parse(&code)
        .map(String::from)
        .map_err(Into::into)
}

/// Encode a WGS-84 coordinate to a Maidenhead locator of `pairs` character
/// pairs (1–3).
///
/// # Errors
/// Returns [`GeoError::CrsMismatch`] for a non-WGS-84 coordinate and
/// [`GeoError::InvalidValue`] for `pairs` outside 1–3.
#[uniffi::export]
pub fn maidenhead_encode(coord: Coordinate, pairs: u8) -> Result<String, GeoError> {
    gc::grids::Maidenhead::encode(coord.try_into()?, pairs)
        .map(String::from)
        .map_err(Into::into)
}

/// Decode a Maidenhead locator to its WGS-84 grid-square center, bounded by
/// the largest geodesic distance from the center to a square corner.
///
/// # Errors
/// Returns a [`GeoError`] for a malformed locator.
#[uniffi::export]
pub fn maidenhead_decode(code: String) -> Result<ApproxCoordinate, GeoError> {
    let mh = gc::grids::Maidenhead::parse(&code).map_err(GeoError::from)?;
    Ok(approx_coordinate(mh.decode()))
}

/// Validate a 2-, 4-, or 6-character Maidenhead locator and return its
/// canonical form (uppercase field, digit square, lowercase subsquare) — the
/// FFI form of `Maidenhead::parse` and `Display`.
///
/// # Errors
/// Returns [`GeoError::InvalidGridRef`] for a malformed locator.
#[uniffi::export]
pub fn maidenhead_canonicalize(code: String) -> Result<String, GeoError> {
    gc::grids::Maidenhead::parse(&code)
        .map(String::from)
        .map_err(Into::into)
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
            easting: u.easting_m(),
            northing: u.northing_m(),
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
            easting: u.easting_m(),
            northing: u.northing_m(),
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
    gc::grids::utm_zone_for(coord.try_into()?).map_err(GeoError::from)
}

/// The central meridian (degrees) of a UTM zone.
///
/// # Errors
/// `GeoError::InvalidValue` (field `zone`) for a zone outside `1..=60`.
#[uniffi::export]
pub fn utm_central_meridian_deg(zone: u8) -> Result<f64, GeoError> {
    gc::grids::utm_central_meridian_deg(zone).map_err(GeoError::from)
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

/// The side of an MGRS square — mirror of [`gc::grids::MgrsPrecision`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MgrsPrecision {
    /// 100 km square: no digits.
    Km100,
    /// 10 km square: one easting and one northing digit.
    Km10,
    /// 1 km square: two digits each.
    Km1,
    /// 100 m square: three digits each.
    M100,
    /// 10 m square: four digits each.
    M10,
    /// 1 m square: five digits each.
    M1,
}

impl From<gc::grids::MgrsPrecision> for MgrsPrecision {
    fn from(p: gc::grids::MgrsPrecision) -> Self {
        match p {
            gc::grids::MgrsPrecision::Km100 => MgrsPrecision::Km100,
            gc::grids::MgrsPrecision::Km10 => MgrsPrecision::Km10,
            gc::grids::MgrsPrecision::Km1 => MgrsPrecision::Km1,
            gc::grids::MgrsPrecision::M100 => MgrsPrecision::M100,
            gc::grids::MgrsPrecision::M10 => MgrsPrecision::M10,
            gc::grids::MgrsPrecision::M1 => MgrsPrecision::M1,
        }
    }
}
impl From<MgrsPrecision> for gc::grids::MgrsPrecision {
    fn from(p: MgrsPrecision) -> Self {
        match p {
            MgrsPrecision::Km100 => gc::grids::MgrsPrecision::Km100,
            MgrsPrecision::Km10 => gc::grids::MgrsPrecision::Km10,
            MgrsPrecision::Km1 => gc::grids::MgrsPrecision::Km1,
            MgrsPrecision::M100 => gc::grids::MgrsPrecision::M100,
            MgrsPrecision::M10 => gc::grids::MgrsPrecision::M10,
            MgrsPrecision::M1 => gc::grids::MgrsPrecision::M1,
        }
    }
}

/// The square's side in meters: 100 000, 10 000, 1 000, 100, 10, or 1.
#[uniffi::export]
pub fn mgrs_precision_meters(precision: MgrsPrecision) -> u32 {
    // The side is an exact power of ten from 1 m to 100 km, so the cast is
    // lossless.
    gc::grids::MgrsPrecision::from(precision).side().meters() as u32
}

/// Encode a coordinate to the canonical string of the MGRS square of the given
/// size that contains it.
///
/// # Errors
/// `GeoError::CrsMismatch` for a non-WGS-84 coordinate.
#[uniffi::export]
pub fn mgrs_from_coordinate(
    coord: Coordinate,
    precision: MgrsPrecision,
) -> Result<String, GeoError> {
    gc::grids::Mgrs::try_from_coordinate(coord.try_into()?, precision.into())
        .map(|mgrs| mgrs.as_str().to_string())
        .map_err(Into::into)
}

/// Decode an MGRS string to the center of its square. The error bound is the
/// square's half diagonal in ground meters, `side / √2 / k_min` (`k_min` is
/// 0.9996 for UTM, 0.994 for UPS), which covers every point of the square.
/// Where the grid-zone designator truncates the square (a zone or band edge,
/// the antimeridian, the Norway / Svalbard exceptions, the UTM / UPS seam),
/// the point is instead one deep inside the part the designator owns, so it
/// encodes back to the same string, and the bound is its distance to the
/// square's farthest corner over `k_min`.
///
/// # Errors
/// `GeoError::InvalidGridRef` when the string is not a valid MGRS reference.
#[uniffi::export]
pub fn mgrs_to_coordinate(mgrs: String) -> Result<ApproxCoordinate, GeoError> {
    let parsed = gc::grids::Mgrs::parse(&mgrs).map_err(GeoError::from)?;
    Ok(approx_coordinate(parsed.to_coordinate()))
}

/// The square size implied by an MGRS string's digit count.
///
/// # Errors
/// `GeoError::InvalidGridRef` when the string is not a valid MGRS reference.
#[uniffi::export]
pub fn mgrs_precision(mgrs: String) -> Result<MgrsPrecision, GeoError> {
    gc::grids::Mgrs::parse(&mgrs)
        .map(|m| m.precision().into())
        .map_err(GeoError::from)
}

/// Validate an MGRS reference and return its canonical form (uppercase, no
/// whitespace, two-digit zone) — the FFI form of `Mgrs::parse` and `Display`.
///
/// # Errors
/// `GeoError::InvalidGridRef` when the string is not a valid MGRS reference.
#[uniffi::export]
pub fn mgrs_canonicalize(mgrs: String) -> Result<String, GeoError> {
    gc::grids::Mgrs::parse(&mgrs)
        .map(String::from)
        .map_err(GeoError::from)
}
