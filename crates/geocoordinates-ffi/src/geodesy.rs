//! Geodesy: ellipsoids, ECEF and local tangent frames, distances and bearings,
//! with the cross/along-track and intersection routines in `track` and the
//! classic-datum Helmert transforms in `datum`.

use geocoordinates as gc;

use crate::{Coordinate, Crs, GeoError};

mod track; // cross/along-track and intersection, spherical and ellipsoidal
pub use track::*;

/// Cheap spherical (haversine) distance between two coordinates, in **meters**.
///
/// `Length` and its operators do not cross the FFI boundary, so the scalar
/// meters value is returned directly.
///
/// # Errors
/// `GeoError::CrsMismatch` when the two coordinates are in different
/// reference systems; `GeoError::OutOfRange` / `GeoError::InvalidValue` for an
/// invalid coordinate record.
#[uniffi::export]
pub fn haversine_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let a: gc::Coordinate = a.try_into()?;
    let b: gc::Coordinate = b.try_into()?;
    gc::geodesy::haversine_distance(a, b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

// ===========================================================================
// Geodesy: ellipsoids, ECEF, and local tangent frames
// ===========================================================================

/// A reference ellipsoid — mirror of [`gc::Ellipsoid`](gc::geodesy::Ellipsoid).
///
/// Unvalidated until used: every export that takes one validates it as the
/// core `Ellipsoid::new` does (semi-major axis finite and positive, inverse
/// flattening greater than one or positive infinity). Build one with
/// [`ellipsoid_new`] to fail at construction instead, or call
/// [`ellipsoid_validate`] on a hand-built record.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ellipsoid {
    /// Semi-major axis `a`, in meters.
    pub semi_major_m: f64,
    /// Inverse flattening `1/f`.
    pub inverse_flattening: f64,
}

/// A geocentric ECEF position in meters — mirror of [`gc::Ecef`](gc::geodesy::Ecef).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ecef {
    /// X axis (meters), through the prime meridian at the equator.
    pub x: f64,
    /// Y axis (meters), 90° east at the equator.
    pub y: f64,
    /// Z axis (meters), through the north pole.
    pub z: f64,
}

/// East-North-Up offset (meters) — mirror of [`gc::Enu`](gc::geodesy::Enu).
///
/// Unvalidated until used: every export that takes one validates it as the
/// core `Enu::new` does (each component finite, and the offset's length
/// finite). Call [`enu_validate`] to check a hand-built record up front.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Enu {
    /// East offset (meters).
    pub east: f64,
    /// North offset (meters).
    pub north: f64,
    /// Up offset (meters).
    pub up: f64,
}

/// North-East-Down offset (meters) — mirror of [`gc::Ned`](gc::geodesy::Ned).
///
/// Unvalidated until used, as [`Enu`]; call [`ned_validate`] to check a
/// hand-built record up front.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ned {
    /// North offset (meters).
    pub north: f64,
    /// East offset (meters).
    pub east: f64,
    /// Down offset (meters).
    pub down: f64,
}

/// Azimuth-Elevation-Range — mirror of [`gc::Aer`](gc::geodesy::Aer), with the
/// slant range flattened to meters.
///
/// Unvalidated until used: every export that takes one validates it as the
/// core `Aer::new` does (azimuth finite, elevation in `[-90, 90]`, range
/// finite and non-negative). An azimuth outside `[0, 360)` is accepted and
/// normalized. [`aer_validate`] checks a hand-built record; [`aer_new`] builds one.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Aer {
    /// Azimuth (degrees clockwise from north).
    pub azimuth_deg: f64,
    /// Elevation (degrees above the local horizontal).
    pub elevation_deg: f64,
    /// Slant range, in meters.
    pub range_m: f64,
}

impl From<gc::geodesy::Ellipsoid> for Ellipsoid {
    fn from(e: gc::geodesy::Ellipsoid) -> Self {
        Ellipsoid {
            semi_major_m: e.semi_major().meters(),
            inverse_flattening: e.inverse_flattening(),
        }
    }
}
impl TryFrom<Ellipsoid> for gc::geodesy::Ellipsoid {
    type Error = GeoError;

    fn try_from(e: Ellipsoid) -> Result<Self, GeoError> {
        gc::geodesy::Ellipsoid::new(
            gc::Length::from_meters(e.semi_major_m),
            e.inverse_flattening,
        )
        .map_err(GeoError::from)
    }
}
impl From<gc::geodesy::Ecef> for Ecef {
    fn from(c: gc::geodesy::Ecef) -> Self {
        Ecef {
            x: c.x_m,
            y: c.y_m,
            z: c.z_m,
        }
    }
}
impl From<Ecef> for gc::geodesy::Ecef {
    fn from(c: Ecef) -> Self {
        gc::geodesy::Ecef::new(c.x, c.y, c.z)
    }
}
impl From<gc::geodesy::Enu> for Enu {
    fn from(e: gc::geodesy::Enu) -> Self {
        Enu {
            east: e.east_m(),
            north: e.north_m(),
            up: e.up_m(),
        }
    }
}
impl TryFrom<Enu> for gc::geodesy::Enu {
    type Error = GeoError;

    fn try_from(e: Enu) -> Result<Self, GeoError> {
        gc::geodesy::Enu::new(e.east, e.north, e.up).map_err(GeoError::from)
    }
}
impl From<gc::geodesy::Ned> for Ned {
    fn from(n: gc::geodesy::Ned) -> Self {
        Ned {
            north: n.north_m(),
            east: n.east_m(),
            down: n.down_m(),
        }
    }
}
impl TryFrom<Ned> for gc::geodesy::Ned {
    type Error = GeoError;

    fn try_from(n: Ned) -> Result<Self, GeoError> {
        gc::geodesy::Ned::new(n.north, n.east, n.down).map_err(GeoError::from)
    }
}
impl From<gc::geodesy::Aer> for Aer {
    fn from(a: gc::geodesy::Aer) -> Self {
        Aer {
            azimuth_deg: a.azimuth_deg(),
            elevation_deg: a.elevation_deg(),
            range_m: a.range().meters(),
        }
    }
}
impl TryFrom<Aer> for gc::geodesy::Aer {
    type Error = GeoError;

    fn try_from(a: Aer) -> Result<Self, GeoError> {
        gc::geodesy::Aer::new(
            a.azimuth_deg,
            a.elevation_deg,
            gc::Length::from_meters(a.range_m),
        )
        .map_err(GeoError::from)
    }
}

/// The WGS-84 ellipsoid.
#[uniffi::export]
pub fn ellipsoid_wgs84() -> Ellipsoid {
    gc::geodesy::Ellipsoid::WGS84.into()
}
/// The GRS80 ellipsoid (NAD83 / ETRS89).
#[uniffi::export]
pub fn ellipsoid_grs80() -> Ellipsoid {
    gc::geodesy::Ellipsoid::GRS80.into()
}
/// The Krasovsky-1940 ellipsoid (Pulkovo-1942 / SK-42).
#[uniffi::export]
pub fn ellipsoid_krasovsky_1940() -> Ellipsoid {
    gc::geodesy::Ellipsoid::KRASOVSKY_1940.into()
}
/// The Airy-1830 ellipsoid (OSGB36).
#[uniffi::export]
pub fn ellipsoid_airy_1830() -> Ellipsoid {
    gc::geodesy::Ellipsoid::AIRY_1830.into()
}
/// The Bessel-1841 ellipsoid (Tokyo datum).
#[uniffi::export]
pub fn ellipsoid_bessel_1841() -> Ellipsoid {
    gc::geodesy::Ellipsoid::BESSEL_1841.into()
}
/// The Clarke-1866 ellipsoid (NAD27).
#[uniffi::export]
pub fn ellipsoid_clarke_1866() -> Ellipsoid {
    gc::geodesy::Ellipsoid::CLARKE_1866.into()
}

/// Build a validated ellipsoid from its semi-major axis (meters) and inverse
/// flattening (positive infinity for a sphere) — the FFI form of
/// `Ellipsoid::new`.
///
/// # Errors
/// `GeoError::InvalidValue` when the semi-major axis is not finite and
/// positive, or the inverse flattening is NaN or not greater than one.
#[uniffi::export]
pub fn ellipsoid_new(semi_major_m: f64, inverse_flattening: f64) -> Result<Ellipsoid, GeoError> {
    gc::geodesy::Ellipsoid::new(gc::Length::from_meters(semi_major_m), inverse_flattening)
        .map(Into::into)
        .map_err(Into::into)
}
/// Validate a (possibly hand-built) ellipsoid record as `Ellipsoid::new`
/// does.
///
/// # Errors
/// As [`ellipsoid_new`].
#[uniffi::export]
pub fn ellipsoid_validate(ellipsoid: Ellipsoid) -> Result<(), GeoError> {
    gc::geodesy::Ellipsoid::try_from(ellipsoid).map(drop)
}
/// The ellipsoid a reference system's coordinates are defined on, or `None`
/// for GCJ-02 and BD-09, which are obfuscated renderings of WGS-84 rather than
/// geodetic datums.
#[uniffi::export]
pub fn ellipsoid_for_crs(crs: Crs) -> Option<Ellipsoid> {
    gc::geodesy::Ellipsoid::for_crs(crs.into()).map(Into::into)
}

/// Flattening `f` of the ellipsoid.
///
/// # Errors
/// `GeoError::InvalidValue` for a malformed ellipsoid record.
#[uniffi::export]
pub fn ellipsoid_flattening(ellipsoid: Ellipsoid) -> Result<f64, GeoError> {
    Ok(gc::geodesy::Ellipsoid::try_from(ellipsoid)?.flattening())
}
/// Semi-minor axis `b` (meters) of the ellipsoid.
///
/// # Errors
/// `GeoError::InvalidValue` for a malformed ellipsoid record.
#[uniffi::export]
pub fn ellipsoid_semi_minor_m(ellipsoid: Ellipsoid) -> Result<f64, GeoError> {
    Ok(gc::geodesy::Ellipsoid::try_from(ellipsoid)?
        .semi_minor()
        .meters())
}
/// First eccentricity squared `e²` of the ellipsoid.
///
/// # Errors
/// `GeoError::InvalidValue` for a malformed ellipsoid record.
#[uniffi::export]
pub fn ellipsoid_eccentricity_sq(ellipsoid: Ellipsoid) -> Result<f64, GeoError> {
    Ok(gc::geodesy::Ellipsoid::try_from(ellipsoid)?.eccentricity_sq())
}

/// Geodetic [`Coordinate`] → ECEF on the ellipsoid of the coordinate's own
/// reference system (exact).
///
/// # Errors
/// `GeoError::CrsMismatch` (expecting WGS-84) for a GCJ-02 or BD-09
/// coordinate, which has no ellipsoid; `GeoError::InvalidValue` (field
/// `height`) for an orthometric height; `GeoError::OutOfRange` /
/// `GeoError::InvalidValue` for an invalid record.
#[uniffi::export]
pub fn ecef_from_coordinate(coord: Coordinate) -> Result<Ecef, GeoError> {
    gc::geodesy::Ecef::try_from_coordinate(coord.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// ECEF → geodetic [`Coordinate`] on the ellipsoid of `crs`, tagged with
/// `crs`. Exact to floating-point round-off at any altitude (Vermeille's closed
/// form, as refined in GeographicLib).
///
/// # Errors
/// `GeoError::InvalidValue` (field `crs`) when `crs` is GCJ-02 or BD-09;
/// `GeoError::InvalidValue` (field `ECEF`) for a non-finite component, and
/// (field `height`) when the height overflows.
#[uniffi::export]
pub fn ecef_to_coordinate(ecef: Ecef, crs: Crs) -> Result<Coordinate, GeoError> {
    gc::geodesy::Ecef::from(ecef)
        .try_to_coordinate(crs.into())
        .map(Into::into)
        .map_err(Into::into)
}

/// Validate a (possibly hand-built) ENU record — the FFI form of `Enu::new`.
///
/// # Errors
/// `GeoError::InvalidValue` naming `east`, `north`, or `up` when that
/// component is not finite, and `magnitude` when the offset's length
/// overflows.
#[uniffi::export]
pub fn enu_validate(enu: Enu) -> Result<(), GeoError> {
    gc::geodesy::Enu::try_from(enu).map(drop)
}
/// Validate a (possibly hand-built) NED record — the FFI form of `Ned::new`.
///
/// # Errors
/// `GeoError::InvalidValue` naming `north`, `east`, or `down` when that
/// component is not finite, and `magnitude` when the offset's length
/// overflows.
#[uniffi::export]
pub fn ned_validate(ned: Ned) -> Result<(), GeoError> {
    gc::geodesy::Ned::try_from(ned).map(drop)
}
/// Validate a (possibly hand-built) AER record — the FFI form of `Aer::new`.
/// An azimuth outside `[0, 360)` is valid; the conversions normalize it.
///
/// # Errors
/// `GeoError::InvalidValue` naming `azimuth` when it is not finite,
/// `elevation` when it is not finite or outside `[-90, 90]`, and `range` when
/// it is not finite or negative.
#[uniffi::export]
pub fn aer_validate(aer: Aer) -> Result<(), GeoError> {
    gc::geodesy::Aer::try_from(aer).map(drop)
}
/// Construct an AER record as `Aer::new` does: azimuth normalized to
/// `[0, 360)`, a `-0.0` range made `+0.0`.
///
/// # Errors
/// As [`aer_validate`].
#[uniffi::export]
pub fn aer_new(azimuth_deg: f64, elevation_deg: f64, range_m: f64) -> Result<Aer, GeoError> {
    let range = gc::Length::from_meters(range_m);
    Ok(gc::geodesy::Aer::new(azimuth_deg, elevation_deg, range)?.into())
}

/// The ENU offset of `target` relative to `origin`, on the ellipsoid of the
/// classic datum (WGS-84, NAD27, Tokyo, Pulkovo-1942) they share.
///
/// # Errors
/// `GeoError::CrsMismatch` when either coordinate is GCJ-02 or BD-09, or the
/// two are on different datums; `GeoError::InvalidValue` (field `height`) for
/// an orthometric height or an offset that overflows.
#[uniffi::export]
pub fn enu_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Enu, GeoError> {
    gc::geodesy::Enu::try_from_coordinate(target.try_into()?, origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// Recover the absolute coordinate of an ENU offset about `origin` (exact,
/// through the closed-form inverse of [`ecef_to_coordinate`]), tagged with the
/// origin's datum.
///
/// # Errors
/// As [`enu_validate`] for an invalid record; as [`enu_from_coordinate`] for
/// an `origin` it would reject.
#[uniffi::export]
pub fn enu_to_coordinate(enu: Enu, origin: Coordinate) -> Result<Coordinate, GeoError> {
    gc::geodesy::Enu::try_from(enu)?
        .try_to_coordinate(origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// ENU → NED (exact).
///
/// # Errors
/// As [`enu_validate`], for an invalid record.
#[uniffi::export]
pub fn enu_to_ned(enu: Enu) -> Result<Ned, GeoError> {
    Ok(gc::geodesy::Enu::try_from(enu)?.to_ned().into())
}
/// ENU → azimuth/elevation/range (exact).
///
/// # Errors
/// As [`enu_validate`], for an invalid record.
#[uniffi::export]
pub fn enu_to_aer(enu: Enu) -> Result<Aer, GeoError> {
    Ok(gc::geodesy::Enu::try_from(enu)?.to_aer().into())
}

/// The NED offset of `target` relative to `origin` (as [`enu_from_coordinate`]).
///
/// # Errors
/// As [`enu_from_coordinate`].
#[uniffi::export]
pub fn ned_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Ned, GeoError> {
    gc::geodesy::Ned::try_from_coordinate(target.try_into()?, origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// Recover the absolute coordinate of a NED offset about `origin` (exact; see
/// [`enu_to_coordinate`]).
///
/// # Errors
/// As [`ned_validate`] for an invalid record; as [`enu_from_coordinate`] for
/// an `origin` it would reject.
#[uniffi::export]
pub fn ned_to_coordinate(ned: Ned, origin: Coordinate) -> Result<Coordinate, GeoError> {
    gc::geodesy::Ned::try_from(ned)?
        .try_to_coordinate(origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// NED → ENU (exact).
///
/// # Errors
/// As [`ned_validate`], for an invalid record.
#[uniffi::export]
pub fn ned_to_enu(ned: Ned) -> Result<Enu, GeoError> {
    Ok(gc::geodesy::Ned::try_from(ned)?.to_enu().into())
}
/// NED → azimuth/elevation/range (exact).
///
/// # Errors
/// As [`ned_validate`], for an invalid record.
#[uniffi::export]
pub fn ned_to_aer(ned: Ned) -> Result<Aer, GeoError> {
    Ok(gc::geodesy::Ned::try_from(ned)?.to_aer().into())
}

/// The azimuth/elevation/range of `target` relative to `origin` (as
/// [`enu_from_coordinate`]).
///
/// # Errors
/// As [`enu_from_coordinate`].
#[uniffi::export]
pub fn aer_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Aer, GeoError> {
    gc::geodesy::Aer::try_from_coordinate(target.try_into()?, origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// Recover the absolute coordinate of an AER offset about `origin` (exact; see
/// [`enu_to_coordinate`]).
///
/// # Errors
/// As [`aer_validate`] for an invalid record; as [`enu_from_coordinate`] for
/// an `origin` it would reject.
#[uniffi::export]
pub fn aer_to_coordinate(aer: Aer, origin: Coordinate) -> Result<Coordinate, GeoError> {
    gc::geodesy::Aer::try_from(aer)?
        .try_to_coordinate(origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// AER → ENU (exact).
///
/// # Errors
/// As [`aer_validate`], for an invalid record.
#[uniffi::export]
pub fn aer_to_enu(aer: Aer) -> Result<Enu, GeoError> {
    Ok(gc::geodesy::Aer::try_from(aer)?.to_enu().into())
}
/// AER → NED (exact).
///
/// # Errors
/// As [`aer_validate`], for an invalid record.
#[uniffi::export]
pub fn aer_to_ned(aer: Aer) -> Result<Ned, GeoError> {
    Ok(gc::geodesy::Aer::try_from(aer)?.to_ned().into())
}

// ===========================================================================
// Geodesics (distances, bearings, producers)
// ===========================================================================

/// Ellipsoidal (Karney) geodesic distance between two coordinates, in
/// **meters**, measured on the ellipsoid of their shared CRS (NAD27 on Clarke
/// 1866, Tokyo on Bessel 1841, …), accurate to round-off.
///
/// # Errors
/// `GeoError::CrsMismatch` when the coordinates are in different reference
/// systems, or are GCJ-02 / BD-09, which are obfuscated renderings with no
/// ellipsoid (convert them to WGS-84 first); `GeoError::OutOfRange` /
/// `GeoError::InvalidValue` for an invalid coordinate record.
#[uniffi::export]
pub fn geodesic_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::geodesic_distance(a, b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees `[0, 360)`,
/// on the ellipsoid of their shared CRS — the FFI form of the core
/// `geodesy::initial_bearing`, suffixed `_deg` as every angle this surface
/// returns is. From a pole the bearing follows the meridian of `a`'s own
/// longitude.
///
/// # Errors
/// As [`geodesic_distance_m`], and `GeoError::InvalidValue` (field `points`)
/// when `a` and `b` are the same place, which has no bearing.
#[uniffi::export]
pub fn initial_bearing_deg(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::initial_bearing(a, b).map_err(Into::into)
}

/// Final bearing (azimuth on arrival) from `a` to `b`, in degrees `[0, 360)`,
/// on the ellipsoid of their shared CRS — the FFI form of the core
/// `geodesy::final_bearing`.
///
/// # Errors
/// As [`initial_bearing_deg`].
#[uniffi::export]
pub fn final_bearing_deg(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::final_bearing(a, b).map_err(Into::into)
}

/// The point reached from `start` along `bearing_deg` for `distance_m` meters
/// (exact, Karney), on the ellipsoid of `start`'s CRS; the result keeps that
/// CRS and has no height.
///
/// # Errors
/// `GeoError::CrsMismatch` for a GCJ-02 / BD-09 start;
/// `GeoError::InvalidValue` for a non-finite bearing (field `bearing`) or a
/// non-finite or negative distance (field `distance`); `GeoError::OutOfRange`
/// / `GeoError::InvalidValue` for an invalid coordinate record.
#[uniffi::export]
pub fn destination(
    start: Coordinate,
    bearing_deg: f64,
    distance_m: f64,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::destination(
        start.try_into()?,
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .map(Into::into)
    .map_err(Into::into)
}

/// The geodesic midpoint between `a` and `b`, on the ellipsoid of their
/// shared CRS (kept, with no height).
///
/// # Errors
/// As [`geodesic_distance_m`].
#[uniffi::export]
pub fn midpoint(a: Coordinate, b: Coordinate) -> Result<Coordinate, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::midpoint(a, b)
        .map(Into::into)
        .map_err(Into::into)
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the
/// geodesic, on the ellipsoid of their shared CRS (kept, with no height).
///
/// # Errors
/// As [`geodesic_distance_m`], and `GeoError::InvalidValue` (field
/// `fraction`) when `fraction` is not finite or not in `[0, 1]`.
#[uniffi::export]
pub fn intermediate(a: Coordinate, b: Coordinate, fraction: f64) -> Result<Coordinate, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::intermediate(a, b, fraction)
        .map(Into::into)
        .map_err(Into::into)
}

/// Rhumb-line (loxodrome) distance between two coordinates, in **meters**,
/// on the ellipsoid of their shared CRS (the meridian arc when either is a
/// pole).
///
/// # Errors
/// As [`geodesic_distance_m`].
#[uniffi::export]
pub fn rhumb_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::rhumb_distance(a, b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Rhumb-line (constant) bearing from `a` to `b`, in degrees `[0, 360)`, on
/// the ellipsoid of their shared CRS — the FFI form of the core
/// `geodesy::rhumb_bearing`. To a pole it is 0° (north) or 180° (south). From
/// a pole it is taken against the meridian of `a`'s longitude and names the
/// meridian reaching `b`: `180° − (b.lon − a.lon)` from the north pole,
/// `b.lon − a.lon` from the south pole, so [`rhumb_destination`] inverts it.
///
/// # Errors
/// As [`rhumb_distance_m`], and `GeoError::InvalidValue` (field `points`) for
/// coincident points (including one pole named at two longitudes), which have
/// no bearing.
#[uniffi::export]
pub fn rhumb_bearing_deg(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::rhumb_bearing(a, b).map_err(Into::into)
}

/// The point reached from `start` along a constant `bearing_deg` rhumb line for
/// `distance_m` meters, on the ellipsoid of its CRS (kept, with no height). A
/// meridional line (0° or 180°) passes over a pole onto the far meridian; any
/// other bearing errors (`InvalidValue`) when it would reach a pole. From a
/// pole every bearing is accepted, read against the meridian of
/// `start.lon`: from the north pole bearing `β` runs down meridian
/// `start.lon + 180° − β`, from the south pole up `start.lon + β`.
///
/// # Errors
/// As [`destination`], and `GeoError::InvalidValue` (field `distance`) when a
/// line that does not start at a pole, on a bearing other than 0° or 180°,
/// would be carried to or past a pole.
#[uniffi::export]
pub fn rhumb_destination(
    start: Coordinate,
    bearing_deg: f64,
    distance_m: f64,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::rhumb_destination(
        start.try_into()?,
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .map(Into::into)
    .map_err(Into::into)
}
