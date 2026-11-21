//! Geodesy: ellipsoids, ECEF and local tangent frames, and the mean-sphere
//! distance, with the ellipsoidal geodesics and rhumb lines in `geodesic`,
//! the `GeoBounds` record and its `geo_bounds_*` functions in `bounds`, the
//! cross/along-track and intersection routines in `track`, and the core's
//! classic-datum Helmert transforms in `datum`.

use geocoordinates as gc;

use crate::{Coordinate, Crs, GeoError};

mod bounds; // the antimeridian-aware GeoBounds record and its free functions
pub use bounds::*;
mod datum; // Helmert parameters and the classic-datum transforms built on them
pub use datum::*;
mod geodesic; // ellipsoidal geodesics and rhumb lines
pub use geodesic::*;
mod track; // cross/along-track and intersection, spherical and ellipsoidal
pub use track::*;

/// Cheap spherical (haversine) distance between two coordinates, in **meters**
/// (`Length` does not cross the FFI boundary), on the IUGG mean sphere.
///
/// **Error bound.** Approximate: against the WGS-84 ellipsoidal geodesic
/// ([`geodesic_distance_m`]), the relative error measured over a global 5° grid
/// of pairs peaks at about 0.56% (≈ 5.6 m per km). Use the geodesic where that
/// matters.
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
/// Get one from [`ellipsoid_for_crs`] or a named export ([`ellipsoid_wgs84`],
/// …). The core crate has no public constructor from raw parameters, because
/// no function takes an arbitrary ellipsoid, so neither does this mirror.
/// Unvalidated until used: every export that takes one accepts only a record
/// equal to an ellipsoid [`ellipsoid_for_crs`] returns. Call
/// [`ellipsoid_validate`] to check a hand-built record up front.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ellipsoid {
    /// Semi-major axis `a`, in meters.
    pub semi_major_m: f64,
    /// Inverse flattening `1/f`.
    pub inverse_flattening: f64,
}

/// A geocentric ECEF position in meters — mirror of [`gc::Ecef`](gc::geodesy::Ecef).
///
/// Unvalidated until used: every export that takes one validates it as the
/// core `Ecef::new` does (each component finite). Call [`ecef_validate`] to
/// check a hand-built record up front.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ecef {
    /// X axis (meters), through the prime meridian at the equator.
    pub x_m: f64,
    /// Y axis (meters), 90° east at the equator.
    pub y_m: f64,
    /// Z axis (meters), through the north pole.
    pub z_m: f64,
}

/// East-North-Up offset (meters) — mirror of [`gc::Enu`](gc::geodesy::Enu).
///
/// Unvalidated until used: every export that takes one validates it as the
/// core `Enu::new` does (each component finite, and the offset's length
/// finite). Call [`enu_validate`] to check a hand-built record up front.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Enu {
    /// East offset (meters).
    pub east_m: f64,
    /// North offset (meters).
    pub north_m: f64,
    /// Up offset (meters).
    pub up_m: f64,
}

/// North-East-Down offset (meters) — mirror of [`gc::Ned`](gc::geodesy::Ned).
///
/// Unvalidated until used, as [`Enu`]; call [`ned_validate`] to check a
/// hand-built record up front.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ned {
    /// North offset (meters).
    pub north_m: f64,
    /// East offset (meters).
    pub east_m: f64,
    /// Down offset (meters).
    pub down_m: f64,
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
/// The ellipsoids a caller can hold: those `ellipsoid_for_crs` returns.
fn catalogued_ellipsoids() -> impl Iterator<Item = gc::geodesy::Ellipsoid> {
    use Crs::{Bd09, Gcj02, Nad27, Pulkovo42, Tokyo, Wgs84};
    // Exhaustive: a new `Crs` variant fails this build until listed below.
    let _ = |crs: Crs| match crs {
        Wgs84 | Gcj02 | Bd09 | Nad27 | Tokyo | Pulkovo42 => {}
    };
    [Wgs84, Gcj02, Bd09, Nad27, Tokyo, Pulkovo42]
        .into_iter()
        .filter_map(|crs| gc::geodesy::Ellipsoid::for_crs(crs.into()))
}
impl TryFrom<Ellipsoid> for gc::geodesy::Ellipsoid {
    type Error = GeoError;

    /// Resolve the record to the catalogued ellipsoid it equals, since the
    /// core crate builds an `Ellipsoid` only from its catalogue.
    fn try_from(e: Ellipsoid) -> Result<Self, GeoError> {
        catalogued_ellipsoids()
            .find(|catalogued| Ellipsoid::from(*catalogued) == e)
            .ok_or_else(|| {
                gc::Error::InvalidValue {
                    field: "ellipsoid",
                    detail: "not the ellipsoid of any reference system (see ellipsoid_for_crs)"
                        .into(),
                }
                .into()
            })
    }
}
impl From<gc::geodesy::Ecef> for Ecef {
    fn from(c: gc::geodesy::Ecef) -> Self {
        Ecef {
            x_m: c.x_m(),
            y_m: c.y_m(),
            z_m: c.z_m(),
        }
    }
}
impl TryFrom<Ecef> for gc::geodesy::Ecef {
    type Error = GeoError;

    fn try_from(c: Ecef) -> Result<Self, GeoError> {
        gc::geodesy::Ecef::new(c.x_m, c.y_m, c.z_m).map_err(GeoError::from)
    }
}
impl From<gc::geodesy::Enu> for Enu {
    fn from(e: gc::geodesy::Enu) -> Self {
        Enu {
            east_m: e.east_m(),
            north_m: e.north_m(),
            up_m: e.up_m(),
        }
    }
}
impl TryFrom<Enu> for gc::geodesy::Enu {
    type Error = GeoError;

    fn try_from(e: Enu) -> Result<Self, GeoError> {
        gc::geodesy::Enu::new(e.east_m, e.north_m, e.up_m).map_err(GeoError::from)
    }
}
impl From<gc::geodesy::Ned> for Ned {
    fn from(n: gc::geodesy::Ned) -> Self {
        Ned {
            north_m: n.north_m(),
            east_m: n.east_m(),
            down_m: n.down_m(),
        }
    }
}
impl TryFrom<Ned> for gc::geodesy::Ned {
    type Error = GeoError;

    fn try_from(n: Ned) -> Result<Self, GeoError> {
        gc::geodesy::Ned::new(n.north_m, n.east_m, n.down_m).map_err(GeoError::from)
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
/// The Krasovsky-1940 ellipsoid (Pulkovo-1942 / SK-42).
#[uniffi::export]
pub fn ellipsoid_krasovsky_1940() -> Ellipsoid {
    gc::geodesy::Ellipsoid::KRASOVSKY_1940.into()
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

/// Validate a (possibly hand-built) ellipsoid record: it must equal an
/// ellipsoid [`ellipsoid_for_crs`] returns, since those are the only ones the
/// core crate builds.
///
/// # Errors
/// `GeoError::InvalidValue` (field `ellipsoid`) for a record equal to no
/// catalogued ellipsoid.
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
/// As [`ellipsoid_validate`].
#[uniffi::export]
pub fn ellipsoid_flattening(ellipsoid: Ellipsoid) -> Result<f64, GeoError> {
    Ok(gc::geodesy::Ellipsoid::try_from(ellipsoid)?.flattening())
}
/// Semi-minor axis `b` (meters) of the ellipsoid.
///
/// # Errors
/// As [`ellipsoid_validate`].
#[uniffi::export]
pub fn ellipsoid_semi_minor_m(ellipsoid: Ellipsoid) -> Result<f64, GeoError> {
    Ok(gc::geodesy::Ellipsoid::try_from(ellipsoid)?
        .semi_minor()
        .meters())
}
/// First eccentricity squared `e²` of the ellipsoid.
///
/// # Errors
/// As [`ellipsoid_validate`].
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
/// `GeoError::InvalidValue` naming `x_m`, `y_m`, or `z_m` for a non-finite
/// component of the record, as [`ecef_validate`]; (field `crs`) when `crs` is
/// GCJ-02 or BD-09; and (field `height`) when the height overflows.
#[uniffi::export]
pub fn ecef_to_coordinate(ecef: Ecef, crs: Crs) -> Result<Coordinate, GeoError> {
    gc::geodesy::Ecef::try_from(ecef)?
        .try_to_coordinate(crs.into())
        .map(Into::into)
        .map_err(Into::into)
}

/// Validate a (possibly hand-built) ECEF record — the FFI form of `Ecef::new`.
///
/// # Errors
/// `GeoError::InvalidValue` naming `x_m`, `y_m`, or `z_m` when that component
/// (the record's `x`, `y`, or `z`) is not finite.
#[uniffi::export]
pub fn ecef_validate(ecef: Ecef) -> Result<(), GeoError> {
    gc::geodesy::Ecef::try_from(ecef).map(drop)
}

/// Validate a (possibly hand-built) ENU record — the FFI form of `Enu::new`.
///
/// # Errors
/// `GeoError::InvalidValue` naming `east_m`, `north_m`, or `up_m` (the
/// parameters of `Enu::new`) when that component is not finite, and `magnitude` when the offset's length
/// overflows.
#[uniffi::export]
pub fn enu_validate(enu: Enu) -> Result<(), GeoError> {
    gc::geodesy::Enu::try_from(enu).map(drop)
}
/// Validate a (possibly hand-built) NED record — the FFI form of `Ned::new`.
///
/// # Errors
/// `GeoError::InvalidValue` naming `north_m`, `east_m`, or `down_m` (the
/// parameters of `Ned::new`) when that component is not finite, and `magnitude` when the offset's length
/// overflows.
#[uniffi::export]
pub fn ned_validate(ned: Ned) -> Result<(), GeoError> {
    gc::geodesy::Ned::try_from(ned).map(drop)
}
/// Validate a (possibly hand-built) AER record — the FFI form of `Aer::new`.
/// An azimuth outside `[0, 360)` is valid; the conversions normalize it.
///
/// # Errors
/// `GeoError::InvalidValue` naming `azimuth_deg` when it is not finite,
/// `elevation_deg` when it is not finite or outside `[-90, 90]`, and `range`
/// (the parameters of `Aer::new`) when it is not finite or negative.
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
/// an orthometric height or an offset that overflows; `GeoError::OutOfRange` /
/// `GeoError::InvalidValue` (field `height`) for an invalid coordinate record.
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
