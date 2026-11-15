//! The geodesic spatial index.
//!
//! `SpatialIndex<T>` is generic, so it crosses as the concrete object
//! [`SpatialIndex`] over coordinates alone: each result carries the entry's
//! insertion position, which a foreign caller uses to look up its own payload.
//! The `GeoBounds` record and its `geo_bounds_*` functions are geodesy.

use std::sync::Arc;

use geocoordinates as gc;

use crate::{Coordinate, Crs, GeoError};

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
