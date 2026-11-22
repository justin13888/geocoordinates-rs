//! Antimeridian-aware geographic bounds: the [`GeoBounds`] record and its free
//! functions.

use geocoordinates as gc;

use crate::{Coordinate, Crs, GeoError};

/// A geographic box — mirror of [`gc::geodesy::GeoBounds`].
///
/// Decimal degrees on `crs`, every edge inclusive. A `west` greater than `east`
/// crosses the antimeridian. Plain data: every export taking one applies the core
/// crate's rules, and [`geo_bounds_validate`] checks a hand-built one.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct GeoBounds {
    /// South edge, degrees.
    pub south: f64,
    /// West edge, degrees.
    pub west: f64,
    /// North edge, degrees.
    pub north: f64,
    /// East edge, degrees.
    pub east: f64,
    /// The reference system of the edges.
    pub crs: Crs,
}

impl From<gc::geodesy::GeoBounds> for GeoBounds {
    fn from(bounds: gc::geodesy::GeoBounds) -> Self {
        Self {
            south: bounds.south(),
            west: bounds.west(),
            north: bounds.north(),
            east: bounds.east(),
            crs: bounds.crs().into(),
        }
    }
}

impl TryFrom<GeoBounds> for gc::geodesy::GeoBounds {
    type Error = gc::Error;

    fn try_from(bounds: GeoBounds) -> Result<Self, gc::Error> {
        Self::from_edges(gc::geodesy::GeoBoundsEdges {
            south: bounds.south,
            west: bounds.west,
            north: bounds.north,
            east: bounds.east,
            crs: bounds.crs.into(),
        })
    }
}

/// Checks a hand-built [`GeoBounds`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` naming the edge when it is non-finite or out of range,
/// and field `south_west` when south lies north of north.
#[uniffi::export]
pub fn geo_bounds_validate(bounds: GeoBounds) -> Result<(), GeoError> {
    gc::geodesy::GeoBounds::try_from(bounds)?;
    Ok(())
}

/// The smallest box holding every point, choosing the smallest longitude span, so
/// a cluster around ±180° does not span the globe.
///
/// # Errors
/// `GeoError::InvalidValue` (field `points`) for no points;
/// `GeoError::CrsMismatch` when the points' reference systems differ;
/// `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid coordinate
/// record.
#[uniffi::export]
pub fn geo_bounds_from_points(points: Vec<Coordinate>) -> Result<GeoBounds, GeoError> {
    let points = points
        .into_iter()
        .map(gc::Coordinate::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(gc::geodesy::GeoBounds::from_points(points)?.into())
}

/// Whether the box crosses the antimeridian: its west edge lies east of its east
/// edge.
///
/// # Errors
/// `GeoError::InvalidValue` for an invalid [`GeoBounds`] record.
#[uniffi::export]
pub fn geo_bounds_crosses_antimeridian(bounds: GeoBounds) -> Result<bool, GeoError> {
    Ok(gc::geodesy::GeoBounds::try_from(bounds)?.crosses_antimeridian())
}

/// Whether `point` lies in the box, edges included. A pole is inside a box that
/// reaches it, whatever the box's longitudes.
///
/// # Errors
/// `GeoError::CrsMismatch` when `point` is not on the box's reference system;
/// `GeoError::InvalidValue` / `GeoError::OutOfRange` for an invalid record.
#[uniffi::export]
pub fn geo_bounds_contains(bounds: GeoBounds, point: Coordinate) -> Result<bool, GeoError> {
    let bounds = gc::geodesy::GeoBounds::try_from(bounds)?;
    Ok(bounds.contains(point.try_into()?)?)
}

/// Whether the two boxes share any point, edges included.
///
/// # Errors
/// `GeoError::CrsMismatch` when the boxes are on different reference systems;
/// `GeoError::InvalidValue` for an invalid [`GeoBounds`] record.
#[uniffi::export]
pub fn geo_bounds_intersects(a: GeoBounds, b: GeoBounds) -> Result<bool, GeoError> {
    let (a, b) = (
        gc::geodesy::GeoBounds::try_from(a)?,
        gc::geodesy::GeoBounds::try_from(b)?,
    );
    Ok(a.intersects(b)?)
}

/// The smallest box holding both boxes, taking the shorter way around the globe
/// between them.
///
/// # Errors
/// `GeoError::CrsMismatch` when the boxes are on different reference systems;
/// `GeoError::InvalidValue` for an invalid [`GeoBounds`] record.
#[uniffi::export]
pub fn geo_bounds_union(a: GeoBounds, b: GeoBounds) -> Result<GeoBounds, GeoError> {
    let (a, b) = (
        gc::geodesy::GeoBounds::try_from(a)?,
        gc::geodesy::GeoBounds::try_from(b)?,
    );
    Ok(a.union(b)?.into())
}

/// The middle of the box in latitude and longitude: the midpoint of the edges, not
/// a geodesic centroid.
///
/// # Errors
/// `GeoError::InvalidValue` for an invalid [`GeoBounds`] record.
#[uniffi::export]
pub fn geo_bounds_center(bounds: GeoBounds) -> Result<Coordinate, GeoError> {
    Ok(gc::geodesy::GeoBounds::try_from(bounds)?.center().into())
}

/// The box grown so it holds every point within geodesic `distance_m` meters of the
/// original box, clamped at the poles. Reaching a pole takes in every longitude.
///
/// # Errors
/// `GeoError::InvalidValue` (field `distance`) when `distance_m` is negative or
/// non-finite, or for an invalid [`GeoBounds`] record; `GeoError::CrsMismatch` on
/// GCJ-02 / BD-09, which have no ellipsoid.
#[uniffi::export]
pub fn geo_bounds_expand_by(bounds: GeoBounds, distance_m: f64) -> Result<GeoBounds, GeoError> {
    let bounds = gc::geodesy::GeoBounds::try_from(bounds)?;
    Ok(bounds
        .expand_by(gc::Length::from_meters(distance_m))?
        .into())
}
