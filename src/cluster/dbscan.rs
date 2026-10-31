//! [`dbscan`], re-exported flat from [`cluster`](super).

use std::cmp::Ordering;

use super::centroid::centroid;
use super::types::Cluster;
use crate::Length;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::index::{GeoBounds, Neighbor, SpatialIndex};

/// What [`dbscan`] made of one input point.
///
/// ```
/// use geocoordinates::{Coordinate, Length, cluster::{PointLabel, dbscan}};
///
/// let lone = Coordinate::wgs84(1.0, 2.0)?;
/// let found = dbscan(&[lone], Length::from_meters(10.0), 2)?;
/// assert_eq!(found.labels(), [PointLabel::Noise]);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PointLabel {
    /// In no cluster: not a core point, and within `eps` of none.
    Noise,
    /// A member of the cluster with this id.
    Cluster {
        /// The cluster's [`id`](Cluster::id).
        id: usize,
    },
}

/// The result of [`dbscan`]: one label per input point, and the clusters.
///
/// ```
/// use geocoordinates::{Coordinate, Length, cluster::{PointLabel, dbscan}};
///
/// let here = Coordinate::wgs84(1.0, 2.0)?;
/// let found = dbscan(&[here, here], Length::from_meters(10.0), 2)?;
/// assert_eq!(found.labels()[1], PointLabel::Cluster { id: 0 });
/// assert_eq!(found.clusters().len(), 1);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "DbscanRepr"))]
pub struct Dbscan {
    labels: Vec<PointLabel>,
    clusters: Vec<Cluster>,
}

/// The unchecked wire shape of a [`Dbscan`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct DbscanRepr {
    labels: Vec<PointLabel>,
    clusters: Vec<Cluster>,
}

#[cfg(feature = "serde")]
impl TryFrom<DbscanRepr> for Dbscan {
    type Error = Error;

    fn try_from(repr: DbscanRepr) -> Result<Self> {
        let mut expected = vec![PointLabel::Noise; repr.labels.len()];
        for (position, cluster) in repr.clusters.iter().enumerate() {
            if cluster.id() != position {
                return Err(Error::InvalidValue {
                    field: "clusters",
                    detail: format!("cluster at {position} has id {}", cluster.id()),
                });
            }
            for member in cluster.members() {
                let slot = expected
                    .get_mut(*member)
                    .ok_or_else(|| Error::InvalidValue {
                        field: "clusters",
                        detail: format!("member {member} has no label"),
                    })?;
                if *slot != PointLabel::Noise {
                    return Err(Error::InvalidValue {
                        field: "clusters",
                        detail: format!("member {member} is listed in two clusters"),
                    });
                }
                *slot = PointLabel::Cluster { id: position };
            }
        }
        if expected != repr.labels {
            return Err(Error::InvalidValue {
                field: "labels",
                detail: "labels disagree with the clusters' members".into(),
            });
        }
        Ok(Self {
            labels: repr.labels,
            clusters: repr.clusters,
        })
    }
}

impl Dbscan {
    /// One label per input point, in input order.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Length, cluster::{PointLabel, dbscan}};
    ///
    /// let (a, far) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(9.0, 9.0)?);
    /// let found = dbscan(&[far, a, a], Length::from_meters(1.0), 2)?;
    /// assert_eq!(
    ///     found.labels(),
    ///     [PointLabel::Noise, PointLabel::Cluster { id: 0 }, PointLabel::Cluster { id: 0 }]
    /// );
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn labels(&self) -> &[PointLabel] {
        &self.labels
    }

    /// The clusters, ordered by id, which is the order of their earliest members.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Length, cluster::dbscan};
    ///
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(9.0, 9.0)?);
    /// let found = dbscan(&[b, a, b, a], Length::from_meters(1.0), 2)?;
    /// assert_eq!(found.clusters()[0].members(), [0, 2]);
    /// assert_eq!(found.clusters()[1].members(), [1, 3]);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn clusters(&self) -> &[Cluster] {
        &self.clusters
    }
}

/// The root of `node` in a union-find forest, halving the path on the way up.
fn find(parents: &mut [usize], mut node: usize) -> usize {
    // A path visits each node at most once; the bound keeps this loop finite whatever
    // the forest holds.
    for _ in 0..parents.len() {
        if parents[node] == node {
            break;
        }
        parents[node] = parents[parents[node]];
        node = parents[node];
    }
    node
}

/// The order that picks a border point's cluster: nearest core point first, then
/// the lower latitude and longitude, neither of which depends on the input order.
fn nearer_core(a: &Neighbor<'_, ()>, b: &Neighbor<'_, ()>) -> Ordering {
    let (a_at, b_at) = (a.coordinate(), b.coordinate());
    a.distance()
        .meters()
        .total_cmp(&b.distance().meters())
        .then(a_at.lat().total_cmp(&b_at.lat()))
        .then(a_at.lon().total_cmp(&b_at.lon()))
}

/// Density clusters by DBSCAN, with a geodesic `eps` over the
/// [`SpatialIndex`]. Noise points are labelled, not dropped.
///
/// A point is *core* when at least `min_points` inputs, itself included, lie within
/// `eps` of it, the distance included. Core points within `eps` of each other share
/// a cluster, and a non-core point within `eps` of a core point joins one.
///
/// The result does not depend on input order. Textbook DBSCAN gives a border point
/// to whichever of its clusters reached it first; here it joins the cluster of its
/// nearest core point, ties going to the lower latitude then longitude. Cluster ids
/// count up by each cluster's earliest member, by input position, so a reordered
/// input yields the same clusters, renumbered only as their earliest members move.
///
/// Each query costs geodesic solutions, so this runs in `O(n log n)` when `eps` is
/// small against the spacing, and tends to `O(n²)` as most points fall within `eps`
/// of most others. [`bucket_by_geohash`](super::bucket_by_geohash) is the cheap
/// alternative for very large sets.
///
/// # Errors
///
/// [`Error::InvalidValue`] with `field: "eps"` when `eps` is negative or non-finite,
/// `field: "min_points"` when `min_points` is zero, and `field: "points"` when a
/// cluster's unit vectors cancel, which takes an `eps` around half the planet;
/// [`Error::CrsMismatch`] when the points' CRSs differ or are GCJ-02 / BD-09.
///
/// ```
/// use geocoordinates::{Coordinate, Error, Length, cluster::{PointLabel, dbscan}};
///
/// let points = [
///     Coordinate::wgs84(-17.0, 179.9999)?,
///     Coordinate::wgs84(10.0, 10.0)?,
///     Coordinate::wgs84(-17.0, -179.9999)?,
/// ];
/// let found = dbscan(&points, Length::from_meters(100.0), 2)?;
/// assert_eq!(found.labels()[1], PointLabel::Noise);
/// assert_eq!(found.clusters()[0].members(), [0, 2]); // across the antimeridian
/// assert!(matches!(
///     dbscan(&points, Length::from_meters(-1.0), 2),
///     Err(Error::InvalidValue { field: "eps", .. })
/// ));
/// assert!(matches!(
///     dbscan(&points, Length::from_meters(1.0), 0),
///     Err(Error::InvalidValue { field: "min_points", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn dbscan(points: &[Coordinate], eps: Length, min_points: usize) -> Result<Dbscan> {
    if !eps.meters().is_finite() || eps.meters() < 0.0 {
        return Err(Error::InvalidValue {
            field: "eps",
            detail: "must be finite and nonnegative".into(),
        });
    }
    if min_points == 0 {
        return Err(Error::InvalidValue {
            field: "min_points",
            detail: "must be at least 1".into(),
        });
    }
    let index = SpatialIndex::new(points.iter().map(|point| (*point, ())))?;

    let mut is_core = Vec::with_capacity(points.len());
    for point in points {
        is_core.push(index.within(*point, eps)?.len() >= min_points);
    }

    // Each core point's root, and for a border point the core point it joins.
    let mut parents: Vec<usize> = (0..points.len()).collect();
    let mut joins: Vec<Option<usize>> = vec![None; points.len()];
    for (position, point) in points.iter().enumerate() {
        let hits = index.within(*point, eps)?;
        let core_hits = hits.iter().filter(|hit| is_core[hit.index()]);
        if is_core[position] {
            for hit in core_hits {
                let (a, b) = (
                    find(&mut parents, position),
                    find(&mut parents, hit.index()),
                );
                parents[a.max(b)] = a.min(b);
            }
        } else {
            joins[position] = core_hits
                .min_by(|a, b| nearer_core(a, b))
                .map(Neighbor::index);
        }
    }

    // Walking positions upward numbers each cluster by its earliest member.
    let mut id_of_root: Vec<Option<usize>> = vec![None; points.len()];
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut labels = Vec::with_capacity(points.len());
    for position in 0..points.len() {
        let core = if is_core[position] {
            Some(position)
        } else {
            joins[position]
        };
        let Some(core) = core else {
            labels.push(PointLabel::Noise);
            continue;
        };
        let root = find(&mut parents, core);
        let id = *id_of_root[root].get_or_insert_with(|| {
            members.push(Vec::new());
            members.len() - 1
        });
        members[id].push(position);
        labels.push(PointLabel::Cluster { id });
    }

    let clusters = members
        .into_iter()
        .enumerate()
        .map(|(id, members)| summarize(id, points, members))
        .collect::<Result<_>>()?;
    Ok(Dbscan { labels, clusters })
}

/// The [`Cluster`] over `members`, positions in `points`, ascending.
pub(super) fn summarize(id: usize, points: &[Coordinate], members: Vec<usize>) -> Result<Cluster> {
    let member_points: Vec<Coordinate> = members.iter().map(|member| points[*member]).collect();
    Cluster::new(
        id,
        centroid(&member_points)?,
        GeoBounds::from_points(member_points)?,
        members,
    )
}
