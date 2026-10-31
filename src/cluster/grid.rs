//! [`bucket_by_geohash`] and [`bucket_by_h3`], re-exported flat from
//! [`cluster`](super).

use std::collections::HashMap;

use super::dbscan::summarize;
use super::types::Cluster;
use crate::coord::Coordinate;
use crate::error::Result;
use crate::grids::Geohash;

/// One occupied grid cell and the points in it.
///
/// ```
/// use geocoordinates::{Coordinate, cluster::bucket_by_geohash};
///
/// let point = Coordinate::wgs84(57.64911, 10.40744)?;
/// let buckets = bucket_by_geohash(&[point], 5)?;
/// assert_eq!(buckets[0].key(), "u4pru");
/// assert_eq!(buckets[0].cluster().count(), 1);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GridBucket {
    key: String,
    cluster: Cluster,
}

impl GridBucket {
    /// The cell: a geohash, or an H3 index in lowercase hexadecimal.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, cluster::bucket_by_geohash};
    ///
    /// let point = Coordinate::wgs84(57.64911, 10.40744)?;
    /// assert_eq!(bucket_by_geohash(&[point], 3)?[0].key(), "u4p");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The cell's points as a [`Cluster`], whose id orders the buckets by earliest
    /// member.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, cluster::bucket_by_geohash};
    ///
    /// let point = Coordinate::wgs84(57.64911, 10.40744)?;
    /// let buckets = bucket_by_geohash(&[point, point], 3)?;
    /// assert_eq!(buckets[0].cluster().members(), [0, 1]);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn cluster(&self) -> &Cluster {
        &self.cluster
    }
}

/// Groups `points` by cell key, ordering the buckets by earliest member.
fn buckets<F>(points: &[Coordinate], key_of: F) -> Result<Vec<GridBucket>>
where
    F: Fn(Coordinate) -> Result<String>,
{
    let mut slots: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (position, point) in points.iter().enumerate() {
        let key = key_of(*point)?;
        let slot = *slots.entry(key.clone()).or_insert_with(|| {
            groups.push((key, Vec::new()));
            groups.len() - 1
        });
        groups[slot].1.push(position);
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(id, (key, members))| {
            Ok(GridBucket {
                key,
                cluster: summarize(id, points, members)?,
            })
        })
        .collect()
}

/// Buckets WGS-84 points by geohash cell: the cheap grouping for sets too large for
/// [`dbscan`](super::dbscan) or a [`ClusterIndex`](super::ClusterIndex).
///
/// It takes one encode per point, needs no neighbor search, and is order-independent
/// in its groups: buckets are numbered by earliest member. The price is that a cell
/// is not a density estimate. Neighbors on either side of a cell edge land in
/// different buckets, and the antimeridian and the poles are always cell edges, so a
/// tight group straddling ±180° makes two buckets. Each bucket's centroid and bounds
/// are still correct for its own points.
///
/// # Errors
///
/// [`Error::InvalidValue`](crate::Error::InvalidValue) for a `length` outside the
/// geohash range, checked even with no points;
/// [`Error::CrsMismatch`](crate::Error::CrsMismatch) for a non-WGS-84 point.
///
/// ```
/// use geocoordinates::{Coordinate, Error, cluster::bucket_by_geohash};
///
/// let points = [
///     Coordinate::wgs84(48.8566, 2.3522)?,
///     Coordinate::wgs84(35.6762, 139.6503)?,
///     Coordinate::wgs84(48.8567, 2.3523)?,
/// ];
/// let buckets = bucket_by_geohash(&points, 4)?;
/// assert_eq!(buckets[0].cluster().members(), [0, 2]);
/// assert_eq!(buckets[1].cluster().members(), [1]);
/// assert!(matches!(bucket_by_geohash(&points, 0), Err(Error::InvalidValue { .. })));
/// assert!(matches!(
///     bucket_by_geohash(&[Coordinate::gcj02(1.0, 1.0)?], 4),
///     Err(Error::CrsMismatch { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn bucket_by_geohash(points: &[Coordinate], length: u8) -> Result<Vec<GridBucket>> {
    Geohash::encode(Coordinate::wgs84(0.0, 0.0)?, length)?;
    buckets(points, |point| {
        Ok(Geohash::encode(point, length)?.as_str().to_owned())
    })
}

/// Buckets WGS-84 points by H3 cell (needs the `h3` feature as well), with the same
/// trade-offs as [`bucket_by_geohash`], except that H3 cells do not take the
/// antimeridian or the poles as edges.
///
/// # Errors
///
/// [`Error::InvalidValue`](crate::Error::InvalidValue) for a `resolution` above 15,
/// checked even with no points;
/// [`Error::CrsMismatch`](crate::Error::CrsMismatch) for a non-WGS-84 point.
///
/// ```
/// use geocoordinates::{Coordinate, Error, cluster::bucket_by_h3};
///
/// let nyc = Coordinate::wgs84(40.7128, -74.006)?;
/// assert_eq!(bucket_by_h3(&[nyc], 9)?[0].key(), "892a1072893ffff");
/// assert!(matches!(bucket_by_h3(&[nyc], 16), Err(Error::InvalidValue { .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "h3")]
#[cfg_attr(docsrs, doc(cfg(feature = "h3")))]
pub fn bucket_by_h3(points: &[Coordinate], resolution: u8) -> Result<Vec<GridBucket>> {
    use crate::dgg::H3Cell;

    H3Cell::encode(Coordinate::wgs84(0.0, 0.0)?, resolution)?;
    buckets(points, |point| {
        Ok(format!("{:x}", H3Cell::encode(point, resolution)?.as_u64()))
    })
}
