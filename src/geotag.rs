//! Geotagging photos from a GPS track by capture time (feature `geotag`).
//!
//! A camera with no GPS records when a photo was taken but not where. A phone or
//! watch that logged a track at the same time knows where it was at every moment,
//! so the photo's position is the track's position at the photo's time. Build a
//! [`Track`] from timestamped [`Fix`](crate::fix::Fix)es (GPX via
//! `parse::interchange::from_gpx`, NMEA `RMC` sentences, or any sequence of your
//! own), then ask it where it was with [`Track::locate`].
//!
//! Camera clocks drift and are often set to local time with no zone, so
//! [`GeotagOptions`] carries the clock offset to apply, and [`estimate_offset`]
//! suggests one from a few photos whose true position is known.
//!
//! **Every result is derived, never recorded.** A located [`Fix`](crate::fix::Fix)
//! carries a [`RawSource`](crate::fix::RawSource) whose `raw` starts with
//! `geotag:` and names how the position was derived (see [`Track::locate`]), and
//! an [`Accuracy`](crate::fix::Accuracy) that grows with the time to the nearest
//! recorded fix. Its kind is [`Bound`](crate::fix::AccuracyKind::Bound) only when
//! every fix it was derived from carries a bound, and only under the speed limit
//! [`GeotagOptions::max_speed_mps`] states; otherwise it is
//! [`Unspecified`](crate::fix::AccuracyKind::Unspecified).
//!
//! Every item is reached flat at `geotag::NAME`.
//!
//! ```
//! use std::time::{Duration, UNIX_EPOCH};
//!
//! use geocoordinates::Coordinate;
//! use geocoordinates::fix::Fix;
//! use geocoordinates::geotag::{GeotagOptions, Track};
//!
//! let at = |secs| UNIX_EPOCH + Duration::from_secs(secs);
//! let mut a = Fix::from_coord(Coordinate::wgs84(0.0, 0.0)?);
//! a.timestamp = Some(at(1_000));
//! let mut b = Fix::from_coord(Coordinate::wgs84(0.0, 0.01)?);
//! b.timestamp = Some(at(1_100));
//! let track = Track::new(vec![a, b])?;
//!
//! // A quarter of the way through the gap, a quarter of the way along.
//! let fix = track.locate(at(1_025), &GeotagOptions::default())?.expect("inside the track");
//! assert!((fix.coord.lon() - 0.0025).abs() < 1e-9);
//! assert!(fix.source.expect("derived").raw.starts_with("geotag:interpolated"));
//!
//! // Outside the track there is no position: extrapolation is off by default.
//! assert_eq!(track.locate(at(2_000), &GeotagOptions::default())?, None);
//! # Ok::<(), geocoordinates::Error>(())
//! ```

mod offset;
mod track;

#[cfg(test)]
mod golden_tests;
#[cfg(test)]
mod tests;

use std::time::Duration;

pub use self::offset::estimate_offset;
pub use self::track::Track;

/// How [`Track::locate`] places a time that falls between two recorded fixes.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every variant,
/// so adding one here is a deliberate, compile-forcing change on both sides.
///
/// ```
/// use geocoordinates::geotag::{GeotagOptions, Interpolation};
///
/// assert_eq!(GeotagOptions::default().interpolation, Interpolation::Geodesic);
/// let nearest = GeotagOptions::default().with_interpolation(Interpolation::Nearest);
/// assert_eq!(nearest.interpolation, Interpolation::Nearest);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Interpolation {
    /// The point on the geodesic between the two fixes, the same fraction of the
    /// way along it as the time is through the gap: constant speed along the
    /// shortest path, which crosses the antimeridian where that path does.
    #[default]
    Geodesic,
    /// The position of whichever fix is nearer in time (the earlier on a tie).
    Nearest,
}

/// How [`Track::locate`] matches a photo's time against the track.
///
/// `#[non_exhaustive]`: build it from [`Default`] and the `with_*` builders. The
/// defaults follow ExifTool's `-geotag`: no clock offset, interpolation across gaps
/// of up to 30 minutes, and no extrapolation beyond the recorded fixes.
///
/// ```
/// use std::time::Duration;
///
/// use geocoordinates::geotag::GeotagOptions;
///
/// // A camera set to UTC+2 with no zone: its 14:00 is 12:00 UTC.
/// let options = GeotagOptions::default()
///     .with_clock_offset_ms(-2 * 3_600_000)
///     .with_max_gap(Duration::from_secs(600))
///     .with_max_extrapolation(Duration::from_secs(60));
/// assert_eq!(options.clock_offset_ms, -7_200_000);
/// assert_eq!(options.max_gap, Duration::from_secs(600));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct GeotagOptions {
    /// Milliseconds added to the photo's time to reach the track's (UTC) time:
    /// `track time = photo time + clock_offset_ms`. Negative for a camera clock
    /// that runs ahead, such as one set to a zone east of UTC. Default 0.
    pub clock_offset_ms: i64,
    /// The longest gap between two neighbouring fixes that is interpolated
    /// across. A photo inside a longer gap is treated as outside the track, so
    /// only [`max_extrapolation`](Self::max_extrapolation) can place it. Default
    /// 30 minutes.
    pub max_gap: Duration,
    /// How far beyond the recorded fixes (before the first, after the last, or
    /// into a gap longer than [`max_gap`](Self::max_gap)) a photo still takes the
    /// nearest fix's position. Default zero: a photo outside the track has no
    /// position.
    pub max_extrapolation: Duration,
    /// How a time between two fixes is placed. Default
    /// [`Interpolation::Geodesic`].
    pub interpolation: Interpolation,
    /// The fastest the track's device is assumed to move, in meters per second:
    /// the rate at which a located fix's accuracy grows with the time to the
    /// fixes it was derived from. A [`Bound`](crate::fix::AccuracyKind::Bound)
    /// result holds only if the device never moved faster. Finite and
    /// nonnegative; default 50 m/s (180 km/h).
    pub max_speed_mps: f64,
}

impl Default for GeotagOptions {
    fn default() -> Self {
        Self {
            clock_offset_ms: 0,
            max_gap: Duration::from_secs(30 * 60),
            max_extrapolation: Duration::ZERO,
            interpolation: Interpolation::Geodesic,
            max_speed_mps: 50.0,
        }
    }
}

impl GeotagOptions {
    /// Sets the clock offset, in milliseconds added to a photo's time.
    ///
    /// ```
    /// use geocoordinates::geotag::GeotagOptions;
    ///
    /// assert_eq!(GeotagOptions::default().with_clock_offset_ms(1_500).clock_offset_ms, 1_500);
    /// ```
    #[must_use]
    pub const fn with_clock_offset_ms(mut self, clock_offset_ms: i64) -> Self {
        self.clock_offset_ms = clock_offset_ms;
        self
    }

    /// Sets the longest gap between fixes that is interpolated across.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use geocoordinates::geotag::GeotagOptions;
    ///
    /// let options = GeotagOptions::default().with_max_gap(Duration::from_secs(120));
    /// assert_eq!(options.max_gap.as_secs(), 120);
    /// ```
    #[must_use]
    pub const fn with_max_gap(mut self, max_gap: Duration) -> Self {
        self.max_gap = max_gap;
        self
    }

    /// Sets how far beyond the recorded fixes a photo still takes the nearest
    /// fix's position.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use geocoordinates::geotag::GeotagOptions;
    ///
    /// let options = GeotagOptions::default().with_max_extrapolation(Duration::from_secs(30));
    /// assert_eq!(options.max_extrapolation.as_secs(), 30);
    /// ```
    #[must_use]
    pub const fn with_max_extrapolation(mut self, max_extrapolation: Duration) -> Self {
        self.max_extrapolation = max_extrapolation;
        self
    }

    /// Sets how a time between two fixes is placed.
    ///
    /// ```
    /// use geocoordinates::geotag::{GeotagOptions, Interpolation};
    ///
    /// let options = GeotagOptions::default().with_interpolation(Interpolation::Nearest);
    /// assert_eq!(options.interpolation, Interpolation::Nearest);
    /// ```
    #[must_use]
    pub const fn with_interpolation(mut self, interpolation: Interpolation) -> Self {
        self.interpolation = interpolation;
        self
    }

    /// Sets the fastest the device is assumed to move, in meters per second.
    ///
    /// ```
    /// use geocoordinates::geotag::GeotagOptions;
    ///
    /// // Walking pace.
    /// assert_eq!(GeotagOptions::default().with_max_speed_mps(2.0).max_speed_mps, 2.0);
    /// ```
    #[must_use]
    pub const fn with_max_speed_mps(mut self, max_speed_mps: f64) -> Self {
        self.max_speed_mps = max_speed_mps;
        self
    }
}
