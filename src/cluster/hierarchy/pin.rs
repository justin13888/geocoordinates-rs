//! [`MapCluster`], re-exported flat from [`cluster`](crate::cluster), and the ids that
//! name one.

use super::forest::{Aggregate, LEAF_LEVEL};
#[cfg(doc)]
use super::{ClusterIndex, MAX_ZOOM};
use crate::coord::Coordinate;
#[cfg(feature = "serde")]
use crate::error::Error;
use crate::error::Result;
use crate::index::GeoBounds;
use crate::{Approx, Length};

/// One cluster or point at one zoom of a [`ClusterIndex`].
///
/// It holds no member list, which would cost a whole index per query at low zoom:
/// [`ClusterIndex::leaves`] pages through the member ids instead.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex, index::GeoBounds};
///
/// let here = Coordinate::wgs84(1.0, 2.0)?;
/// let index = ClusterIndex::new([here, here], Default::default())?;
/// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
/// let pin = index.clusters(world, 3)?[0];
/// assert_eq!(pin.count(), 2);
/// assert_eq!(index.leaves(pin.id(), 10, 0)?, [0, 1]);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "MapClusterRepr"))]
pub struct MapCluster {
    id: u64,
    zoom: u8,
    centroid: Approx<Coordinate>,
    bounds: GeoBounds,
    count: u64,
}

/// The unchecked wire shape of a [`MapCluster`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct MapClusterRepr {
    id: u64,
    zoom: u8,
    centroid: Approx<Coordinate>,
    bounds: GeoBounds,
    count: u64,
}

#[cfg(feature = "serde")]
impl TryFrom<MapClusterRepr> for MapCluster {
    type Error = Error;

    fn try_from(repr: MapClusterRepr) -> Result<Self> {
        let level = decode_id(repr.id).map(|(level, _)| level);
        if level != Some(usize::from(repr.zoom)) {
            return Err(Error::InvalidValue {
                field: "id",
                detail: format!("{} does not name a cluster at zoom {}", repr.id, repr.zoom),
            });
        }
        if repr.count == 0 {
            return Err(Error::InvalidValue {
                field: "count",
                detail: "a cluster holds at least one point".into(),
            });
        }
        if repr.centroid.value().crs() != repr.bounds.crs() {
            return Err(Error::CrsMismatch {
                expected: repr.bounds.crs(),
                found: repr.centroid.value().crs(),
            });
        }
        Ok(Self {
            id: repr.id,
            zoom: repr.zoom,
            centroid: repr.centroid,
            bounds: repr.bounds,
            count: repr.count,
        })
    }
}

impl MapCluster {
    pub(super) fn from_node(level: usize, id: u32, aggregate: &Aggregate) -> Result<Self> {
        Ok(Self {
            id: encode_id(level, id),
            zoom: level as u8,
            centroid: Approx::new(
                aggregate.centroid,
                Length::from_meters(aggregate.max_error_m),
            )?,
            bounds: aggregate.bounds,
            count: aggregate.count,
        })
    }

    /// The cluster's id, for [`ClusterIndex::children`] and [`ClusterIndex::leaves`].
    /// It names the zoom and the cluster's seed point, so it is stable for the same
    /// input, and stays the same across updates that leave the seed's role alone.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let index = ClusterIndex::new([Coordinate::wgs84(1.0, 2.0)?], Default::default())?;
    /// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    /// let id = index.clusters(world, 5)?[0].id();
    /// assert_eq!(index.leaves(id, 1, 0)?, [0]);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// The zoom this cluster is drawn at. A single point returned by
    /// [`ClusterIndex::children`] of a cluster at [`MAX_ZOOM`] has one more.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let index = ClusterIndex::new([Coordinate::wgs84(1.0, 2.0)?], Default::default())?;
    /// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    /// assert_eq!(index.clusters(world, 5)?[0].zoom(), 5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn zoom(&self) -> u8 {
        self.zoom
    }

    /// The mean of the cluster's points, built up from its children's.
    ///
    /// The bound is an upper limit on the geodesic distance to the farthest point,
    /// summed up the hierarchy with the triangle inequality, so it can exceed
    /// [`centroid`](crate::cluster::centroid)'s for the same points. A lone point is
    /// exact.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let here = Coordinate::wgs84(1.0, 2.0)?;
    /// let index = ClusterIndex::new([here], Default::default())?;
    /// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    /// let pin = index.clusters(world, 5)?[0];
    /// assert_eq!(*pin.centroid().value(), here);
    /// assert_eq!(pin.centroid().max_error().meters(), 0.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn centroid(&self) -> Approx<Coordinate> {
        self.centroid
    }

    /// A box holding every point of the cluster, built from its children's boxes, so
    /// it can be a little larger than the smallest such box.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let index = ClusterIndex::new([Coordinate::wgs84(1.0, 2.0)?], Default::default())?;
    /// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    /// assert_eq!(index.clusters(world, 5)?[0].bounds().north(), 1.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn bounds(&self) -> GeoBounds {
        self.bounds
    }

    /// The number of points in the cluster.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, cluster::ClusterIndex, index::GeoBounds};
    ///
    /// let index = ClusterIndex::new([Coordinate::wgs84(1.0, 2.0)?], Default::default())?;
    /// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
    /// assert_eq!(index.clusters(world, 5)?[0].count(), 1);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }
}

/// A cluster id packs the level above the 32 bits of its seed point's id.
const fn encode_id(level: usize, point: u32) -> u64 {
    ((level as u64) << 32) | point as u64
}

/// The level and seed point an id names, if its level exists.
pub(super) fn decode_id(id: u64) -> Option<(usize, u32)> {
    let level = usize::try_from(id >> 32).ok()?;
    (level <= LEAF_LEVEL).then_some((level, id as u32))
}
