//! [`SpatialIndex`], re-exported flat from [`index`](super).
//!
//! ## Why the prefilter never drops an answer
//!
//! Each point maps to the unit vector of its *reduced* latitude `β`
//! (`tan β = (1 - f) tan φ`) and longitude `λ`. In those coordinates the ellipsoid's
//! line element is `ds² = a²(1 - e² cos²β) dβ² + a² cos²β dλ²`, and the unit sphere's
//! is `dσ² = dβ² + cos²β dλ²`. Since `1 - e² cos²β ≥ 1 - e² = (b/a)²`, every curve
//! on the ellipsoid is at least `b` times as long as its image on the sphere. So a
//! geodesic of length `s` joins points at most `s / b` radians apart, and points
//! further apart than that are safely skipped. Chord lengths compare against that
//! angle with a small slack that covers round-off.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use geographiclib_rs::{Geodesic, InverseGeodesic};

use super::kdtree::KdTree;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::geodesy::Ellipsoid;
use crate::units::Length;

/// Relative slack on a squared chord limit.
const CHORD_SLACK_REL: f64 = 1e-12;
/// Absolute slack on a squared chord limit. It covers round-off in the unit
/// vectors and admits at most a few extra candidates, which refinement rejects.
const CHORD_SLACK_ABS: f64 = 1e-14;

/// A static spatial index over `(Coordinate, T)` entries, answering geodesic
/// radius and k-nearest queries with exact results.
///
/// The index is built once and is immutable. It prefilters candidates by chord on
/// the unit sphere with an in-crate k-d tree, then refines each candidate with the
/// Karney geodesic on the ellipsoid of the entries' CRS, so every result and
/// distance matches a brute-force geodesic search, across the antimeridian and at
/// the poles. Results are ordered by distance, then by insertion order. Heights
/// are ignored: distances are on the ellipsoid surface.
///
/// Building takes `O(n log n)` time. A query's cost grows with the candidates near
/// its answer, and each candidate costs one geodesic solution.
///
/// ```
/// use geocoordinates::{Coordinate, Error, index::SpatialIndex};
///
/// let index = SpatialIndex::new([
///     (Coordinate::wgs84(0.0, 179.9)?, "west of the antimeridian"),
///     (Coordinate::wgs84(0.0, -179.9)?, "east of the antimeridian"),
///     (Coordinate::wgs84(0.0, 0.0)?, "far away"),
/// ])?;
/// let nearest = index.nearest(Coordinate::wgs84(0.0, -179.95)?, 2)?;
/// assert_eq!(*nearest[0].value(), "east of the antimeridian");
/// assert_eq!(*nearest[1].value(), "west of the antimeridian");
/// // Every entry and query shares one datum.
/// assert!(matches!(
///     index.nearest(Coordinate::new(0.0, 0.0, geocoordinates::Crs::Tokyo)?, 1),
///     Err(Error::CrsMismatch { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct SpatialIndex<T> {
    entries: Vec<(Coordinate, T)>,
    /// The entries' model; `None` for an index with no entries.
    model: Option<Model>,
    tree: KdTree,
}

/// The ellipsoid-dependent parts of a query.
#[derive(Debug, Clone)]
struct Model {
    crs: Crs,
    geodesic: Geodesic,
    semi_minor_m: f64,
    flattening: f64,
}

impl Model {
    fn for_crs(crs: Crs) -> Result<Self> {
        let ellipsoid = Ellipsoid::for_crs(crs).ok_or(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: crs,
        })?;
        Ok(Self {
            crs,
            geodesic: Geodesic::new(ellipsoid.semi_major().meters(), ellipsoid.flattening()),
            semi_minor_m: ellipsoid.semi_minor().meters(),
            flattening: ellipsoid.flattening(),
        })
    }

    /// The unit vector of `point`'s reduced latitude and longitude.
    fn unit_vector(&self, point: Coordinate) -> [f64; 3] {
        let (sin_lat, cos_lat) = point.lat().to_radians().sin_cos();
        let (sin_lon, cos_lon) = point.lon().to_radians().sin_cos();
        let z = (1.0 - self.flattening) * sin_lat;
        let scale = z.hypot(cos_lat);
        [
            cos_lat / scale * cos_lon,
            cos_lat / scale * sin_lon,
            z / scale,
        ]
    }

    /// The squared chord beyond which two points are further apart than `meters`.
    fn chord_sq_limit(&self, meters: f64) -> f64 {
        let half_angle = meters / self.semi_minor_m / 2.0;
        if half_angle >= std::f64::consts::FRAC_PI_2 {
            return f64::INFINITY;
        }
        (2.0 * half_angle.sin()).powi(2) * (1.0 + CHORD_SLACK_REL) + CHORD_SLACK_ABS
    }

    fn distance_m(&self, a: Coordinate, b: Coordinate) -> f64 {
        self.geodesic.inverse(a.lat(), a.lon(), b.lat(), b.lon())
    }
}

/// One query result: an entry of a [`SpatialIndex`] with its distance from the
/// query point.
///
/// ```
/// use geocoordinates::{Coordinate, index::SpatialIndex};
///
/// let point = Coordinate::wgs84(1.0, 2.0)?;
/// let index = SpatialIndex::new([(point, "a")])?;
/// let hit = index.nearest(point, 1)?[0];
/// assert_eq!((hit.coordinate(), *hit.value(), hit.index()), (point, "a", 0));
/// assert_eq!(hit.distance().meters(), 0.0);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, PartialEq)]
pub struct Neighbor<'a, T> {
    coordinate: Coordinate,
    value: &'a T,
    index: usize,
    distance: Length,
}

impl<T> Clone for Neighbor<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Neighbor<'_, T> {}

impl<'a, T> Neighbor<'a, T> {
    /// The entry's coordinate, as it was inserted.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, index::SpatialIndex};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let index = SpatialIndex::new([(point, ())])?;
    /// assert_eq!(index.nearest(point, 1)?[0].coordinate(), point);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn coordinate(&self) -> Coordinate {
        self.coordinate
    }

    /// The entry's value.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, index::SpatialIndex};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let index = SpatialIndex::new([(point, 7)])?;
    /// assert_eq!(*index.nearest(point, 1)?[0].value(), 7);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn value(&self) -> &'a T {
        self.value
    }

    /// The entry's position in the build order, which breaks distance ties.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, index::SpatialIndex};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let index = SpatialIndex::new([(point, "first"), (point, "second")])?;
    /// let hits = index.nearest(point, 2)?;
    /// assert_eq!((hits[0].index(), hits[1].index()), (0, 1));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The geodesic distance from the query point.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, geodesy::geodesic_distance, index::SpatialIndex};
    ///
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 1.0)?);
    /// let index = SpatialIndex::new([(b, ())])?;
    /// assert_eq!(index.nearest(a, 1)?[0].distance(), geodesic_distance(a, b)?);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn distance(&self) -> Length {
        self.distance
    }
}

/// A refined candidate in the k-nearest heap, ordered by distance then position.
#[derive(Debug, Clone, Copy)]
struct Refined {
    meters: f64,
    index: usize,
}

impl Refined {
    fn order(&self, other: &Self) -> Ordering {
        self.meters
            .total_cmp(&other.meters)
            .then(self.index.cmp(&other.index))
    }
}

impl PartialEq for Refined {
    fn eq(&self, other: &Self) -> bool {
        self.order(other) == Ordering::Equal
    }
}

impl Eq for Refined {}

impl PartialOrd for Refined {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Refined {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order(other)
    }
}

impl<T> SpatialIndex<T> {
    /// Builds the index from `(coordinate, value)` entries, all on one CRS. An
    /// empty set is a valid index whose queries return nothing.
    ///
    /// # Errors
    ///
    /// [`Error::CrsMismatch`] when the entries' CRSs differ or are GCJ-02 / BD-09
    /// (obfuscated renderings with no ellipsoid; convert them to WGS-84 first).
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, index::SpatialIndex};
    ///
    /// let index = SpatialIndex::new([(Coordinate::wgs84(1.0, 2.0)?, "a")])?;
    /// assert_eq!(index.len(), 1);
    /// assert!(matches!(
    ///     SpatialIndex::new([(Coordinate::gcj02(1.0, 2.0)?, "a")]),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(entries: impl IntoIterator<Item = (Coordinate, T)>) -> Result<Self> {
        let entries: Vec<(Coordinate, T)> = entries.into_iter().collect();
        let model = entries
            .first()
            .map(|(point, _)| Model::for_crs(point.crs()))
            .transpose()?;
        let Some(model) = model else {
            return Ok(Self {
                entries,
                model: None,
                tree: KdTree::build([]),
            });
        };
        let mut vectors = Vec::with_capacity(entries.len());
        for (point, _) in &entries {
            if point.crs() != model.crs {
                return Err(Error::CrsMismatch {
                    expected: model.crs,
                    found: point.crs(),
                });
            }
            vectors.push(model.unit_vector(*point));
        }
        Ok(Self {
            entries,
            model: Some(model),
            tree: KdTree::build(vectors),
        })
    }

    /// The number of entries.
    ///
    /// ```
    /// use geocoordinates::index::SpatialIndex;
    ///
    /// assert_eq!(SpatialIndex::<()>::new([])?.len(), 0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index has no entries.
    ///
    /// ```
    /// use geocoordinates::index::SpatialIndex;
    ///
    /// assert!(SpatialIndex::<()>::new([])?.is_empty());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The CRS of the entries, or `None` for an index with no entries.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, index::SpatialIndex};
    ///
    /// let index = SpatialIndex::new([(Coordinate::wgs84(1.0, 2.0)?, ())])?;
    /// assert_eq!(index.crs(), Some(Crs::Wgs84));
    /// assert_eq!(SpatialIndex::<()>::new([])?.crs(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn crs(&self) -> Option<Crs> {
        self.model.as_ref().map(|model| model.crs)
    }

    /// Every entry within geodesic `radius` of `center`, the radius included,
    /// ordered by distance and then by insertion order.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "radius"`) when `radius` is negative or
    /// non-finite; [`Error::CrsMismatch`] when `center` is on GCJ-02 / BD-09 or on
    /// a CRS other than the entries'.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, Length, LengthUnit, index::SpatialIndex};
    ///
    /// let center = Coordinate::wgs84(89.9, 0.0)?;
    /// // Across the pole: the two points differ by 180° of longitude.
    /// let index = SpatialIndex::new([(Coordinate::wgs84(89.9, 180.0)?, "over the pole")])?;
    /// let radius = Length::from_unit(30.0, LengthUnit::Kilometer);
    /// assert_eq!(index.within(center, radius)?.len(), 1);
    /// assert!(matches!(
    ///     index.within(center, Length::from_meters(-1.0)),
    ///     Err(Error::InvalidValue { field: "radius", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn within(&self, center: Coordinate, radius: Length) -> Result<Vec<Neighbor<'_, T>>> {
        let meters = radius.meters();
        if !meters.is_finite() || meters < 0.0 {
            return Err(Error::InvalidValue {
                field: "radius",
                detail: "must be finite and nonnegative".into(),
            });
        }
        let Some(model) = self.model_for(center)? else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        self.tree.for_each_within(
            model.unit_vector(center),
            model.chord_sq_limit(meters),
            |index| {
                let distance = model.distance_m(center, self.entries[index].0);
                if distance <= meters {
                    found.push(Refined {
                        meters: distance,
                        index,
                    });
                }
            },
        );
        found.sort_unstable();
        Ok(self.neighbors(found))
    }

    /// The `k` entries nearest `center` by geodesic distance, ordered by distance
    /// and then by insertion order, or every entry when there are fewer than `k`.
    /// Of entries at the same distance, the earlier-inserted one is kept.
    ///
    /// # Errors
    ///
    /// [`Error::CrsMismatch`] when `center` is on GCJ-02 / BD-09 or on a CRS other
    /// than the entries'.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, index::SpatialIndex};
    ///
    /// let index = SpatialIndex::new([
    ///     (Coordinate::wgs84(0.0, 2.0)?, "far"),
    ///     (Coordinate::wgs84(0.0, 1.0)?, "near"),
    /// ])?;
    /// let center = Coordinate::wgs84(0.0, 0.0)?;
    /// let hits = index.nearest(center, 5)?;
    /// assert_eq!((*hits[0].value(), *hits[1].value()), ("near", "far"));
    /// assert!(matches!(
    ///     index.nearest(Coordinate::gcj02(0.0, 0.0)?, 1),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn nearest(&self, center: Coordinate, k: usize) -> Result<Vec<Neighbor<'_, T>>> {
        let Some(model) = self.model_for(center)? else {
            return Ok(Vec::new());
        };
        if k == 0 {
            return Ok(Vec::new());
        }
        let mut best: BinaryHeap<Refined> = BinaryHeap::with_capacity(k.min(self.len()));
        let mut limit_sq = f64::INFINITY;
        for (chord_sq, index) in self.tree.by_chord(model.unit_vector(center)) {
            // Candidates arrive by rising chord, so once one cannot be as near as
            // the current k-th, none after it can. A candidate *at* that distance
            // is still refined: it may win the tie on insertion order.
            if chord_sq > limit_sq {
                break;
            }
            let candidate = Refined {
                meters: model.distance_m(center, self.entries[index].0),
                index,
            };
            if best.len() < k {
                best.push(candidate);
            } else if best.peek().is_some_and(|worst| candidate < *worst) {
                best.pop();
                best.push(candidate);
            }
            if best.len() == k {
                if let Some(worst) = best.peek() {
                    limit_sq = model.chord_sq_limit(worst.meters);
                }
            }
        }
        Ok(self.neighbors(best.into_sorted_vec()))
    }

    /// The model to query with, after checking `center` against it: `None` for an
    /// index with no entries, where `center` need only have an ellipsoid at all.
    fn model_for(&self, center: Coordinate) -> Result<Option<&Model>> {
        let found = center.crs();
        if Ellipsoid::for_crs(found).is_none() {
            return Err(Error::CrsMismatch {
                expected: self.crs().unwrap_or(Crs::Wgs84),
                found,
            });
        }
        match &self.model {
            Some(model) if model.crs != found => Err(Error::CrsMismatch {
                expected: model.crs,
                found,
            }),
            model => Ok(model.as_ref()),
        }
    }

    fn neighbors(&self, refined: Vec<Refined>) -> Vec<Neighbor<'_, T>> {
        refined
            .into_iter()
            .map(|Refined { meters, index }| {
                let (coordinate, value) = &self.entries[index];
                Neighbor {
                    coordinate: *coordinate,
                    value,
                    index,
                    distance: Length::from_meters(meters),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
