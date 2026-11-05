//! The time-ordered track and the lookup that places a photo on it.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{GeotagOptions, Interpolation};
use crate::fix::{Accuracy, AccuracyKind, Confidence, Fix, RawSource};
use crate::geodesy::{Ellipsoid, geodesic_distance, intermediate};
use crate::{Coordinate, Crs, Error, Height, Length, Result};

/// A GPS track: recorded [`Fix`]es in time order, each with a timestamp, all on
/// one geodetic datum.
///
/// Built once and immutable. [`locate`](Self::locate) answers where the track was
/// at a given time in `O(log n)`.
///
/// ```
/// use std::time::{Duration, UNIX_EPOCH};
///
/// use geocoordinates::Coordinate;
/// use geocoordinates::fix::Fix;
/// use geocoordinates::geotag::Track;
///
/// let mut timed = Fix::from_coord(Coordinate::wgs84(51.5, -0.12)?);
/// timed.timestamp = Some(UNIX_EPOCH + Duration::from_secs(60));
/// let untimed = Fix::from_coord(Coordinate::wgs84(51.6, -0.13)?);
/// let track = Track::new(vec![timed, untimed])?;
/// assert_eq!((track.len(), track.dropped_untimed()), (1, 1));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// The kept fixes, sorted by `times`.
    fixes: Vec<Fix>,
    /// Each kept fix's timestamp, strictly increasing.
    times: Vec<SystemTime>,
    dropped_untimed: usize,
}

/// How a located fix was derived from the track, with the indices it used.
#[derive(Debug, Clone, Copy)]
enum Derivation {
    /// The photo's time is a fix's own time.
    Matched(usize),
    /// Between fixes `a` and `b`, at `fraction` of the way through the gap.
    Interpolated { a: usize, b: usize, fraction: f64 },
    /// Between fixes `a` and `b`, at the position of `nearest`.
    Nearest { a: usize, b: usize, nearest: usize },
    /// Beyond the recorded fixes (or inside an over-long gap), held at this fix.
    Extrapolated(usize),
}

impl Track {
    /// Build a track from `fixes` in any order.
    ///
    /// Sorts the fixes by timestamp and drops those with none, counting them in
    /// [`dropped_untimed`](Self::dropped_untimed). Two fixes at the same instant
    /// at the same position (height and datum included) are one fix, recorded
    /// twice: the first in input order is kept. An empty track, or one whose fixes
    /// all lacked a time, is valid and locates nothing.
    ///
    /// # Errors
    /// [`Error::InvalidValue`] (field `fixes`) when two fixes at the same instant
    /// disagree on the position; [`Error::CrsMismatch`] when the timed fixes are
    /// on different datums, or on GCJ-02 / BD-09, which have no ellipsoid to
    /// interpolate on (convert them to WGS-84 first).
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    ///
    /// use geocoordinates::fix::Fix;
    /// use geocoordinates::geotag::Track;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let at = |lat, lon| -> Result<Fix, Error> {
    ///     let mut fix = Fix::from_coord(Coordinate::wgs84(lat, lon)?);
    ///     fix.timestamp = Some(UNIX_EPOCH + Duration::from_secs(10));
    ///     Ok(fix)
    /// };
    /// // The same fix recorded twice is kept once.
    /// assert_eq!(Track::new(vec![at(1.0, 2.0)?, at(1.0, 2.0)?])?.len(), 1);
    /// // Two places at one instant cannot both be right.
    /// assert!(matches!(
    ///     Track::new(vec![at(1.0, 2.0)?, at(1.0, 2.5)?]),
    ///     Err(Error::InvalidValue { field: "fixes", .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn new(fixes: Vec<Fix>) -> Result<Self> {
        let total = fixes.len();
        let mut timed: Vec<(SystemTime, Fix)> = fixes
            .into_iter()
            .filter_map(|fix| fix.timestamp.map(|time| (time, fix)))
            .collect();
        let dropped_untimed = total - timed.len();
        // Stable, so the first of two equal fixes in input order is the one kept.
        timed.sort_by_key(|(time, _)| *time);

        let mut crs: Option<Crs> = None;
        let mut kept: Vec<Fix> = Vec::with_capacity(timed.len());
        let mut times: Vec<SystemTime> = Vec::with_capacity(timed.len());
        for (time, fix) in timed {
            let found = fix.coord.crs();
            match crs {
                None if Ellipsoid::for_crs(found).is_none() => {
                    return Err(Error::CrsMismatch {
                        expected: Crs::Wgs84,
                        found,
                    });
                }
                None => crs = Some(found),
                Some(expected) if expected != found => {
                    return Err(Error::CrsMismatch { expected, found });
                }
                Some(_) => {}
            }
            if times.last() == Some(&time) {
                if kept.last().is_some_and(|last| last.coord != fix.coord) {
                    return Err(Error::InvalidValue {
                        field: "fixes",
                        detail: format!(
                            "two fixes at {:.3} s (Unix time) disagree on the position",
                            unix_secs(time)
                        ),
                    });
                }
                continue;
            }
            times.push(time);
            kept.push(fix);
        }
        Ok(Self {
            fixes: kept,
            times,
            dropped_untimed,
        })
    }

    /// The kept fixes, in time order, each with a timestamp.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    ///
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::fix::Fix;
    /// use geocoordinates::geotag::Track;
    ///
    /// let fix = |secs| -> Result<Fix, geocoordinates::Error> {
    ///     let mut fix = Fix::from_coord(Coordinate::wgs84(0.0, secs as f64 / 100.0)?);
    ///     fix.timestamp = Some(UNIX_EPOCH + Duration::from_secs(secs));
    ///     Ok(fix)
    /// };
    /// let track = Track::new(vec![fix(20)?, fix(10)?])?;
    /// assert_eq!(track.fixes()[0].coord.lon(), 0.1);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn fixes(&self) -> &[Fix] {
        &self.fixes
    }

    /// The number of kept fixes.
    ///
    /// ```
    /// use geocoordinates::geotag::Track;
    ///
    /// assert_eq!(Track::new(vec![])?.len(), 0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.fixes.len()
    }

    /// Whether the track kept no fix, so it locates nothing.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::fix::Fix;
    /// use geocoordinates::geotag::Track;
    ///
    /// // A fix with no time cannot be matched against a photo's time.
    /// let untimed = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// assert!(Track::new(vec![untimed])?.is_empty());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fixes.is_empty()
    }

    /// How many of the fixes given to [`new`](Self::new) had no timestamp and
    /// were dropped. Same-instant duplicates are not counted here.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::fix::Fix;
    /// use geocoordinates::geotag::Track;
    ///
    /// let untimed = Fix::from_coord(Coordinate::wgs84(1.0, 2.0)?);
    /// assert_eq!(Track::new(vec![untimed.clone(), untimed])?.dropped_untimed(), 2);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn dropped_untimed(&self) -> usize {
        self.dropped_untimed
    }

    /// The datum every fix is on, or `None` for an empty track.
    ///
    /// ```
    /// use std::time::UNIX_EPOCH;
    ///
    /// use geocoordinates::fix::Fix;
    /// use geocoordinates::geotag::Track;
    /// use geocoordinates::{Coordinate, Crs};
    ///
    /// let mut fix = Fix::from_coord(Coordinate::new(35.0, 139.0, Crs::Tokyo)?);
    /// fix.timestamp = Some(UNIX_EPOCH);
    /// assert_eq!(Track::new(vec![fix])?.crs(), Some(Crs::Tokyo));
    /// assert_eq!(Track::new(vec![])?.crs(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn crs(&self) -> Option<Crs> {
        self.fixes.first().map(|fix| fix.coord.crs())
    }

    /// Where the track was at the photo time `at`, or `None` when the track does
    /// not cover it.
    ///
    /// The photo time is first moved by
    /// [`clock_offset_ms`](GeotagOptions::clock_offset_ms) to track time. Then:
    ///
    /// - **At a fix's own time**, the result is that fix's position.
    /// - **Between two fixes** at most [`max_gap`](GeotagOptions::max_gap) apart,
    ///   it is placed as [`interpolation`](GeotagOptions::interpolation) says. A
    ///   geodesic interpolation also interpolates the height linearly in time
    ///   when both fixes carry one on the same surface, and has none otherwise.
    /// - **Otherwise** (before the first fix, after the last, or inside a longer
    ///   gap), it takes the position of the fix nearer in time if that fix is at
    ///   most [`max_extrapolation`](GeotagOptions::max_extrapolation) away, and is
    ///   `None` if not. The position is held, not projected along the track.
    ///
    /// The result is a new [`Fix`] on the track's datum. Its `timestamp` is the
    /// track time it was located at. Its accuracy has a horizontal magnitude
    /// only: for each fix it was derived from, that fix's own horizontal
    /// magnitude (zero where it has none), plus the farthest the device can move
    /// at [`max_speed_mps`](GeotagOptions::max_speed_mps) in the time between,
    /// plus the distance from that fix to the result; the smallest of these
    /// holds. Its kind is [`AccuracyKind::Bound`] when each of those fixes carries
    /// a `Bound` horizontal magnitude, and [`AccuracyKind::Unspecified`]
    /// otherwise. Its [`RawSource`] has full confidence, a datum ambiguity
    /// carried over from either fix, a note naming any clock offset applied, and
    /// a `raw` that says how it was derived, with times in Unix seconds:
    ///
    /// | `raw` | Derived from |
    /// |---|---|
    /// | `geotag:matched t=T` | the fix at `T` |
    /// | `geotag:interpolated t0=T0 t1=T1 f=F` | the fixes at `T0` and `T1`, a fraction `F` through the gap |
    /// | `geotag:nearest t=T t0=T0 t1=T1` | the fix at `T`, the nearer of those at `T0` and `T1` |
    /// | `geotag:extrapolated t=T dt=D` | the fix at `T`, held for `D` seconds (negative before it) |
    ///
    /// # Errors
    /// [`Error::InvalidValue`] (field `max_speed_mps`) when
    /// [`GeotagOptions::max_speed_mps`] is negative or not finite.
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    ///
    /// use geocoordinates::fix::{Accuracy, AccuracyKind, Fix};
    /// use geocoordinates::geotag::{GeotagOptions, Track};
    /// use geocoordinates::{Coordinate, Error, Length};
    ///
    /// let at = |secs| UNIX_EPOCH + Duration::from_secs(secs);
    /// let mut fix = Fix::from_coord(Coordinate::wgs84(46.0, 7.0)?);
    /// fix.timestamp = Some(at(1_000));
    /// fix.accuracy = Some(Accuracy::new(AccuracyKind::Bound, Some(Length::from_meters(5.0)), None)?);
    /// let track = Track::new(vec![fix])?;
    ///
    /// // A camera 30 s fast took this photo 10 s after the fix.
    /// let options = GeotagOptions::default()
    ///     .with_clock_offset_ms(-30_000)
    ///     .with_max_extrapolation(Duration::from_secs(60))
    ///     .with_max_speed_mps(1.5);
    /// let located = track.locate(at(1_040), &options)?.expect("within a minute of the fix");
    /// assert_eq!(located.timestamp, Some(at(1_010)));
    /// let accuracy = located.accuracy.expect("always estimated");
    /// assert_eq!(accuracy.kind(), AccuracyKind::Bound);
    /// assert_eq!(accuracy.horizontal(), Some(Length::from_meters(5.0 + 1.5 * 10.0)));
    /// assert_eq!(located.source.expect("derived").raw, "geotag:extrapolated t=1000.000 dt=10.000");
    ///
    /// let reckless = options.with_max_speed_mps(-1.0);
    /// assert!(matches!(
    ///     track.locate(at(1_040), &reckless),
    ///     Err(Error::InvalidValue { field: "max_speed_mps", .. })
    /// ));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn locate(&self, at: SystemTime, options: &GeotagOptions) -> Result<Option<Fix>> {
        let speed = options.max_speed_mps;
        if !(speed.is_finite() && speed >= 0.0) {
            return Err(Error::InvalidValue {
                field: "max_speed_mps",
                detail: format!("must be finite and nonnegative, got {speed}"),
            });
        }
        let Some(time) = shift(at, options.clock_offset_ms) else {
            return Ok(None);
        };
        let Some(derivation) = self.derivation(time, options) else {
            return Ok(None);
        };
        self.derive(time, derivation, options).map(Some)
    }

    /// Which fixes the time `time` (already in track time) is derived from.
    fn derivation(&self, time: SystemTime, options: &GeotagOptions) -> Option<Derivation> {
        // The first fix at or after `time`.
        let next = self.times.partition_point(|t| *t < time);
        if self.times.get(next) == Some(&time) {
            return Some(Derivation::Matched(next));
        }
        let before = next.checked_sub(1);
        let after = (next < self.times.len()).then_some(next);
        let bracket = before
            .zip(after)
            .filter(|&(a, b)| elapsed(self.times[a], self.times[b]) <= options.max_gap);
        if let Some((a, b)) = bracket {
            let fraction = elapsed(self.times[a], time).as_secs_f64()
                / elapsed(self.times[a], self.times[b]).as_secs_f64();
            return Some(match options.interpolation {
                Interpolation::Geodesic => Derivation::Interpolated { a, b, fraction },
                Interpolation::Nearest => Derivation::Nearest {
                    a,
                    b,
                    nearest: if elapsed(time, self.times[b]) < elapsed(self.times[a], time) {
                        b
                    } else {
                        a
                    },
                },
            });
        }
        // Outside the track's coverage: hold the nearer fix in reach, the earlier
        // on a tie.
        let reach = |i: usize| {
            let away = elapsed(self.times[i], time).max(elapsed(time, self.times[i]));
            (away <= options.max_extrapolation).then_some((away, i))
        };
        let held = match (before.and_then(reach), after.and_then(reach)) {
            (Some(a), Some(b)) => Some(if b.0 < a.0 { b } else { a }),
            (a, b) => a.or(b),
        };
        held.map(|(_, i)| Derivation::Extrapolated(i))
    }

    /// Build the located fix at `time` from the fixes `derivation` names.
    fn derive(
        &self,
        time: SystemTime,
        derivation: Derivation,
        options: &GeotagOptions,
    ) -> Result<Fix> {
        let t = |i: usize| unix_secs(self.times[i]);
        let (coord, from, raw): (Coordinate, Vec<usize>, String) = match derivation {
            Derivation::Matched(i) => (
                self.fixes[i].coord,
                vec![i],
                format!("geotag:matched t={:.3}", t(i)),
            ),
            Derivation::Interpolated { a, b, fraction } => (
                self.interpolate(a, b, fraction)?,
                vec![a, b],
                format!(
                    "geotag:interpolated t0={:.3} t1={:.3} f={fraction:.3}",
                    t(a),
                    t(b)
                ),
            ),
            Derivation::Nearest { a, b, nearest } => (
                self.fixes[nearest].coord,
                vec![a, b],
                format!(
                    "geotag:nearest t={:.3} t0={:.3} t1={:.3}",
                    t(nearest),
                    t(a),
                    t(b)
                ),
            ),
            Derivation::Extrapolated(i) => (
                self.fixes[i].coord,
                vec![i],
                format!(
                    "geotag:extrapolated t={:.3} dt={:.3}",
                    t(i),
                    unix_secs(time) - t(i)
                ),
            ),
        };

        let mut horizontal = f64::INFINITY;
        let mut bounded = true;
        for &i in &from {
            let fix = &self.fixes[i];
            let own = fix
                .accuracy
                .and_then(|a| a.horizontal().map(|h| (a.kind(), h)));
            bounded &= matches!(own, Some((AccuracyKind::Bound, _)));
            let away = elapsed(self.times[i], time)
                .max(elapsed(time, self.times[i]))
                .as_secs_f64();
            let reach = own.map_or(0.0, |(_, h)| h.meters())
                + options.max_speed_mps * away
                + geodesic_distance(fix.coord, coord)?.meters();
            horizontal = horizontal.min(reach);
        }
        let kind = if bounded {
            AccuracyKind::Bound
        } else {
            AccuracyKind::Unspecified
        };

        let mut source = RawSource::new(raw, Confidence::CERTAIN);
        source.datum_ambiguity = from
            .iter()
            .find_map(|&i| self.fixes[i].source.as_ref()?.datum_ambiguity);
        if options.clock_offset_ms != 0 {
            source.notes.push(format!(
                "clock offset of {:+} ms applied to the photo time",
                options.clock_offset_ms
            ));
        }

        let mut fix = Fix::from_coord(coord);
        // Overflows to infinity only for an absurd speed and time; then no
        // magnitude is known.
        fix.accuracy = Accuracy::new(kind, Some(Length::from_meters(horizontal)), None).ok();
        fix.timestamp = Some(time);
        fix.source = Some(source);
        Ok(fix)
    }

    /// The point `fraction` of the way along the geodesic from fix `a` to fix
    /// `b`, with a height interpolated when both carry one on the same surface.
    fn interpolate(&self, a: usize, b: usize, fraction: f64) -> Result<Coordinate> {
        let (from, to) = (self.fixes[a].coord, self.fixes[b].coord);
        let fraction = fraction.clamp(0.0, 1.0);
        let point = intermediate(from, to, fraction)?;
        let lerp = |x: f64, y: f64| x + (y - x) * fraction;
        match (from.height(), to.height()) {
            (Some(Height::Ellipsoidal(x)), Some(Height::Ellipsoidal(y))) => {
                point.with_height(Height::Ellipsoidal(lerp(x, y)))
            }
            (Some(Height::Orthometric(x)), Some(Height::Orthometric(y))) => {
                point.with_height(Height::Orthometric(lerp(x, y)))
            }
            _ => Ok(point),
        }
    }
}

/// `at` moved by `offset_ms` milliseconds, or `None` past the clock's range.
pub(super) fn shift(at: SystemTime, offset_ms: i64) -> Option<SystemTime> {
    let by = Duration::from_millis(offset_ms.unsigned_abs());
    if offset_ms >= 0 {
        at.checked_add(by)
    } else {
        at.checked_sub(by)
    }
}

/// The time from `earlier` to `later`, zero if `later` is not later.
pub(super) fn elapsed(earlier: SystemTime, later: SystemTime) -> Duration {
    later.duration_since(earlier).unwrap_or(Duration::ZERO)
}

/// Seconds since the Unix epoch, negative before it.
pub(super) fn unix_secs(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_secs_f64(),
        Err(before) => -before.duration().as_secs_f64(),
    }
}
