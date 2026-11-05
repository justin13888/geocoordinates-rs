//! Geotagging photos from a GPS track by capture time.
//!
//! [`Track`] is an object built once from fixes and immutable after, so it is
//! safe to share between threads. A reference photo, a tuple in Rust, crosses as
//! the record [`GeotagReference`].

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use geocoordinates as gc;

use crate::{Coordinate, Crs, Fix, GeoError};

/// How a time between two fixes is placed — mirror of
/// [`gc::geotag::Interpolation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Interpolation {
    /// The point on the geodesic between the two fixes, the same fraction of the
    /// way along it as the time is through the gap.
    Geodesic,
    /// The position of whichever fix is nearer in time (the earlier on a tie).
    Nearest,
}

impl From<Interpolation> for gc::geotag::Interpolation {
    fn from(interpolation: Interpolation) -> Self {
        match interpolation {
            Interpolation::Geodesic => Self::Geodesic,
            Interpolation::Nearest => Self::Nearest,
        }
    }
}

impl From<gc::geotag::Interpolation> for Interpolation {
    fn from(interpolation: gc::geotag::Interpolation) -> Self {
        match interpolation {
            gc::geotag::Interpolation::Geodesic => Self::Geodesic,
            gc::geotag::Interpolation::Nearest => Self::Nearest,
        }
    }
}

/// How [`Track::locate`] matches a photo's time against the track — mirror of
/// [`gc::geotag::GeotagOptions`]. `max_speed_mps` is checked when it is used.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct GeotagOptions {
    /// Milliseconds added to the photo's time to reach the track's (UTC) time;
    /// negative for a camera clock that runs ahead.
    pub clock_offset_ms: i64,
    /// The longest gap between two neighbouring fixes that is interpolated across.
    pub max_gap: Duration,
    /// How far beyond the recorded fixes, or into a longer gap, a photo still
    /// takes the nearest fix's position.
    pub max_extrapolation: Duration,
    /// How a time between two fixes is placed.
    pub interpolation: Interpolation,
    /// The fastest the device is assumed to move, in meters per second: the rate
    /// a located fix's accuracy grows with time. Finite and nonnegative.
    pub max_speed_mps: f64,
}

impl From<GeotagOptions> for gc::geotag::GeotagOptions {
    fn from(options: GeotagOptions) -> Self {
        Self::default()
            .with_clock_offset_ms(options.clock_offset_ms)
            .with_max_gap(options.max_gap)
            .with_max_extrapolation(options.max_extrapolation)
            .with_interpolation(options.interpolation.into())
            .with_max_speed_mps(options.max_speed_mps)
    }
}

/// The default options: no clock offset, interpolation across gaps of up to 30
/// minutes, no extrapolation, geodesic interpolation, and 50 m/s.
#[uniffi::export]
pub fn geotag_options_default() -> GeotagOptions {
    let options = gc::geotag::GeotagOptions::default();
    GeotagOptions {
        clock_offset_ms: options.clock_offset_ms,
        max_gap: options.max_gap,
        max_extrapolation: options.max_extrapolation,
        interpolation: options.interpolation.into(),
        max_speed_mps: options.max_speed_mps,
    }
}

/// A photo whose true position is known, for [`estimate_offset_ms`]: the time on
/// the camera's clock and where it was taken.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct GeotagReference {
    /// The photo's time on the camera's clock.
    pub photo_time: SystemTime,
    /// Where the photo was taken.
    pub location: Coordinate,
}

/// A GPS track: recorded fixes in time order, each with a timestamp, all on one
/// geodetic datum — the object form of [`gc::geotag::Track`].
#[derive(Debug, uniffi::Object)]
pub struct Track {
    inner: gc::geotag::Track,
}

#[uniffi::export]
impl Track {
    /// Builds a track from `fixes` in any order, dropping those with no
    /// timestamp and keeping one of each same-instant duplicate.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `fixes`) when two fixes at the same instant
    /// disagree on the position; `GeoError::CrsMismatch` when the timed fixes are
    /// on different datums or on GCJ-02 / BD-09; `GeoError::OutOfRange` /
    /// `GeoError::InvalidValue` for an invalid fix record.
    #[uniffi::constructor]
    pub fn new(fixes: Vec<Fix>) -> Result<Arc<Self>, GeoError> {
        let fixes = fixes
            .into_iter()
            .map(gc::fix::Fix::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Arc::new(Self {
            inner: gc::geotag::Track::new(fixes)?,
        }))
    }

    /// Where the track was at the photo time `at`, or `None` when the track does
    /// not cover it. The result is a derived fix: its source's `raw` starts with
    /// `geotag:`, and its accuracy grows with the time to the fixes it came from.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `max_speed_mps`) when the speed is negative
    /// or not finite.
    pub fn locate(&self, at: SystemTime, options: GeotagOptions) -> Result<Option<Fix>, GeoError> {
        Ok(self.inner.locate(at, &options.into())?.map(Into::into))
    }

    /// The kept fixes, in time order.
    pub fn fixes(&self) -> Vec<Fix> {
        self.inner.fixes().iter().cloned().map(Into::into).collect()
    }

    /// The number of kept fixes.
    pub fn len(&self) -> u64 {
        self.inner.len() as u64
    }

    /// Whether the track kept no fix, so it locates nothing.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// How many of the fixes it was built from had no timestamp and were dropped.
    pub fn dropped_untimed(&self) -> u64 {
        self.inner.dropped_untimed() as u64
    }

    /// The datum every fix is on, or `None` for an empty track.
    pub fn crs(&self) -> Option<Crs> {
        self.inner.crs().map(Into::into)
    }
}

/// Suggests a `clock_offset_ms` for a camera from reference photos whose position
/// is known: the median, over the references, of the time the track passed
/// nearest each location minus the photo's time, in milliseconds.
///
/// # Errors
/// `GeoError::InvalidValue` (field `references`) when there are none, and (field
/// `track`) when the track is empty; `GeoError::CrsMismatch` when a location is
/// not on the track's datum; `GeoError::OutOfRange` / `GeoError::InvalidValue`
/// for an invalid coordinate record.
#[uniffi::export]
pub fn estimate_offset_ms(
    track: Arc<Track>,
    references: Vec<GeotagReference>,
) -> Result<i64, GeoError> {
    let references = references
        .into_iter()
        .map(|reference| Ok((reference.photo_time, reference.location.try_into()?)))
        .collect::<Result<Vec<_>, gc::Error>>()?;
    Ok(gc::geotag::estimate_offset(&track.inner, &references)?)
}
