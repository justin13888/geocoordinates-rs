//! Geodesy: ellipsoids, ECEF and local tangent frames, distances and bearings,
//! and classic-datum Helmert transforms.

use geocoordinates as gc;

use crate::{Coordinate, Crs, GeoError};

/// Cheap spherical (haversine) distance between two coordinates, in **meters**.
///
/// `Length` and its operators do not cross the FFI boundary, so the scalar
/// meters value is returned directly.
#[uniffi::export]
pub fn haversine_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let a: gc::Coordinate = a.try_into()?;
    let b: gc::Coordinate = b.try_into()?;
    gc::geodesy::haversine_distance(&a, &b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

// ===========================================================================
// Geodesy: ellipsoids, ECEF, and local tangent frames
// ===========================================================================

/// A reference ellipsoid — mirror of [`gc::Ellipsoid`](gc::geodesy::Ellipsoid).
///
/// Unvalidated until used: constructing one does not check its parameters.
/// The fallible ECEF conversions reject a malformed ellipsoid with
/// [`GeoError`], but the infallible getters (`ellipsoid_flattening`, …) compute
/// from the values as given and can return non-finite or meaningless results.
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
            semi_major_m: e.semi_major_m,
            inverse_flattening: e.inverse_flattening,
        }
    }
}
impl From<Ellipsoid> for gc::geodesy::Ellipsoid {
    fn from(e: Ellipsoid) -> Self {
        gc::geodesy::Ellipsoid {
            semi_major_m: e.semi_major_m,
            inverse_flattening: e.inverse_flattening,
        }
    }
}
impl From<gc::geodesy::Ecef> for Ecef {
    fn from(c: gc::geodesy::Ecef) -> Self {
        Ecef {
            x: c.x,
            y: c.y,
            z: c.z,
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
            east: e.east,
            north: e.north,
            up: e.up,
        }
    }
}
impl From<Enu> for gc::geodesy::Enu {
    fn from(e: Enu) -> Self {
        gc::geodesy::Enu {
            east: e.east,
            north: e.north,
            up: e.up,
        }
    }
}
impl From<gc::geodesy::Ned> for Ned {
    fn from(n: gc::geodesy::Ned) -> Self {
        Ned {
            north: n.north,
            east: n.east,
            down: n.down,
        }
    }
}
impl From<Ned> for gc::geodesy::Ned {
    fn from(n: Ned) -> Self {
        gc::geodesy::Ned {
            north: n.north,
            east: n.east,
            down: n.down,
        }
    }
}
impl From<gc::geodesy::Aer> for Aer {
    fn from(a: gc::geodesy::Aer) -> Self {
        Aer {
            azimuth_deg: a.azimuth_deg,
            elevation_deg: a.elevation_deg,
            range_m: a.range.meters(),
        }
    }
}
impl From<Aer> for gc::geodesy::Aer {
    fn from(a: Aer) -> Self {
        gc::geodesy::Aer {
            azimuth_deg: a.azimuth_deg,
            elevation_deg: a.elevation_deg,
            range: gc::Length::from_meters(a.range_m),
        }
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

/// Flattening `f` of the ellipsoid.
#[uniffi::export]
pub fn ellipsoid_flattening(ellipsoid: Ellipsoid) -> f64 {
    gc::geodesy::Ellipsoid::from(ellipsoid).flattening()
}
/// Semi-minor axis `b` (meters) of the ellipsoid.
#[uniffi::export]
pub fn ellipsoid_semi_minor_m(ellipsoid: Ellipsoid) -> f64 {
    gc::geodesy::Ellipsoid::from(ellipsoid).semi_minor_m()
}
/// First eccentricity squared `e²` of the ellipsoid.
#[uniffi::export]
pub fn ellipsoid_eccentricity_sq(ellipsoid: Ellipsoid) -> f64 {
    gc::geodesy::Ellipsoid::from(ellipsoid).eccentricity_sq()
}

/// Geodetic [`Coordinate`] → ECEF on the given ellipsoid.
#[uniffi::export]
pub fn ecef_from_coordinate(coord: Coordinate, ellipsoid: Ellipsoid) -> Result<Ecef, GeoError> {
    gc::geodesy::Ecef::try_from_coordinate(coord.try_into()?, ellipsoid.into())
        .map(Into::into)
        .map_err(Into::into)
}
/// ECEF → geodetic [`Coordinate`] on the given ellipsoid and reference system,
/// by Bowring's single-step inverse. Not exact: the latitude error is about a
/// micrometer within ±10 km of the surface, about 7 mm at 1,000 km, and up to
/// about 0.3 m at GNSS and geostationary altitudes.
#[uniffi::export]
pub fn ecef_to_coordinate(
    ecef: Ecef,
    ellipsoid: Ellipsoid,
    crs: Crs,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::Ecef::from(ecef)
        .try_to_coordinate(ellipsoid.into(), crs.into())
        .map(Into::into)
        .map_err(Into::into)
}

/// The ENU offset of `target` relative to `origin` (WGS-84).
#[uniffi::export]
pub fn enu_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Enu, GeoError> {
    gc::geodesy::Enu::try_from_coordinate(target.try_into()?, origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// Recover the absolute coordinate of an ENU offset about `origin`. Not exact:
/// latitude is recovered by the Bowring single-step inverse of
/// [`ecef_to_coordinate`] and carries its altitude-dependent bound.
#[uniffi::export]
pub fn enu_to_coordinate(enu: Enu, origin: Coordinate) -> Result<Coordinate, GeoError> {
    gc::geodesy::Enu::from(enu)
        .try_to_coordinate(origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// ENU → NED.
#[uniffi::export]
pub fn enu_to_ned(enu: Enu) -> Ned {
    gc::geodesy::Enu::from(enu).to_ned().into()
}
/// ENU → azimuth/elevation/range.
#[uniffi::export]
pub fn enu_to_aer(enu: Enu) -> Aer {
    gc::geodesy::Enu::from(enu).to_aer().into()
}

/// The NED offset of `target` relative to `origin` (WGS-84).
#[uniffi::export]
pub fn ned_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Ned, GeoError> {
    gc::geodesy::Ned::try_from_coordinate(target.try_into()?, origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// Recover the absolute coordinate of a NED offset about `origin`. Not exact:
/// see [`enu_to_coordinate`].
#[uniffi::export]
pub fn ned_to_coordinate(ned: Ned, origin: Coordinate) -> Result<Coordinate, GeoError> {
    gc::geodesy::Ned::from(ned)
        .try_to_coordinate(origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// NED → ENU.
#[uniffi::export]
pub fn ned_to_enu(ned: Ned) -> Enu {
    gc::geodesy::Ned::from(ned).to_enu().into()
}
/// NED → azimuth/elevation/range.
#[uniffi::export]
pub fn ned_to_aer(ned: Ned) -> Aer {
    gc::geodesy::Ned::from(ned).to_aer().into()
}

/// The azimuth/elevation/range of `target` relative to `origin` (WGS-84).
#[uniffi::export]
pub fn aer_from_coordinate(target: Coordinate, origin: Coordinate) -> Result<Aer, GeoError> {
    gc::geodesy::Aer::try_from_coordinate(target.try_into()?, origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// Recover the absolute coordinate of an AER offset about `origin`. Not exact:
/// see [`enu_to_coordinate`].
#[uniffi::export]
pub fn aer_to_coordinate(aer: Aer, origin: Coordinate) -> Result<Coordinate, GeoError> {
    gc::geodesy::Aer::from(aer)
        .try_to_coordinate(origin.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}
/// AER → ENU.
#[uniffi::export]
pub fn aer_to_enu(aer: Aer) -> Enu {
    gc::geodesy::Aer::from(aer).to_enu().into()
}
/// AER → NED.
#[uniffi::export]
pub fn aer_to_ned(aer: Aer) -> Ned {
    gc::geodesy::Aer::from(aer).to_ned().into()
}

// ===========================================================================
// Geodesics (distances, bearings, producers)
// ===========================================================================

/// Ellipsoidal (Karney) geodesic distance between two coordinates, in
/// **meters**, measured on the **WGS-84** ellipsoid whatever their shared CRS.
///
/// Accurate to round-off for WGS-84 points. Two classic-datum points (NAD27,
/// Tokyo, Pulkovo-1942) are off by up to about 1.3e-4 of the distance, because
/// their own ellipsoid is not WGS-84. Two GCJ-02 or BD-09 points carry the
/// difference of their obfuscation offsets; convert them to WGS-84 first for a
/// true distance.
#[uniffi::export]
pub fn geodesic_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::geodesic_distance(&a, &b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees `[0, 360)`.
#[uniffi::export]
pub fn initial_bearing(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::initial_bearing(&a, &b).map_err(Into::into)
}

/// Final bearing (azimuth on arrival) from `a` to `b`, in degrees `[0, 360)`.
#[uniffi::export]
pub fn final_bearing(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::final_bearing(&a, &b).map_err(Into::into)
}

/// The point reached from `start` along `bearing_deg` for `distance_m` meters
/// (exact, Karney).
#[uniffi::export]
pub fn destination(
    start: Coordinate,
    bearing_deg: f64,
    distance_m: f64,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::destination(
        &start.try_into()?,
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .map(Into::into)
    .map_err(Into::into)
}

/// The geodesic midpoint between `a` and `b`.
#[uniffi::export]
pub fn midpoint(a: Coordinate, b: Coordinate) -> Result<Coordinate, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::midpoint(&a, &b)
        .map(Into::into)
        .map_err(Into::into)
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the geodesic.
#[uniffi::export]
pub fn intermediate(a: Coordinate, b: Coordinate, fraction: f64) -> Result<Coordinate, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::intermediate(&a, &b, fraction)
        .map(Into::into)
        .map_err(Into::into)
}

/// Rhumb-line (loxodrome) distance between two coordinates, in **meters**.
#[uniffi::export]
pub fn rhumb_distance_m(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::rhumb_distance(&a, &b)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Rhumb-line (constant) bearing from `a` to `b`, in degrees `[0, 360)`.
#[uniffi::export]
pub fn rhumb_bearing(a: Coordinate, b: Coordinate) -> Result<f64, GeoError> {
    let (a, b): (gc::Coordinate, gc::Coordinate) = (a.try_into()?, b.try_into()?);
    gc::geodesy::rhumb_bearing(&a, &b).map_err(Into::into)
}

/// The point reached from `start` along a constant `bearing_deg` rhumb line for
/// `distance_m` meters.
#[uniffi::export]
pub fn rhumb_destination(
    start: Coordinate,
    bearing_deg: f64,
    distance_m: f64,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::rhumb_destination(
        &start.try_into()?,
        bearing_deg,
        gc::Length::from_meters(distance_m),
    )
    .map(Into::into)
    .map_err(Into::into)
}

/// Signed perpendicular distance (meters) from `point` to the path
/// `start` → `end` (positive to the right).
#[uniffi::export]
pub fn cross_track_distance_m(
    point: Coordinate,
    start: Coordinate,
    end: Coordinate,
) -> Result<f64, GeoError> {
    let (point, start, end): (gc::Coordinate, gc::Coordinate, gc::Coordinate) =
        (point.try_into()?, start.try_into()?, end.try_into()?);
    gc::geodesy::cross_track_distance(&point, &start, &end)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Along-track distance (meters) from `start` to the foot of the perpendicular
/// from `point` onto `start` → `end`.
#[uniffi::export]
pub fn along_track_distance_m(
    point: Coordinate,
    start: Coordinate,
    end: Coordinate,
) -> Result<f64, GeoError> {
    let (point, start, end): (gc::Coordinate, gc::Coordinate, gc::Coordinate) =
        (point.try_into()?, start.try_into()?, end.try_into()?);
    gc::geodesy::along_track_distance(&point, &start, &end)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Intersection of two great circles (each a point + initial bearing), or
/// `None` when they are parallel/coincident or ambiguous.
#[uniffi::export]
pub fn intersection(
    a: Coordinate,
    bearing_a_deg: f64,
    b: Coordinate,
    bearing_b_deg: f64,
) -> Result<Option<Coordinate>, GeoError> {
    gc::geodesy::intersection(&a.try_into()?, bearing_a_deg, &b.try_into()?, bearing_b_deg)
        .map(|point| point.map(Into::into))
        .map_err(Into::into)
}

// ===========================================================================
// Classic datums — Helmert (Bursa-Wolf) transforms
// ===========================================================================

/// The seven Bursa-Wolf parameters of a Helmert transform — mirror of
/// [`gc::Helmert`](gc::geodesy::Helmert). Translations in meters, rotations in
/// arc-seconds (position-vector convention), scale in parts-per-million.
///
/// Unvalidated until used: the seven parameters are applied as given, with no
/// check that they are finite or of plausible magnitude.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Helmert {
    /// X-axis translation, meters.
    pub tx_m: f64,
    /// Y-axis translation, meters.
    pub ty_m: f64,
    /// Z-axis translation, meters.
    pub tz_m: f64,
    /// X-axis rotation, arc-seconds (position-vector convention).
    pub rx_arcsec: f64,
    /// Y-axis rotation, arc-seconds (position-vector convention).
    pub ry_arcsec: f64,
    /// Z-axis rotation, arc-seconds (position-vector convention).
    pub rz_arcsec: f64,
    /// Scale difference, parts-per-million.
    pub scale_ppm: f64,
}

/// A complete datum transform (source/target ellipsoids + the Helmert shift) —
/// mirror of [`gc::DatumTransform`](gc::geodesy::DatumTransform).
///
/// Unvalidated until used: nothing checks that `from` / `to` are the
/// ellipsoids of `from_crs` / `to_crs`, or that `helmert` connects those two
/// datums. Applying it rejects an input whose CRS is not `from_crs` and a
/// malformed ellipsoid, but otherwise uses a hand-built transform as given.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct DatumTransform {
    /// Source reference system.
    pub from_crs: Crs,
    /// Target reference system.
    pub to_crs: Crs,
    /// Ellipsoid of the source datum.
    pub from: Ellipsoid,
    /// Ellipsoid of the target datum.
    pub to: Ellipsoid,
    /// Helmert parameters carrying the source frame to the target frame.
    pub helmert: Helmert,
}

impl From<gc::geodesy::Helmert> for Helmert {
    fn from(h: gc::geodesy::Helmert) -> Self {
        Helmert {
            tx_m: h.tx_m,
            ty_m: h.ty_m,
            tz_m: h.tz_m,
            rx_arcsec: h.rx_arcsec,
            ry_arcsec: h.ry_arcsec,
            rz_arcsec: h.rz_arcsec,
            scale_ppm: h.scale_ppm,
        }
    }
}
impl From<Helmert> for gc::geodesy::Helmert {
    fn from(h: Helmert) -> Self {
        gc::geodesy::Helmert {
            tx_m: h.tx_m,
            ty_m: h.ty_m,
            tz_m: h.tz_m,
            rx_arcsec: h.rx_arcsec,
            ry_arcsec: h.ry_arcsec,
            rz_arcsec: h.rz_arcsec,
            scale_ppm: h.scale_ppm,
        }
    }
}
impl From<gc::geodesy::DatumTransform> for DatumTransform {
    fn from(d: gc::geodesy::DatumTransform) -> Self {
        DatumTransform {
            from_crs: d.from_crs.into(),
            to_crs: d.to_crs.into(),
            from: d.from.into(),
            to: d.to.into(),
            helmert: d.helmert.into(),
        }
    }
}
impl From<DatumTransform> for gc::geodesy::DatumTransform {
    fn from(d: DatumTransform) -> Self {
        gc::geodesy::DatumTransform {
            from_crs: d.from_crs.into(),
            to_crs: d.to_crs.into(),
            from: d.from.into(),
            to: d.to.into(),
            helmert: d.helmert.into(),
        }
    }
}

/// The identity Helmert transform (no translation, rotation, or scale).
#[uniffi::export]
pub fn helmert_identity() -> Helmert {
    gc::geodesy::Helmert::IDENTITY.into()
}

/// Apply a Helmert transform to a geocentric (ECEF) position.
#[uniffi::export]
pub fn helmert_apply_ecef(helmert: Helmert, ecef: Ecef) -> Ecef {
    gc::geodesy::Helmert::from(helmert)
        .apply_ecef(ecef.into())
        .into()
}

/// The inverse Helmert transform (negated parameters). Exact for
/// translation-only transforms, as every catalogued one is; otherwise
/// first-order, leaving about a centimeter of round-trip residual for a full
/// seven-parameter set such as WGS-84 → OSGB36.
#[uniffi::export]
pub fn helmert_inverse(helmert: Helmert) -> Helmert {
    gc::geodesy::Helmert::from(helmert).inverse().into()
}

/// The catalogued transform carrying `datum` to WGS-84, or `None` when none is
/// built in (WGS-84 itself and the China obfuscation systems).
#[uniffi::export]
pub fn datum_transform_to_wgs84(datum: Crs) -> Option<DatumTransform> {
    gc::geodesy::DatumTransform::to_wgs84(datum.into()).map(Into::into)
}

/// Transform a geodetic coordinate from the source to the target datum, tagging
/// the result with `to`.
///
/// Not exact. The computation adds well under a millimeter at terrestrial
/// heights, and the result is only as accurate as the Helmert parameters: the
/// catalogued ones are regional mean, translation-only shifts good to several
/// meters.
#[uniffi::export]
pub fn datum_transform_apply(
    transform: DatumTransform,
    coord: Coordinate,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::DatumTransform::from(transform)
        .transform(coord.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}

/// The reverse datum transform (swaps ellipsoids, inverts the Helmert shift).
#[uniffi::export]
pub fn datum_transform_inverse(transform: DatumTransform) -> DatumTransform {
    gc::geodesy::DatumTransform::from(transform)
        .inverse()
        .into()
}
