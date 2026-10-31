//! Geographic bounds and the geodesic spatial index.
//!
//! `GeoBounds` crosses as a record with free functions. `SpatialIndex<T>` is
//! generic, so it crosses as the concrete object [`SpatialIndex`] over
//! coordinates alone: each result carries the entry's insertion position, which
//! a foreign caller uses to look up its own payload.

use std::sync::Arc;

use geocoordinates as gc;

use crate::{Coordinate, Crs, GeoError};

/// A geographic box — mirror of [`gc::index::GeoBounds`].
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

impl From<gc::index::GeoBounds> for GeoBounds {
    fn from(bounds: gc::index::GeoBounds) -> Self {
        Self {
            south: bounds.south(),
            west: bounds.west(),
            north: bounds.north(),
            east: bounds.east(),
            crs: bounds.crs().into(),
        }
    }
}

impl TryFrom<GeoBounds> for gc::index::GeoBounds {
    type Error = gc::Error;

    fn try_from(bounds: GeoBounds) -> Result<Self, gc::Error> {
        Self::new(
            bounds.south,
            bounds.west,
            bounds.north,
            bounds.east,
            bounds.crs.into(),
        )
    }
}

/// Checks a hand-built [`GeoBounds`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` naming the edge when it is non-finite or out of range,
/// and field `south` when south lies north of north.
#[uniffi::export]
pub fn geo_bounds_validate(bounds: GeoBounds) -> Result<(), GeoError> {
    gc::index::GeoBounds::try_from(bounds)?;
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
    Ok(gc::index::GeoBounds::from_points(points)?.into())
}

/// Whether the box crosses the antimeridian: its west edge lies east of its east
/// edge.
///
/// # Errors
/// `GeoError::InvalidValue` for an invalid [`GeoBounds`] record.
#[uniffi::export]
pub fn geo_bounds_crosses_antimeridian(bounds: GeoBounds) -> Result<bool, GeoError> {
    Ok(gc::index::GeoBounds::try_from(bounds)?.crosses_antimeridian())
}

/// Whether `point` lies in the box, edges included. A pole is inside a box that
/// reaches it, whatever the box's longitudes.
///
/// # Errors
/// `GeoError::CrsMismatch` when `point` is not on the box's reference system;
/// `GeoError::InvalidValue` / `GeoError::OutOfRange` for an invalid record.
#[uniffi::export]
pub fn geo_bounds_contains(bounds: GeoBounds, point: Coordinate) -> Result<bool, GeoError> {
    let bounds = gc::index::GeoBounds::try_from(bounds)?;
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
        gc::index::GeoBounds::try_from(a)?,
        gc::index::GeoBounds::try_from(b)?,
    );
    Ok(a.intersects(&b)?)
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
        gc::index::GeoBounds::try_from(a)?,
        gc::index::GeoBounds::try_from(b)?,
    );
    Ok(a.union(&b)?.into())
}

/// The middle of the box in latitude and longitude: the midpoint of the edges, not
/// a geodesic centroid.
///
/// # Errors
/// `GeoError::InvalidValue` for an invalid [`GeoBounds`] record.
#[uniffi::export]
pub fn geo_bounds_center(bounds: GeoBounds) -> Result<Coordinate, GeoError> {
    Ok(gc::index::GeoBounds::try_from(bounds)?.center().into())
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
    let bounds = gc::index::GeoBounds::try_from(bounds)?;
    Ok(bounds
        .expand_by(gc::Length::from_meters(distance_m))?
        .into())
}

/// One query result from a [`SpatialIndex`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct SpatialNeighbor {
    /// The entry's coordinate, as it was inserted.
    pub coordinate: Coordinate,
    /// The entry's position in the list the index was built from.
    pub index: u64,
    /// The exact geodesic distance from the query point, in meters.
    pub distance_m: f64,
}

fn neighbors(hits: Vec<gc::index::Neighbor<'_, ()>>) -> Vec<SpatialNeighbor> {
    hits.into_iter()
        .map(|hit| SpatialNeighbor {
            coordinate: hit.coordinate().into(),
            index: hit.index() as u64,
            distance_m: hit.distance().meters(),
        })
        .collect()
}

/// A static spatial index answering exact geodesic radius and k-nearest queries —
/// the object form of [`gc::index::SpatialIndex`].
///
/// Built once and immutable, so it is safe to share between threads. Results are
/// ordered by distance, then by position in the build list.
#[derive(Debug, uniffi::Object)]
pub struct SpatialIndex {
    inner: gc::index::SpatialIndex<()>,
}

#[uniffi::export]
impl SpatialIndex {
    /// Builds the index from `points`, all on one reference system. An empty list
    /// is a valid index whose queries return nothing.
    ///
    /// # Errors
    /// `GeoError::CrsMismatch` when the points' reference systems differ or are
    /// GCJ-02 / BD-09; `GeoError::OutOfRange` / `GeoError::InvalidValue` for an
    /// invalid coordinate record.
    #[uniffi::constructor]
    pub fn new(points: Vec<Coordinate>) -> Result<Arc<Self>, GeoError> {
        let entries = points
            .into_iter()
            .map(|point| gc::Coordinate::try_from(point).map(|point| (point, ())))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Arc::new(Self {
            inner: gc::index::SpatialIndex::new(entries)?,
        }))
    }

    /// Every entry within geodesic `radius_m` meters of `center`, the radius
    /// included.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `radius`) when `radius_m` is negative or
    /// non-finite; `GeoError::CrsMismatch` when `center` is on GCJ-02 / BD-09 or on
    /// another reference system than the entries; `GeoError::OutOfRange` /
    /// `GeoError::InvalidValue` for an invalid coordinate record.
    pub fn within(
        &self,
        center: Coordinate,
        radius_m: f64,
    ) -> Result<Vec<SpatialNeighbor>, GeoError> {
        let radius = gc::Length::from_meters(radius_m);
        Ok(neighbors(self.inner.within(center.try_into()?, radius)?))
    }

    /// The `k` entries nearest `center` by geodesic distance, or every entry when
    /// there are fewer than `k`.
    ///
    /// # Errors
    /// `GeoError::CrsMismatch` when `center` is on GCJ-02 / BD-09 or on another
    /// reference system than the entries; `GeoError::OutOfRange` /
    /// `GeoError::InvalidValue` for an invalid coordinate record.
    pub fn nearest(&self, center: Coordinate, k: u32) -> Result<Vec<SpatialNeighbor>, GeoError> {
        Ok(neighbors(
            self.inner.nearest(center.try_into()?, k as usize)?,
        ))
    }

    /// The number of entries.
    pub fn len(&self) -> u64 {
        self.inner.len() as u64
    }

    /// Whether the index has no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// The reference system of the entries, or `None` for an empty index.
    pub fn crs(&self) -> Option<Crs> {
        self.inner.crs().map(Into::into)
    }
}
