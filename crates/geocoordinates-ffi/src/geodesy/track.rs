//! Cross-track and along-track distance and path intersection: the
//! mean-sphere forms (`*_spherical`) and the ellipsoidal ones, which run on
//! the ellipsoid of the inputs' own CRS.

use geocoordinates as gc;

use crate::{Coordinate, GeoError};

/// Signed perpendicular distance (meters) from `point` to the great-circle
/// path `start` → `end` on the mean sphere (positive to the right).
/// Approximate: the sphere is not the ellipsoid.
///
/// Errors when `start` and `end` are the same place or antipodal (no single
/// great circle runs through them).
#[uniffi::export]
pub fn cross_track_distance_spherical_m(
    point: Coordinate,
    start: Coordinate,
    end: Coordinate,
) -> Result<f64, GeoError> {
    let (point, start, end): (gc::Coordinate, gc::Coordinate, gc::Coordinate) =
        (point.try_into()?, start.try_into()?, end.try_into()?);
    gc::geodesy::cross_track_distance_spherical(&point, &start, &end)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// Along-track distance (meters) from `start` to the foot of the perpendicular
/// from `point` onto the great-circle path `start` → `end` on the mean sphere.
/// Approximate; errors as [`cross_track_distance_spherical_m`] does, and with
/// `GeoError::InvalidValue` (field `point`) when `point` is a pole of the
/// path's great circle, where no along-track distance is defined.
#[uniffi::export]
pub fn along_track_distance_spherical_m(
    point: Coordinate,
    start: Coordinate,
    end: Coordinate,
) -> Result<f64, GeoError> {
    let (point, start, end): (gc::Coordinate, gc::Coordinate, gc::Coordinate) =
        (point.try_into()?, start.try_into()?, end.try_into()?);
    gc::geodesy::along_track_distance_spherical(&point, &start, &end)
        .map(|length| length.meters())
        .map_err(Into::into)
}

/// The outcome of [`intersection_spherical`] — mirror of
/// [`gc::geodesy::Intersection`].
///
/// `gc::geodesy::Intersection` is exhaustive, so the `From` impl below is an
/// exhaustive match: a new outcome upstream fails this crate's build until
/// mirrored here.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum IntersectionResult {
    /// The paths cross at `point`, less than half a great circle ahead of each
    /// start (in the inputs' CRS, with no height).
    Crossing {
        /// The crossing point.
        point: Coordinate,
    },
    /// Both paths run along the same great circle, so no single crossing
    /// exists.
    SameGreatCircle,
    /// Neither crossing of the two great circles lies ahead of both starts.
    NoForwardCrossing,
}

impl From<gc::geodesy::Intersection> for IntersectionResult {
    fn from(x: gc::geodesy::Intersection) -> Self {
        match x {
            gc::geodesy::Intersection::Crossing(point) => Self::Crossing {
                point: point.into(),
            },
            gc::geodesy::Intersection::SameGreatCircle => Self::SameGreatCircle,
            gc::geodesy::Intersection::NoForwardCrossing => Self::NoForwardCrossing,
        }
    }
}

/// Where two great-circle paths (each a start point and an initial bearing in
/// degrees) cross ahead of both starts, on the mean sphere. Approximate: the
/// sphere is not the ellipsoid.
///
/// At a pole a bearing follows the meridian of the start's own longitude.
/// Errors for GCJ-02 / BD-09 inputs, mixed CRSs, or a non-finite bearing.
#[uniffi::export]
pub fn intersection_spherical(
    a: Coordinate,
    bearing_a_deg: f64,
    b: Coordinate,
    bearing_b_deg: f64,
) -> Result<IntersectionResult, GeoError> {
    gc::geodesy::intersection_spherical(
        &a.try_into()?,
        bearing_a_deg,
        &b.try_into()?,
        bearing_b_deg,
    )
    .map(Into::into)
    .map_err(Into::into)
}

/// Signed perpendicular distance (meters) from `point` to the geodesic path
/// `start` → `end`, on the ellipsoid of their shared CRS (positive to the
/// right). Exact to well under a micrometre, unlike
/// [`cross_track_distance_spherical_m`].
///
/// Errors (`GeoError::CrsMismatch`) for mixed CRSs or GCJ-02 / BD-09 inputs,
/// and (`GeoError::InvalidValue`, field `path`) when `start` and `end` are the
/// same place or antipodal.
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
/// from `point` onto the geodesic path `start` → `end`, on the ellipsoid of
/// their shared CRS (negative behind `start`). Errors as
/// [`cross_track_distance_m`] does, and with `GeoError::InvalidValue` (field
/// `point`) where every foot is equally near (a pole, for an equatorial path).
/// Near such a point the foot is defined only to about `1e-9 m / M`, `M` the
/// geodesic scale of `point` relative to its foot.
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

/// The outcome of [`intersection`] — mirror of
/// [`gc::geodesy::GeodesicIntersection`].
///
/// `gc::geodesy::GeodesicIntersection` is exhaustive, so the `From` impl below
/// is an exhaustive match: a new outcome upstream fails this crate's build
/// until mirrored here.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum GeodesicIntersectionResult {
    /// The paths cross at `point`, which each start reaches along its own path
    /// as the shortest geodesic (in the inputs' CRS, with no height).
    Crossing {
        /// The crossing point.
        point: Coordinate,
    },
    /// Both paths run along the same geodesic, so no single crossing exists.
    SameGeodesic,
    /// The geodesics cross at no point both starts reach forward along their
    /// own paths as the shortest geodesic.
    NoForwardCrossing,
}

impl From<gc::geodesy::GeodesicIntersection> for GeodesicIntersectionResult {
    fn from(x: gc::geodesy::GeodesicIntersection) -> Self {
        match x {
            gc::geodesy::GeodesicIntersection::Crossing(point) => Self::Crossing {
                point: point.into(),
            },
            gc::geodesy::GeodesicIntersection::SameGeodesic => Self::SameGeodesic,
            gc::geodesy::GeodesicIntersection::NoForwardCrossing => Self::NoForwardCrossing,
        }
    }
}

/// Where two geodesic paths (each a start point and an initial bearing in
/// degrees) cross ahead of both starts, on the ellipsoid of their shared CRS.
/// Exact to well under a micrometre, unlike [`intersection_spherical`].
///
/// At a pole a bearing follows the meridian of the start's own longitude.
/// Errors for GCJ-02 / BD-09 inputs, mixed CRSs, a non-finite bearing, or
/// (`GeoError::InvalidValue`, field `paths`) when the iteration cannot locate
/// a crossing the mean sphere places ahead of both starts.
#[uniffi::export]
pub fn intersection(
    a: Coordinate,
    bearing_a_deg: f64,
    b: Coordinate,
    bearing_b_deg: f64,
) -> Result<GeodesicIntersectionResult, GeoError> {
    gc::geodesy::intersection(&a.try_into()?, bearing_a_deg, &b.try_into()?, bearing_b_deg)
        .map(Into::into)
        .map_err(Into::into)
}
