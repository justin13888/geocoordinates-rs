//! [`ClusterIndex`], re-exported flat from [`cluster`](super).

mod forest;
mod pin;

use self::forest::{DEEPEST, Forest, LEAF_LEVEL, world_y};
pub use self::pin::MapCluster;
use self::pin::decode_id;
use super::centroid::normal_radius_m;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::index::GeoBounds;

/// The deepest zoom a [`ClusterIndex`] clusters at. Zooms run from 0 to this.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, cluster::{ClusterIndex, MAX_ZOOM}, index::GeoBounds};
///
/// let index = ClusterIndex::new([Coordinate::wgs84(1.0, 2.0)?], Default::default())?;
/// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
/// assert_eq!(index.clusters(world, MAX_ZOOM)?.len(), 1);
/// assert!(matches!(
///     index.clusters(world, MAX_ZOOM + 1),
///     Err(Error::InvalidValue { field: "zoom", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub const MAX_ZOOM: u8 = DEEPEST as u8;

/// How a [`ClusterIndex`] measures closeness: the pixel radius it merges within, and
/// the width of a tile in pixels. Defaults to a radius of 40 on 512-pixel tiles.
///
/// ```
/// use geocoordinates::cluster::ClusterIndexOptions;
///
/// let options = ClusterIndexOptions::default().with_radius_px(60.0).with_tile_px(256.0);
/// assert_eq!((options.radius_px, options.tile_px), (60.0, 256.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct ClusterIndexOptions {
    /// The merge radius, in pixels, which is the same at every zoom.
    pub radius_px: f64,
    /// The width of a map tile, in pixels; the world is one tile wide at zoom 0.
    pub tile_px: f64,
}

impl Default for ClusterIndexOptions {
    fn default() -> Self {
        Self {
            radius_px: 40.0,
            tile_px: 512.0,
        }
    }
}

impl ClusterIndexOptions {
    /// Sets the merge radius, in pixels.
    ///
    /// ```
    /// use geocoordinates::cluster::ClusterIndexOptions;
    ///
    /// assert_eq!(ClusterIndexOptions::default().with_radius_px(10.0).radius_px, 10.0);
    /// ```
    #[must_use]
    pub const fn with_radius_px(mut self, radius_px: f64) -> Self {
        self.radius_px = radius_px;
        self
    }

    /// Sets the width of a map tile, in pixels.
    ///
    /// ```
    /// use geocoordinates::cluster::ClusterIndexOptions;
    ///
    /// assert_eq!(ClusterIndexOptions::default().with_tile_px(256.0).tile_px, 256.0);
    /// ```
    #[must_use]
    pub const fn with_tile_px(mut self, tile_px: f64) -> Self {
        self.tile_px = tile_px;
        self
    }
}

/// A supercluster-style hierarchy for map pins: one level per Web Mercator zoom from
/// 0 to [`MAX_ZOOM`], each merging what lies within a fixed pixel radius, with
/// [`insert`](Self::insert) and [`remove`](Self::remove) that need no rebuild.
///
/// **What "near" means here.** Distance is measured on the Web Mercator square, as a
/// map draws it, and not on the ellipsoid, because the radius is a promise about
/// pixels. So it wraps across the antimeridian, where a map's edges meet, while
/// points past about ±85.05° clamp to the map's top or bottom edge and are told apart
/// by longitude alone. Over the pole, nearby points on opposite meridians stay
/// apart. Each cluster's centroid and bounds remain geodesic and correct there.
///
/// **Determinism.** The clusters are a function of the set of points, never of their
/// arrival order: seeds are taken by latitude, longitude and id, and every update
/// reaches the bit-for-bit result of a rebuild. Ids of the same input built the same
/// way are the same. Removed ids are not reused.
///
/// **Cost.** Building makes about `3 × 23` tree lookups per point, and an update
/// costs the same order times the neighbors a changed seed disturbs. Memory is
/// about 60 bytes per point per zoom. STABILIZATION.md records measurements; above
/// about 10⁶ points, prefer [`bucket_by_geohash`](super::bucket_by_geohash).
///
/// Heights are ignored, and every point shares a CRS with an ellipsoid.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex, index::GeoBounds};
///
/// let mut index = ClusterIndex::new(
///     [Coordinate::wgs84(0.0, 179.99)?, Coordinate::wgs84(0.0, -179.99)?],
///     Default::default(),
/// )?;
/// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
/// // Merged across the antimeridian at low zoom, apart at high zoom.
/// assert_eq!(index.clusters(world, 2)?.len(), 1);
/// assert_eq!(index.clusters(world, 15)?.len(), 2);
///
/// let added = index.insert(Coordinate::wgs84(40.0, 0.0)?)?;
/// assert_eq!(index.clusters(world, 2)?.len(), 2);
/// index.remove(added)?;
/// assert_eq!(index.clusters(world, 2)?.len(), 1);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug)]
pub struct ClusterIndex {
    options: ClusterIndexOptions,
    /// The forest, once there is a point to fix its CRS. Emptying the index drops
    /// it, so the CRS does not outlive the last point.
    forest: Option<Forest>,
    crs: Option<Crs>,
    len: usize,
    next_id: u32,
}

impl ClusterIndex {
    /// Builds the hierarchy over `points`, whose ids are their positions. An empty
    /// set is a valid index, and its first [`insert`](Self::insert) fixes its CRS.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] with `field: "radius_px"` or `"tile_px"` unless the
    /// option is finite and positive, and `field: "points"` beyond 2³² - 1 points;
    /// [`Error::CrsMismatch`] when the CRSs differ or are GCJ-02 / BD-09.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error};
    /// use geocoordinates::cluster::{ClusterIndex, ClusterIndexOptions};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// assert_eq!(ClusterIndex::new([point], Default::default())?.len(), 1);
    /// assert!(matches!(
    ///     ClusterIndex::new([point], ClusterIndexOptions::default().with_radius_px(0.0)),
    ///     Err(Error::InvalidValue { field: "radius_px", .. })
    /// ));
    /// assert!(matches!(
    ///     ClusterIndex::new([Coordinate::gcj02(1.0, 2.0)?], Default::default()),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(
        points: impl IntoIterator<Item = Coordinate>,
        options: ClusterIndexOptions,
    ) -> Result<Self> {
        for (field, value) in [
            ("radius_px", options.radius_px),
            ("tile_px", options.tile_px),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(Error::InvalidValue {
                    field,
                    detail: format!("{value} is not finite and positive"),
                });
            }
        }
        let mut index = Self {
            options,
            forest: None,
            crs: None,
            len: 0,
            next_id: 0,
        };
        let points: Vec<Coordinate> = points.into_iter().collect();
        let Some(first) = points.first() else {
            return Ok(index);
        };
        let crs = first.crs();
        let normal_radius = normal_radius_m(crs)?;
        if let Some(other) = points.iter().find(|point| point.crs() != crs) {
            return Err(Error::CrsMismatch {
                expected: crs,
                found: other.crs(),
            });
        }
        let next_id = u32::try_from(points.len())
            .ok()
            .filter(|count| *count < u32::MAX)
            .ok_or_else(|| Error::InvalidValue {
                field: "points",
                detail: "an index holds at most 2^32 - 1 points".into(),
            })?;
        index.forest = Some(Forest::build(&points, crs, normal_radius, index.radii()));
        index.crs = Some(crs);
        index.len = points.len();
        index.next_id = next_id;
        Ok(index)
    }

    /// Each level's radius as a fraction of the world's width.
    fn radii(&self) -> Vec<f64> {
        (0..=DEEPEST as i32)
            .map(|zoom| self.options.radius_px / self.options.tile_px / 2.0_f64.powi(zoom))
            .collect()
    }

    /// The number of points.
    ///
    /// ```
    /// use geocoordinates::cluster::ClusterIndex;
    ///
    /// assert_eq!(ClusterIndex::new([], Default::default())?.len(), 0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the index has no points.
    ///
    /// ```
    /// use geocoordinates::cluster::ClusterIndex;
    ///
    /// assert!(ClusterIndex::new([], Default::default())?.is_empty());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The CRS of the points, or `None` while the index has none.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex};
    ///
    /// let index = ClusterIndex::new([Coordinate::wgs84(1.0, 2.0)?], Default::default())?;
    /// assert_eq!(index.crs(), Some(Crs::Wgs84));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn crs(&self) -> Option<Crs> {
        self.crs
    }

    /// The position of the point `point_id`, or `None` if there is no such point.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, cluster::ClusterIndex};
    ///
    /// let here = Coordinate::wgs84(1.0, 2.0)?;
    /// let index = ClusterIndex::new([here], Default::default())?;
    /// assert_eq!(index.coordinate(0), Some(here));
    /// assert_eq!(index.coordinate(1), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn coordinate(&self, point_id: u64) -> Option<Coordinate> {
        let id = u32::try_from(point_id).ok()?;
        let node = self.forest.as_ref()?.get(LEAF_LEVEL, id)?;
        Some(node.aggregate.centroid)
    }

    /// Adds a point without a rebuild, returning its new id. The result is the one a
    /// rebuild over the same points would give.
    ///
    /// # Errors
    ///
    /// [`Error::CrsMismatch`] when `point` is on GCJ-02 / BD-09 or on another CRS than
    /// the points already held; [`Error::InvalidValue`] (`field: "points"`) once 2³² - 1
    /// ids have been issued.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, cluster::ClusterIndex};
    ///
    /// let mut index = ClusterIndex::new([], Default::default())?;
    /// assert_eq!(index.insert(Coordinate::wgs84(1.0, 2.0)?)?, 0);
    /// assert_eq!(index.insert(Coordinate::wgs84(3.0, 4.0)?)?, 1);
    /// assert!(matches!(
    ///     index.insert(Coordinate::gcj02(1.0, 2.0)?),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn insert(&mut self, point: Coordinate) -> Result<u64> {
        let found = point.crs();
        let normal_radius = normal_radius_m(found)?;
        if let Some(crs) = self.crs.filter(|crs| *crs != found) {
            return Err(Error::CrsMismatch {
                expected: crs,
                found,
            });
        }
        if self.next_id == u32::MAX {
            return Err(Error::InvalidValue {
                field: "points",
                detail: "an index issues at most 2^32 - 1 ids".into(),
            });
        }
        let radii = self.radii();
        let forest = self
            .forest
            .get_or_insert_with(|| Forest::build(&[], found, normal_radius, radii));
        // An emptied forest may be taking its next point on another CRS.
        forest.set_model(found, normal_radius);
        let id = forest.insert(point);
        self.crs = Some(found);
        self.len += 1;
        self.next_id = id + 1;
        Ok(u64::from(id))
    }

    /// Removes the point `point_id` without a rebuild, returning its position. Its id
    /// is not reused. The result is the one a rebuild over the remaining points would
    /// give, and removing the last point lets the next insert choose the CRS.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "point_id"`) when there is no such point.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, cluster::ClusterIndex};
    ///
    /// let here = Coordinate::wgs84(1.0, 2.0)?;
    /// let mut index = ClusterIndex::new([here], Default::default())?;
    /// assert_eq!(index.remove(0)?, here);
    /// assert!(index.is_empty());
    /// assert!(matches!(index.remove(0), Err(Error::InvalidValue { field: "point_id", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn remove(&mut self, point_id: u64) -> Result<Coordinate> {
        let point = self
            .coordinate(point_id)
            .ok_or_else(|| Error::InvalidValue {
                field: "point_id",
                detail: format!("no point has id {point_id}"),
            })?;
        if let Some(forest) = &mut self.forest {
            forest.remove(point_id as u32);
        }
        self.len -= 1;
        if self.len == 0 {
            // Ids stay unique, so the emptied forest's id table keeps its length.
            self.crs = None;
        }
        Ok(point)
    }

    /// The clusters at `zoom` whose centroids lie in `bounds`, ordered by id.
    ///
    /// A box that crosses the antimeridian or reaches a pole works like any other.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "zoom"`) above [`MAX_ZOOM`];
    /// [`Error::CrsMismatch`] when `bounds` is on another CRS than the points.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let index = ClusterIndex::new(
    ///     [Coordinate::wgs84(0.0, 179.0)?, Coordinate::wgs84(0.0, 0.0)?],
    ///     Default::default(),
    /// )?;
    /// let pacific = GeoBounds::new(-5.0, 170.0, 5.0, -170.0, Crs::Wgs84)?;
    /// assert_eq!(index.clusters(pacific, 4)?.len(), 1);
    /// assert!(matches!(
    ///     index.clusters(pacific, 23),
    ///     Err(Error::InvalidValue { field: "zoom", .. })
    /// ));
    /// let tokyo = GeoBounds::new(-5.0, 170.0, 5.0, -170.0, Crs::Tokyo)?;
    /// assert!(matches!(index.clusters(tokyo, 4), Err(Error::CrsMismatch { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn clusters(&self, bounds: GeoBounds, zoom: u8) -> Result<Vec<MapCluster>> {
        if zoom > MAX_ZOOM {
            return Err(Error::InvalidValue {
                field: "zoom",
                detail: format!("{zoom} is above the deepest zoom {MAX_ZOOM}"),
            });
        }
        let Some(forest) = self.live_forest() else {
            return Ok(Vec::new());
        };
        if let Some(crs) = self.crs.filter(|crs| *crs != bounds.crs()) {
            return Err(Error::CrsMismatch {
                expected: crs,
                found: bounds.crs(),
            });
        }
        let level = usize::from(zoom);
        let span = (bounds.east() - bounds.west()).rem_euclid(360.0);
        // A box reaching a pole holds the pole at every longitude, so only the
        // full walk can find a centroid exactly there.
        let rows = (bounds.north() < 90.0 && bounds.south() > -90.0)
            .then(|| (world_y(bounds.north()), world_y(bounds.south())));
        let span = if bounds.west() == -180.0 && bounds.east() == 180.0 {
            360.0
        } else {
            span
        };
        let mut found = Vec::new();
        for id in forest.candidates(level, bounds.west(), span, rows) {
            let aggregate = &forest.node(level, id).aggregate;
            if bounds.contains(aggregate.centroid)? {
                found.push(MapCluster::from_node(level, id, aggregate)?);
            }
        }
        found.sort_unstable_by_key(MapCluster::id);
        Ok(found)
    }

    /// The clusters at the next zoom that `cluster_id` splits into, in canonical
    /// order, or the single point under a cluster at [`MAX_ZOOM`]. A point has no
    /// children.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "cluster_id"`) when there is no such cluster.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let index = ClusterIndex::new(
    ///     [Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 0.01)?],
    ///     Default::default(),
    /// )?;
    /// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    /// let top = index.clusters(world, 0)?[0];
    /// assert_eq!(top.count(), 2);
    /// let total: u64 = index.children(top.id())?.iter().map(|child| child.count()).sum();
    /// assert_eq!(total, 2);
    /// assert!(matches!(
    ///     index.children(u64::MAX),
    ///     Err(Error::InvalidValue { field: "cluster_id", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn children(&self, cluster_id: u64) -> Result<Vec<MapCluster>> {
        let (level, id) = self.resolve(cluster_id)?;
        if level == LEAF_LEVEL {
            return Ok(Vec::new());
        }
        let forest = self.live_forest().ok_or_else(|| unknown(cluster_id))?;
        forest
            .node(level, id)
            .children(id)
            .into_iter()
            .map(|child| {
                MapCluster::from_node(level + 1, child, &forest.node(level + 1, child).aggregate)
            })
            .collect()
    }

    /// The ids of up to `limit` points of `cluster_id`, after skipping `offset`, so a
    /// caller can page through a large cluster. The order is a depth-first walk with
    /// each cluster's children in canonical order; a page skips whole subtrees, so it
    /// costs about `limit`, not `offset`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "cluster_id"`) when there is no such cluster.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let here = Coordinate::wgs84(1.0, 2.0)?;
    /// let index = ClusterIndex::new([here, here, here], Default::default())?;
    /// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    /// let pin = index.clusters(world, 0)?[0].id();
    /// assert_eq!(index.leaves(pin, 2, 0)?, [0, 1]);
    /// assert_eq!(index.leaves(pin, 2, 2)?, [2]);
    /// assert!(matches!(
    ///     index.leaves(u64::MAX, 1, 0),
    ///     Err(Error::InvalidValue { field: "cluster_id", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn leaves(&self, cluster_id: u64, limit: usize, offset: usize) -> Result<Vec<u64>> {
        let (level, id) = self.resolve(cluster_id)?;
        let forest = self.live_forest().ok_or_else(|| unknown(cluster_id))?;
        let mut found = Vec::new();
        let mut skip = offset as u64;
        let mut stack = vec![(level, id)];
        while let Some((level, id)) = stack.pop() {
            if found.len() >= limit {
                break;
            }
            let node = forest.node(level, id);
            if node.aggregate.count <= skip {
                skip -= node.aggregate.count;
            } else if level == LEAF_LEVEL {
                found.push(u64::from(id));
            } else {
                let children = node.children(id);
                stack.extend(children.into_iter().rev().map(|child| (level + 1, child)));
            }
        }
        Ok(found)
    }

    /// The forest, unless every point has been removed.
    fn live_forest(&self) -> Option<&Forest> {
        self.forest.as_ref().filter(|_| self.len > 0)
    }

    fn resolve(&self, cluster_id: u64) -> Result<(usize, u32)> {
        decode_id(cluster_id)
            .filter(|(level, id)| {
                self.live_forest()
                    .is_some_and(|forest| forest.get(*level, *id).is_some())
            })
            .ok_or_else(|| unknown(cluster_id))
    }
}

fn unknown(cluster_id: u64) -> Error {
    Error::InvalidValue {
        field: "cluster_id",
        detail: format!("no cluster has id {cluster_id}"),
    }
}
