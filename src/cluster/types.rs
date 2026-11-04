//! [`Cluster`], re-exported flat from [`cluster`](super).

use crate::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::index::GeoBounds;

/// A group of input points with its summary: id, centroid, bounds and members.
///
/// Members are positions in the slice the cluster was computed from, strictly
/// ascending, so a caller looks up its own payload by position.
///
/// ```
/// use geocoordinates::{Coordinate, Length, cluster::dbscan};
///
/// let photos = [Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 0.0001)?];
/// let found = dbscan(&photos, Length::from_meters(50.0), 2)?;
/// assert_eq!(found.clusters()[0].members(), [0, 1]);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "ClusterRepr"))]
pub struct Cluster {
    id: usize,
    centroid: Approx<Coordinate>,
    bounds: GeoBounds,
    members: Vec<usize>,
}

/// The unchecked wire shape of a [`Cluster`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct ClusterRepr {
    id: usize,
    centroid: Approx<Coordinate>,
    bounds: GeoBounds,
    members: Vec<usize>,
}

#[cfg(feature = "serde")]
impl TryFrom<ClusterRepr> for Cluster {
    type Error = Error;

    fn try_from(repr: ClusterRepr) -> Result<Self> {
        Self::new(repr.id, repr.centroid, repr.bounds, repr.members)
    }
}

impl Cluster {
    /// A cluster from its parts.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "members"`) when `members` is empty or not
    /// strictly ascending, and [`Error::CrsMismatch`] when the centroid and the
    /// bounds are on different CRSs.
    ///
    /// ```
    /// use geocoordinates::{Approx, Coordinate, Crs, Error, Length};
    /// use geocoordinates::{cluster::Cluster, index::GeoBounds};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let centroid = Approx::new(point, Length::from_meters(0.0))?;
    /// let bounds = GeoBounds::from_points([point])?;
    /// assert_eq!(Cluster::new(0, centroid, bounds, vec![3, 7])?.count(), 2);
    /// assert!(matches!(
    ///     Cluster::new(0, centroid, bounds, vec![7, 3]),
    ///     Err(Error::InvalidValue { field: "members", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(
        id: usize,
        centroid: Approx<Coordinate>,
        bounds: GeoBounds,
        members: Vec<usize>,
    ) -> Result<Self> {
        if members.is_empty() || members.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Error::InvalidValue {
                field: "members",
                detail: "must be nonempty and strictly ascending".into(),
            });
        }
        if centroid.value().crs() != bounds.crs() {
            return Err(Error::CrsMismatch {
                expected: bounds.crs(),
                found: centroid.value().crs(),
            });
        }
        Ok(Self {
            id,
            centroid,
            bounds,
            members,
        })
    }

    /// The cluster's id: its position among the clusters of one result, assigned
    /// by each cluster's earliest member.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Length, cluster::dbscan};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let found = dbscan(&[point], Length::from_meters(1.0), 1)?;
    /// assert_eq!(found.clusters()[0].id(), 0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    /// The mean of the members, with [`centroid`](super::centroid)'s bound.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Length, cluster::dbscan};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let found = dbscan(&[point], Length::from_meters(1.0), 1)?;
    /// let middle = found.clusters()[0].centroid();
    /// assert!((middle.value().lat() - 1.0).abs() < 1e-9 && middle.max_error().meters() < 1e-3);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn centroid(&self) -> Approx<Coordinate> {
        self.centroid
    }

    /// The smallest box holding every member.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Length, cluster::dbscan};
    ///
    /// let points = [Coordinate::wgs84(0.0, 179.9999)?, Coordinate::wgs84(0.0, -179.9999)?];
    /// let found = dbscan(&points, Length::from_meters(100.0), 2)?;
    /// assert!(found.clusters()[0].bounds().crosses_antimeridian());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn bounds(&self) -> GeoBounds {
        self.bounds
    }

    /// The number of members.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Length, cluster::dbscan};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let found = dbscan(&[point, point], Length::from_meters(1.0), 1)?;
    /// assert_eq!(found.clusters()[0].count(), 2);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn count(&self) -> usize {
        self.members.len()
    }

    /// The members' positions in the input, strictly ascending.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Length, cluster::dbscan};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let found = dbscan(&[point, point], Length::from_meters(1.0), 1)?;
    /// assert_eq!(found.clusters()[0].members(), [0, 1]);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn members(&self) -> &[usize] {
        &self.members
    }
}
