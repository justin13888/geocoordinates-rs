//! Centroids, density and map clustering, grid buckets, and visits and trips.
//!
//! `Approx<Coordinate>` is flattened into `centroid` and `centroid_max_error_m`
//! fields on each record, so a [`Visit`] can come back in as input without
//! [`ApproxCoordinate`](crate::ApproxCoordinate) becoming an input type. Member and
//! visit positions cross as `u64`. [`ClusterIndex`] is an object, whose updates
//! take a lock, so it is safe to share between threads.

use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, SystemTime};

use geocoordinates as gc;

use crate::{ApproxCoordinate, Coordinate, Crs, Fix, GeoBounds, GeoError, approx_coordinate};

fn core_points(points: Vec<Coordinate>) -> Result<Vec<gc::Coordinate>, gc::Error> {
    points.into_iter().map(gc::Coordinate::try_from).collect()
}

/// A group of input points with its summary — mirror of [`gc::cluster::Cluster`].
///
/// Plain data: [`cluster_validate`] checks a hand-built one.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Cluster {
    /// The position among the clusters of one result, by earliest member.
    pub id: u64,
    /// The mean of the members, which is approximate.
    pub centroid: Coordinate,
    /// An upper bound on the geodesic distance from the centroid to its farthest
    /// member, in meters.
    pub centroid_max_error_m: f64,
    /// The smallest box holding every member.
    pub bounds: GeoBounds,
    /// The number of members.
    pub count: u64,
    /// The members' positions in the input, strictly ascending.
    pub members: Vec<u64>,
}

impl From<&gc::cluster::Cluster> for Cluster {
    fn from(cluster: &gc::cluster::Cluster) -> Self {
        let centroid = cluster.centroid();
        Self {
            id: cluster.id() as u64,
            centroid: (*centroid.value()).into(),
            centroid_max_error_m: centroid.max_error().meters(),
            bounds: cluster.bounds().into(),
            count: cluster.count() as u64,
            members: cluster.members().iter().map(|m| *m as u64).collect(),
        }
    }
}

impl TryFrom<Cluster> for gc::cluster::Cluster {
    type Error = gc::Error;

    fn try_from(cluster: Cluster) -> Result<Self, gc::Error> {
        if cluster.count != cluster.members.len() as u64 {
            return Err(gc::Error::InvalidValue {
                field: "count",
                detail: format!(
                    "{} does not match {} members",
                    cluster.count,
                    cluster.members.len()
                ),
            });
        }
        let centroid = gc::Approx::new(
            gc::Coordinate::try_from(cluster.centroid)?,
            gc::Length::from_meters(cluster.centroid_max_error_m),
        )?;
        Self::new(
            cluster.id as usize,
            centroid,
            cluster.bounds.try_into()?,
            cluster.members.iter().map(|m| *m as usize).collect(),
        )
    }
}

/// Checks a hand-built [`Cluster`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` with field `members` when they are empty or not strictly
/// ascending, field `count` when it does not match them, or for an invalid bound or
/// bounds; `GeoError::CrsMismatch` when the centroid and bounds disagree;
/// `GeoError::OutOfRange` for an invalid centroid.
#[uniffi::export]
pub fn cluster_validate(cluster: Cluster) -> Result<(), GeoError> {
    gc::cluster::Cluster::try_from(cluster)?;
    Ok(())
}

/// The mean position of a set of points: the normalized mean of their unit vectors,
/// well defined across ±180° and at the poles. Approximate: `max_error_m` bounds
/// the geodesic distance from the result to the farthest input, tight to about 1%.
///
/// # Errors
/// `GeoError::InvalidValue` (field `points`) for no points, or for points whose unit
/// vectors cancel, such as an antipodal pair; `GeoError::CrsMismatch` when the
/// reference systems differ or are GCJ-02 / BD-09; `GeoError::OutOfRange` for an
/// invalid coordinate record.
#[uniffi::export]
pub fn centroid(points: Vec<Coordinate>) -> Result<ApproxCoordinate, GeoError> {
    Ok(approx_coordinate(gc::cluster::centroid(&core_points(
        points,
    )?)?))
}

/// What [`dbscan`] made of one input point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PointLabel {
    /// In no cluster.
    Noise,
    /// A member of the cluster with this id.
    Cluster {
        /// The cluster's id.
        id: u64,
    },
}

/// The result of [`dbscan`]: one label per input point, and the clusters.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DbscanResult {
    /// One label per input point, in input order.
    pub labels: Vec<PointLabel>,
    /// The clusters, ordered by id.
    pub clusters: Vec<Cluster>,
}

/// Density clusters with a geodesic `eps_m` in meters. Noise points are labelled, and
/// the result does not depend on input order: ids count up by earliest member, and a
/// border point joins the cluster of its nearest core point.
///
/// # Errors
/// `GeoError::InvalidValue` with field `eps` when `eps_m` is negative or non-finite,
/// field `min_points` when it is zero, and field `points` when a cluster's unit
/// vectors cancel; `GeoError::CrsMismatch` when the reference systems differ or are
/// GCJ-02 / BD-09; `GeoError::OutOfRange` for an invalid coordinate record.
#[uniffi::export]
pub fn dbscan(
    points: Vec<Coordinate>,
    eps_m: f64,
    min_points: u32,
) -> Result<DbscanResult, GeoError> {
    let found = gc::cluster::dbscan(
        &core_points(points)?,
        gc::Length::from_meters(eps_m),
        min_points as usize,
    )?;
    Ok(DbscanResult {
        labels: found
            .labels()
            .iter()
            .map(|label| match label {
                gc::cluster::PointLabel::Noise => PointLabel::Noise,
                gc::cluster::PointLabel::Cluster { id } => PointLabel::Cluster { id: *id as u64 },
            })
            .collect(),
        clusters: found.clusters().iter().map(Into::into).collect(),
    })
}

/// One occupied grid cell and the points in it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GridBucket {
    /// The cell: a geohash, or an H3 index in lowercase hexadecimal.
    pub key: String,
    /// The cell's points.
    pub cluster: Cluster,
}

fn grid_buckets(buckets: Vec<gc::cluster::GridBucket>) -> Vec<GridBucket> {
    buckets
        .iter()
        .map(|bucket| GridBucket {
            key: bucket.key().to_owned(),
            cluster: bucket.cluster().into(),
        })
        .collect()
}

/// Buckets WGS-84 points by geohash cell: the cheap grouping for very large sets.
/// The antimeridian and the poles are always cell edges, so a tight group across
/// ±180° makes two buckets.
///
/// # Errors
/// `GeoError::InvalidValue` for a `length` outside the geohash range;
/// `GeoError::CrsMismatch` for a non-WGS-84 point; `GeoError::OutOfRange` for an
/// invalid coordinate record.
#[uniffi::export]
pub fn bucket_by_geohash(points: Vec<Coordinate>, length: u8) -> Result<Vec<GridBucket>, GeoError> {
    Ok(grid_buckets(gc::cluster::bucket_by_geohash(
        &core_points(points)?,
        length,
    )?))
}

/// Buckets WGS-84 points by H3 cell: the cheap grouping for very large sets.
///
/// # Errors
/// `GeoError::InvalidValue` for a `resolution` above 15; `GeoError::CrsMismatch` for
/// a non-WGS-84 point; `GeoError::OutOfRange` for an invalid coordinate record.
#[uniffi::export]
pub fn bucket_by_h3(points: Vec<Coordinate>, resolution: u8) -> Result<Vec<GridBucket>, GeoError> {
    Ok(grid_buckets(gc::cluster::bucket_by_h3(
        &core_points(points)?,
        resolution,
    )?))
}

/// A stay point — mirror of [`gc::cluster::Visit`].
///
/// Plain data: [`visit_validate`] checks a hand-built one.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Visit {
    /// The visit's fixes, as positions in the list given to [`segment_visits`].
    pub cluster: Cluster,
    /// The time of the first fix.
    pub arrival: SystemTime,
    /// The time of the last fix.
    pub departure: SystemTime,
}

impl TryFrom<Visit> for gc::cluster::Visit {
    type Error = gc::Error;

    fn try_from(visit: Visit) -> Result<Self, gc::Error> {
        Self::new(visit.cluster.try_into()?, visit.arrival, visit.departure)
    }
}

/// Checks a hand-built [`Visit`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` with field `departure` when it precedes the arrival, or
/// as [`cluster_validate`] says for its cluster; `GeoError::CrsMismatch` and
/// `GeoError::OutOfRange` likewise.
#[uniffi::export]
pub fn visit_validate(visit: Visit) -> Result<(), GeoError> {
    gc::cluster::Visit::try_from(visit)?;
    Ok(())
}

/// Stay points from timestamped fixes: maximal runs in time order within `distance_m`
/// meters of the run's first fix and lasting at least `dwell`. Does not depend on the
/// order of the input.
///
/// # Errors
/// `GeoError::InvalidValue` with field `distance` when `distance_m` is negative or
/// non-finite, and field `timestamp` for a fix with no timestamp;
/// `GeoError::CrsMismatch` when the reference systems differ or are GCJ-02 / BD-09;
/// `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid record.
#[uniffi::export]
pub fn segment_visits(
    fixes: Vec<Fix>,
    distance_m: f64,
    dwell: Duration,
) -> Result<Vec<Visit>, GeoError> {
    let fixes = fixes
        .into_iter()
        .map(gc::fix::Fix::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let visits = gc::cluster::segment_visits(&fixes, gc::Length::from_meters(distance_m), dwell)?;
    Ok(visits
        .iter()
        .map(|visit| Visit {
            cluster: visit.cluster().into(),
            arrival: visit.arrival(),
            departure: visit.departure(),
        })
        .collect())
}

/// A run of [`Visit`]s — mirror of [`gc::cluster::Trip`].
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Trip {
    /// The visits, as positions in the list given to [`segment_trips`], by arrival.
    pub visits: Vec<u64>,
    /// The earliest arrival.
    pub start: SystemTime,
    /// The latest departure.
    pub end: SystemTime,
    /// A box holding the bounds of every visit.
    pub bounds: GeoBounds,
}

/// Groups visits into trips: taken by arrival, a visit that begins more than `gap`
/// after every earlier visit has ended starts a new trip.
///
/// # Errors
/// `GeoError::CrsMismatch` when the visits' reference systems differ, and as
/// [`visit_validate`] says for an invalid [`Visit`] record.
#[uniffi::export]
pub fn segment_trips(visits: Vec<Visit>, gap: Duration) -> Result<Vec<Trip>, GeoError> {
    let visits = visits
        .into_iter()
        .map(gc::cluster::Visit::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(gc::cluster::segment_trips(&visits, gap)?
        .iter()
        .map(|trip| Trip {
            visits: trip.visits().iter().map(|v| *v as u64).collect(),
            start: trip.start(),
            end: trip.end(),
            bounds: trip.bounds().into(),
        })
        .collect())
}

/// How a [`ClusterIndex`] measures closeness — mirror of
/// [`gc::cluster::ClusterIndexOptions`]. Checked when an index is built.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ClusterIndexOptions {
    /// The merge radius, in pixels, the same at every zoom.
    pub radius_px: f64,
    /// The width of a map tile, in pixels.
    pub tile_px: f64,
}

impl From<ClusterIndexOptions> for gc::cluster::ClusterIndexOptions {
    fn from(options: ClusterIndexOptions) -> Self {
        Self::default()
            .with_radius_px(options.radius_px)
            .with_tile_px(options.tile_px)
    }
}

/// The default options: a radius of 40 pixels on 512-pixel tiles.
#[uniffi::export]
pub fn cluster_index_options_default() -> ClusterIndexOptions {
    let options = gc::cluster::ClusterIndexOptions::default();
    ClusterIndexOptions {
        radius_px: options.radius_px,
        tile_px: options.tile_px,
    }
}

/// One cluster or point at one zoom — mirror of [`gc::cluster::MapCluster`]. Its
/// member ids come from [`ClusterIndex::leaves`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MapCluster {
    /// The id, for [`ClusterIndex::children`] and [`ClusterIndex::leaves`].
    pub id: u64,
    /// The zoom this is drawn at; a point under a zoom-22 cluster has 23.
    pub zoom: u8,
    /// The mean of the cluster's points, which is approximate.
    pub centroid: Coordinate,
    /// An upper bound on the geodesic distance from the centroid to its farthest
    /// point, in meters; `0` for a lone point.
    pub centroid_max_error_m: f64,
    /// A box holding every point of the cluster.
    pub bounds: GeoBounds,
    /// The number of points.
    pub count: u64,
}

impl From<&gc::cluster::MapCluster> for MapCluster {
    fn from(pin: &gc::cluster::MapCluster) -> Self {
        let centroid = pin.centroid();
        Self {
            id: pin.id(),
            zoom: pin.zoom(),
            centroid: (*centroid.value()).into(),
            centroid_max_error_m: centroid.max_error().meters(),
            bounds: pin.bounds().into(),
            count: pin.count(),
        }
    }
}

/// A per-zoom hierarchy for map pins, zooms 0 to 22, with updates that need no rebuild
/// — the object form of [`gc::cluster::ClusterIndex`]. The clusters are a function of
/// the set of points, never of the order they arrived in.
#[derive(Debug, uniffi::Object)]
pub struct ClusterIndex {
    inner: RwLock<gc::cluster::ClusterIndex>,
}

impl ClusterIndex {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, gc::cluster::ClusterIndex> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, gc::cluster::ClusterIndex> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }
}

fn pins(found: &[gc::cluster::MapCluster]) -> Vec<MapCluster> {
    found.iter().map(Into::into).collect()
}

#[uniffi::export]
impl ClusterIndex {
    /// Builds the hierarchy over `points`, whose ids are their positions.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` with field `radius_px` or `tile_px` unless the option
    /// is finite and positive; `GeoError::CrsMismatch` when the reference systems
    /// differ or are GCJ-02 / BD-09; `GeoError::OutOfRange` for an invalid coordinate
    /// record.
    #[uniffi::constructor]
    pub fn new(
        points: Vec<Coordinate>,
        options: ClusterIndexOptions,
    ) -> Result<Arc<Self>, GeoError> {
        let inner = gc::cluster::ClusterIndex::new(core_points(points)?, options.into())?;
        Ok(Arc::new(Self {
            inner: RwLock::new(inner),
        }))
    }

    /// The number of points.
    pub fn len(&self) -> u64 {
        self.read().len() as u64
    }

    /// Whether the index has no points.
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    /// The reference system of the points, or `None` while the index has none.
    pub fn crs(&self) -> Option<Crs> {
        self.read().crs().map(Into::into)
    }

    /// The position of the point `point_id`, or `None` if there is no such point.
    pub fn coordinate(&self, point_id: u64) -> Option<Coordinate> {
        self.read().coordinate(point_id).map(Into::into)
    }

    /// Adds a point without a rebuild, returning its new id.
    ///
    /// # Errors
    /// `GeoError::CrsMismatch` when `point` is on GCJ-02 / BD-09 or another reference
    /// system than the points held; `GeoError::OutOfRange` for an invalid record.
    pub fn insert(&self, point: Coordinate) -> Result<u64, GeoError> {
        Ok(self.write().insert(point.try_into()?)?)
    }

    /// Removes the point `point_id` without a rebuild, returning its position. Its id
    /// is not reused.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `point_id`) when there is no such point.
    pub fn remove(&self, point_id: u64) -> Result<Coordinate, GeoError> {
        Ok(self.write().remove(point_id)?.into())
    }

    /// The clusters at `zoom` whose centroids lie in `bounds`, ordered by id.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `zoom`) above 22, or for an invalid
    /// [`GeoBounds`] record; `GeoError::CrsMismatch` when `bounds` is on another
    /// reference system than the points.
    pub fn clusters(&self, bounds: GeoBounds, zoom: u8) -> Result<Vec<MapCluster>, GeoError> {
        Ok(pins(&self.read().clusters(bounds.try_into()?, zoom)?))
    }

    /// The clusters at the next zoom that `cluster_id` splits into, or the point under
    /// a zoom-22 cluster. A point has no children.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `cluster_id`) when there is no such cluster.
    pub fn children(&self, cluster_id: u64) -> Result<Vec<MapCluster>, GeoError> {
        Ok(pins(&self.read().children(cluster_id)?))
    }

    /// The ids of up to `limit` points of `cluster_id`, after skipping `offset`.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `cluster_id`) when there is no such cluster.
    pub fn leaves(&self, cluster_id: u64, limit: u32, offset: u64) -> Result<Vec<u64>, GeoError> {
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        Ok(self.read().leaves(cluster_id, limit as usize, offset)?)
    }
}
