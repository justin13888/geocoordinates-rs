//! Estimating a camera's clock offset from photos whose position is known.

use std::time::SystemTime;

use super::Track;
use super::track::unix_secs;
use crate::geodesy::{along_track_distance, geodesic_distance, intermediate};
use crate::{Coordinate, Error, Result};

/// Suggest a [`clock_offset_ms`](super::GeotagOptions::clock_offset_ms) for a
/// camera, from reference photos whose true position is known: each pair is a
/// photo's time on the camera's clock and where it was taken (a phone photo
/// taken alongside, or a landmark in the frame).
///
/// For each reference, the time the track passed nearest its location is
/// found, on a fix or between two (assuming constant speed along the geodesic
/// between them, as [`Track::locate`] does); that time minus the photo time is
/// the reference's offset. Where the track passes equally near more than once,
/// the pass nearest the photo time is taken, so choose references where the
/// track passes once. The suggestion is the median of the references'
/// offsets, rounded to the millisecond, so one bad reference among three or
/// more whose other offsets agree does not move it.
///
/// Check the suggestion before trusting it: [`Track::locate`] each reference
/// time with it and compare the result against the known location.
///
/// # Errors
/// [`Error::InvalidValue`] (field `references`) when there are none, and
/// (field `track`) when the track is empty; [`Error::CrsMismatch`] when a
/// reference location is not on the track's datum.
///
/// ```
/// use std::time::{Duration, UNIX_EPOCH};
///
/// use geocoordinates::fix::Fix;
/// use geocoordinates::geotag::{GeotagOptions, Track, estimate_offset};
/// use geocoordinates::{Coordinate, Error};
///
/// let at = |secs| UNIX_EPOCH + Duration::from_secs(secs);
/// let fix = |secs, lon| -> Result<Fix, Error> {
///     let mut fix = Fix::from_coord(Coordinate::wgs84(0.0, lon)?);
///     fix.timestamp = Some(at(secs));
///     Ok(fix)
/// };
/// let track = Track::new(vec![fix(1_000, 0.0)?, fix(1_100, 0.01)?])?;
///
/// // The camera read 3 650 s at the place the track passed a fifth of the way
/// // through, at 1 020 s: its clock runs 2 630 s fast.
/// let reference = (at(3_650), Coordinate::wgs84(0.0, 0.002)?);
/// let offset_ms = estimate_offset(&track, &[reference])?;
/// assert_eq!(offset_ms, -2_630_000);
/// let options = GeotagOptions::default().with_clock_offset_ms(offset_ms);
/// let located = track.locate(at(3_650), &options)?.expect("on the track");
/// assert!((located.coord.lon() - 0.002).abs() < 1e-9);
///
/// assert!(matches!(
///     estimate_offset(&track, &[]),
///     Err(Error::InvalidValue { field: "references", .. })
/// ));
/// # Ok::<(), Error>(())
/// ```
pub fn estimate_offset(track: &Track, references: &[(SystemTime, Coordinate)]) -> Result<i64> {
    if references.is_empty() {
        return Err(Error::InvalidValue {
            field: "references",
            detail: "needs at least one reference photo".to_string(),
        });
    }
    let Some(crs) = track.crs() else {
        return Err(Error::InvalidValue {
            field: "track",
            detail: "an empty track has no time to match".to_string(),
        });
    };
    let mut offsets = Vec::with_capacity(references.len());
    for &(photo_time, location) in references {
        if location.crs() != crs {
            return Err(Error::CrsMismatch {
                expected: crs,
                found: location.crs(),
            });
        }
        let photo = unix_secs(photo_time);
        let passed = nearest_pass(track, location, photo)?;
        offsets.push(passed - photo);
    }
    offsets.sort_by(f64::total_cmp);
    let middle = offsets.len() / 2;
    let median = if offsets.len() % 2 == 0 {
        (offsets[middle - 1] + offsets[middle]) / 2.0
    } else {
        offsets[middle]
    };
    // Track and photo times both fit a `SystemTime`, so their difference in
    // milliseconds is far inside `i64`.
    #[allow(clippy::cast_possible_truncation)]
    Ok((median * 1_000.0).round() as i64)
}

/// The track time, in Unix seconds, at which the track passed nearest
/// `location`, the one nearest `photo` (Unix seconds) among equally near passes.
fn nearest_pass(track: &Track, location: Coordinate, photo: f64) -> Result<f64> {
    let fixes = track.fixes();
    let times: Vec<f64> = track.times().iter().copied().map(unix_secs).collect();
    let distances = fixes
        .iter()
        .map(|fix| geodesic_distance(location, fix.coord).map(|d| d.meters()))
        .collect::<Result<Vec<_>>>()?;

    // (distance in meters, track time)
    let mut best = (f64::INFINITY, 0.0);
    for (&distance, &time) in distances.iter().zip(&times) {
        consider(&mut best, (distance, time), photo);
    }
    let segments = fixes
        .windows(2)
        .zip(distances.windows(2))
        .zip(times.windows(2));
    for ((pair, ends), span) in segments {
        let (a, b) = (pair[0].coord, pair[1].coord);
        let length = geodesic_distance(a, b)?.meters();
        // By the triangle inequality no point of the segment is nearer than
        // this, so a segment that cannot beat the best is skipped. The best
        // already holds both ends, so this never skips a nearer point.
        if ends[0].min(ends[1]) - length >= best.0 {
            continue;
        }
        // (A segment of no length never gets here: its lower bound is an end's
        // own distance.) A location with no defined foot on this geodesic (a
        // pole off an equatorial segment) is as near to one end as to any
        // point: the ends already cover it.
        let Ok(along) = along_track_distance(location, a, b) else {
            continue;
        };
        let along = along.meters();
        if along <= 0.0 || along >= length {
            continue;
        }
        let fraction = along / length;
        let foot = intermediate(a, b, fraction)?;
        let distance = geodesic_distance(location, foot)?.meters();
        // A foot no nearer than an end adds nothing the end, with its exact
        // time, does not; rounding can put one a hair inside the segment.
        if distance >= ends[0].min(ends[1]) {
            continue;
        }
        let passed = span[0] + fraction * (span[1] - span[0]);
        consider(&mut best, (distance, passed), photo);
    }
    Ok(best.1)
}

/// Replace `best` with `candidate` (distance in meters, track time in Unix
/// seconds) when it is nearer, or as near and nearer in time to `photo`.
fn consider(best: &mut (f64, f64), candidate: (f64, f64), photo: f64) {
    let closer = candidate.0 < best.0;
    let as_near_but_sooner =
        candidate.0 == best.0 && (candidate.1 - photo).abs() < (best.1 - photo).abs();
    if closer || as_near_but_sooner {
        *best = candidate;
    }
}
