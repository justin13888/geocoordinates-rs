//! [`segment_visits`] and [`segment_trips`], re-exported flat from
//! [`cluster`](super).

use std::time::{Duration, SystemTime};

use super::centroid::normal_radius_m;
use super::dbscan::summarize;
use super::types::Cluster;
use crate::Length;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::fix::Fix;
use crate::geodesy::geodesic_distance;
use crate::index::GeoBounds;

/// A stay point: a run of fixes that stayed near one place for long enough.
///
/// ```
/// use std::time::{Duration, UNIX_EPOCH};
/// use geocoordinates::{Coordinate, Length, cluster::segment_visits, fix::Fix};
///
/// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
/// fix.timestamp = Some(UNIX_EPOCH);
/// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
/// assert_eq!(visits[0].cluster().members(), [0]);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "VisitRepr"))]
pub struct Visit {
    cluster: Cluster,
    arrival: SystemTime,
    departure: SystemTime,
}

/// The unchecked wire shape of a [`Visit`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct VisitRepr {
    cluster: Cluster,
    arrival: SystemTime,
    departure: SystemTime,
}

#[cfg(feature = "serde")]
impl TryFrom<VisitRepr> for Visit {
    type Error = Error;

    fn try_from(repr: VisitRepr) -> Result<Self> {
        Self::new(repr.cluster, repr.arrival, repr.departure)
    }
}

impl Visit {
    /// A visit from its parts.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (`field: "departure"`) when `departure` precedes
    /// `arrival`.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Error, Length};
    /// use geocoordinates::cluster::{Visit, dbscan};
    ///
    /// let point = Coordinate::wgs84(1.0, 2.0)?;
    /// let cluster = dbscan(&[point], Length::from_meters(1.0), 1)?.clusters()[0].clone();
    /// let later = UNIX_EPOCH + Duration::from_secs(60);
    /// assert!(Visit::new(cluster.clone(), UNIX_EPOCH, later).is_ok());
    /// assert!(matches!(
    ///     Visit::new(cluster, later, UNIX_EPOCH),
    ///     Err(Error::InvalidValue { field: "departure", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(cluster: Cluster, arrival: SystemTime, departure: SystemTime) -> Result<Self> {
        if departure < arrival {
            return Err(Error::InvalidValue {
                field: "departure",
                detail: "must not precede the arrival".into(),
            });
        }
        Ok(Self {
            cluster,
            arrival,
            departure,
        })
    }

    /// The visit's fixes as a [`Cluster`], whose members are positions in the fixes
    /// given to [`segment_visits`] and whose id is the visit's place in time.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Length, cluster::segment_visits, fix::Fix};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
    /// assert_eq!(visits[0].cluster().id(), 0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn cluster(&self) -> &Cluster {
        &self.cluster
    }

    /// The time of the visit's first fix.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Length, cluster::segment_visits, fix::Fix};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
    /// assert_eq!(visits[0].arrival(), UNIX_EPOCH);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn arrival(&self) -> SystemTime {
        self.arrival
    }

    /// The time of the visit's last fix.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Length, cluster::segment_visits, fix::Fix};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
    /// assert_eq!(visits[0].departure(), UNIX_EPOCH);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn departure(&self) -> SystemTime {
        self.departure
    }
}

/// A run of [`Visit`]s no further apart in time than a gap.
///
/// ```
/// use std::time::{Duration, UNIX_EPOCH};
/// use geocoordinates::{Coordinate, Length, cluster::{segment_trips, segment_visits}, fix::Fix};
///
/// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
/// fix.timestamp = Some(UNIX_EPOCH);
/// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
/// assert_eq!(segment_trips(&visits, Duration::ZERO)?[0].visits(), [0]);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "TripRepr"))]
pub struct Trip {
    visits: Vec<usize>,
    start: SystemTime,
    end: SystemTime,
    bounds: GeoBounds,
}

/// The unchecked wire shape of a [`Trip`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct TripRepr {
    visits: Vec<usize>,
    start: SystemTime,
    end: SystemTime,
    bounds: GeoBounds,
}

#[cfg(feature = "serde")]
impl TryFrom<TripRepr> for Trip {
    type Error = Error;

    fn try_from(repr: TripRepr) -> Result<Self> {
        if repr.visits.is_empty() {
            return Err(Error::InvalidValue {
                field: "visits",
                detail: "a trip holds at least one visit".into(),
            });
        }
        if repr.end < repr.start {
            return Err(Error::InvalidValue {
                field: "end",
                detail: "must not precede the start".into(),
            });
        }
        Ok(Self {
            visits: repr.visits,
            start: repr.start,
            end: repr.end,
            bounds: repr.bounds,
        })
    }
}

impl Trip {
    /// The trip's visits as positions in the list given to [`segment_trips`], in
    /// order of arrival.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Length, cluster::{segment_trips, segment_visits}, fix::Fix};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
    /// assert_eq!(segment_trips(&visits, Duration::ZERO)?[0].visits(), [0]);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn visits(&self) -> &[usize] {
        &self.visits
    }

    /// The earliest arrival of the trip's visits.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Length, cluster::{segment_trips, segment_visits}, fix::Fix};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
    /// assert_eq!(segment_trips(&visits, Duration::ZERO)?[0].start(), UNIX_EPOCH);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn start(&self) -> SystemTime {
        self.start
    }

    /// The latest departure of the trip's visits.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Length, cluster::{segment_trips, segment_visits}, fix::Fix};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
    /// assert_eq!(segment_trips(&visits, Duration::ZERO)?[0].end(), UNIX_EPOCH);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn end(&self) -> SystemTime {
        self.end
    }

    /// A box holding the bounds of every visit in the trip.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use geocoordinates::{Coordinate, Length, cluster::{segment_trips, segment_visits}, fix::Fix};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// let visits = segment_visits(&[fix], Length::from_meters(10.0), Duration::ZERO)?;
    /// assert_eq!(segment_trips(&visits, Duration::ZERO)?[0].bounds().north(), 1.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn bounds(&self) -> GeoBounds {
        self.bounds
    }
}

/// Stay points from timestamped fixes: each visit is a maximal run of fixes, in time
/// order, within geodesic `distance` of the run's first fix and lasting at least
/// `dwell`. Fixes in no visit are movement between visits.
///
/// Fixes need not arrive sorted: they are taken by timestamp, then latitude, then
/// longitude, then input position, so the visits do not depend on input order. A
/// run that is too short gives up only its first fix, and the search restarts at
/// the next one. Visit ids count up in time. Heights and accuracies are ignored.
///
/// # Errors
///
/// [`Error::InvalidValue`] with `field: "distance"` when `distance` is negative or
/// non-finite, and `field: "timestamp"` for a fix with no timestamp, naming its
/// position; [`Error::CrsMismatch`] when the fixes' CRSs differ or are GCJ-02 /
/// BD-09.
///
/// ```
/// use std::time::{Duration, UNIX_EPOCH};
/// use geocoordinates::{Coordinate, Error, Length, cluster::segment_visits, fix::Fix};
///
/// let fix_at = |lat: f64, minute: u64| -> geocoordinates::Result<Fix> {
///     let mut fix = Fix::from_coord(Coordinate::wgs84(lat, 0.0)?);
///     fix.timestamp = Some(UNIX_EPOCH + Duration::from_secs(minute * 60));
///     Ok(fix)
/// };
/// // Twenty minutes at one place, then a move.
/// let fixes = [fix_at(0.0, 0)?, fix_at(0.0, 20)?, fix_at(5.0, 30)?];
/// let (near, dwell) = (Length::from_meters(50.0), Duration::from_secs(600));
/// let visits = segment_visits(&fixes, near, dwell)?;
/// assert_eq!(visits.len(), 1);
/// assert_eq!(visits[0].cluster().members(), [0, 1]);
///
/// let no_time = Fix::from_coord(Coordinate::wgs84(0.0, 0.0)?);
/// assert!(matches!(
///     segment_visits(&[no_time], near, dwell),
///     Err(Error::InvalidValue { field: "timestamp", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn segment_visits(fixes: &[Fix], distance: Length, dwell: Duration) -> Result<Vec<Visit>> {
    if !distance.meters().is_finite() || distance.meters() < 0.0 {
        return Err(Error::InvalidValue {
            field: "distance",
            detail: "must be finite and nonnegative".into(),
        });
    }
    let mut ordered = Vec::with_capacity(fixes.len());
    for (position, fix) in fixes.iter().enumerate() {
        let time = fix.timestamp.ok_or_else(|| Error::InvalidValue {
            field: "timestamp",
            detail: format!("fix {position} has no timestamp"),
        })?;
        ordered.push((time, fix.coord, position));
    }
    ordered.sort_unstable_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.lat().total_cmp(&b.1.lat()))
            .then(a.1.lon().total_cmp(&b.1.lon()))
            .then(a.2.cmp(&b.2))
    });
    if let Some(first) = ordered.first() {
        normal_radius_m(first.1.crs())?;
        let crs = first.1.crs();
        if let Some(other) = ordered.iter().find(|fix| fix.1.crs() != crs) {
            return Err(Error::CrsMismatch {
                expected: crs,
                found: other.1.crs(),
            });
        }
    }

    let points: Vec<Coordinate> = fixes.iter().map(|fix| fix.coord).collect();
    let mut visits = Vec::new();
    let mut start = 0;
    while start < ordered.len() {
        let (arrival, anchor, _) = ordered[start];
        let mut end = start + 1;
        while end < ordered.len() && geodesic_distance(anchor, ordered[end].1)? <= distance {
            end += 1;
        }
        let departure = ordered[end - 1].0;
        // Sorted times, so the subtraction cannot fail.
        if departure.duration_since(arrival).unwrap_or_default() < dwell {
            start += 1;
            continue;
        }
        let mut members: Vec<usize> = ordered[start..end].iter().map(|fix| fix.2).collect();
        members.sort_unstable();
        let cluster = summarize(visits.len(), &points, members)?;
        visits.push(Visit::new(cluster, arrival, departure)?);
        start = end;
    }
    Ok(visits)
}

/// Groups visits into trips: taken in order of arrival, a visit that begins more than
/// `gap` after every earlier visit has ended starts a new trip. Visits that overlap
/// in time share a trip.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when the visits' CRSs differ.
///
/// ```
/// use std::time::{Duration, UNIX_EPOCH};
/// use geocoordinates::{Coordinate, Error, Length};
/// use geocoordinates::cluster::{Visit, dbscan, segment_trips};
///
/// let hours = |hour: u64| UNIX_EPOCH + Duration::from_secs(hour * 3600);
/// let visit_at = |point: Coordinate, hour: u64| -> geocoordinates::Result<Visit> {
///     let cluster = dbscan(&[point], Length::from_meters(1.0), 1)?.clusters()[0].clone();
///     Visit::new(cluster, hours(hour), hours(hour))
/// };
/// let here = Coordinate::wgs84(1.0, 2.0)?;
/// let visits = [visit_at(here, 0)?, visit_at(here, 1)?, visit_at(here, 72)?];
/// let trips = segment_trips(&visits, Duration::from_secs(6 * 3600))?;
/// assert_eq!(trips.len(), 2);
/// assert_eq!(trips[0].visits(), [0, 1]);
/// assert_eq!(trips[1].start(), hours(72));
///
/// let tokyo = visit_at(Coordinate::new(1.0, 2.0, geocoordinates::Crs::Tokyo)?, 2)?;
/// assert!(matches!(
///     segment_trips(&[visit_at(here, 0)?, tokyo], Duration::ZERO),
///     Err(Error::CrsMismatch { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn segment_trips(visits: &[Visit], gap: Duration) -> Result<Vec<Trip>> {
    let mut order: Vec<usize> = (0..visits.len()).collect();
    order.sort_by_key(|position| (visits[*position].arrival, *position));

    let mut trips: Vec<Trip> = Vec::new();
    for position in order {
        let visit = &visits[position];
        let bounds = visit.cluster.bounds();
        if let Some(trip) = trips.last_mut() {
            // A visit overlapping the trip gives an error here, which means no gap.
            let waited = visit.arrival.duration_since(trip.end).unwrap_or_default();
            if waited <= gap {
                trip.visits.push(position);
                trip.end = trip.end.max(visit.departure);
                trip.bounds = trip.bounds.union(&bounds)?;
                continue;
            }
            trip.bounds.union(&bounds)?;
        }
        trips.push(Trip {
            visits: vec![position],
            start: visit.arrival,
            end: visit.departure,
            bounds,
        });
    }
    Ok(trips)
}
