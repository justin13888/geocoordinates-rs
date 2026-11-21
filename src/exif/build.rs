//! The tag interpretation shared by the IFD and XMP paths: both reduce their input
//! to [`Tags`], so one set of rules decides both.

use std::time::{Duration, SystemTime};

use super::{
    AuxiliaryTagPolicy, CaptureGeo, ExifOptions, NorthBearing, NorthReference, NullIslandPolicy,
    no_fix,
};
use crate::china::outside_china_box;
use crate::coord::{Coordinate, Crs, Height};
use crate::fix::{Accuracy, AccuracyKind, Confidence, DatumAmbiguity, Fix, RawSource};
use crate::units::{Length, LengthUnit, days_from_civil};
use crate::{Error, Result};

/// Confidence when `GPSMapDatum` did not name a datum and WGS-84 was assumed.
const CONFIDENCE_DATUM_ASSUMED: Confidence = Confidence::from_constant(0.7);

const KMH_TO_MPS: f64 = LengthUnit::Kilometer.meters_per_unit() / 3600.0;
const MPH_TO_MPS: f64 = LengthUnit::Mile.meters_per_unit() / 3600.0;
const KNOT_TO_MPS: f64 = LengthUnit::NauticalMile.meters_per_unit() / 3600.0;

/// Tag values with their numbers already read, from either path. An auxiliary tag
/// that failed to read keeps its error, for [`Tags::build`] to apply the
/// [`AuxiliaryTagPolicy`] to.
pub(super) struct Tags<'a> {
    pub raw: String,
    pub latitude: [f64; 3],
    pub latitude_ref: Option<&'a str>,
    pub longitude: [f64; 3],
    pub longitude_ref: Option<&'a str>,
    pub altitude: Result<Option<f64>>,
    pub altitude_ref: Result<Option<u8>>,
    pub map_datum: Option<&'a str>,
    pub h_positioning_error: Result<Option<f64>>,
    pub dop: Result<Option<f64>>,
    pub stamp: Result<Stamp>,
    pub img_direction: Result<Option<f64>>,
    pub img_direction_ref: Option<&'a str>,
    pub speed: Result<Option<f64>>,
    pub speed_ref: Option<&'a str>,
    pub track: Result<Option<f64>>,
    pub track_ref: Option<&'a str>,
}

/// EXIF string padding removed: surrounding whitespace and NULs.
pub(super) fn clean(value: &str) -> &str {
    value.trim_matches(|c: char| c.is_whitespace() || c == '\0')
}

/// The tag's cleaned value, with a blank one read as absent.
pub(super) fn present(value: Option<&str>) -> Option<&str> {
    value.map(clean).filter(|v| !v.is_empty())
}

impl Tags<'_> {
    pub(super) fn build(self, options: ExifOptions) -> Result<CaptureGeo> {
        let lat = signed(self.latitude, self.latitude_ref, Axis::LATITUDE)?;
        let lon = signed(self.longitude, self.longitude_ref, Axis::LONGITUDE)?;

        let datum = present(self.map_datum);
        let crs = datum.and_then(crs_for_datum);
        let mut coord = Coordinate::new(lat, lon, crs.unwrap_or(Crs::Wgs84))?;
        if options.null_island == NullIslandPolicy::Reject && lat == 0.0 && lon == 0.0 {
            return Err(no_fix("position is exactly 0, 0"));
        }

        let confidence = if crs.is_some() {
            Confidence::CERTAIN
        } else {
            CONFIDENCE_DATUM_ASSUMED
        };
        let mut source = RawSource::new(self.raw, confidence);
        if crs.is_none() {
            source.notes.push(match datum {
                Some(name) => format!("GPSMapDatum {name:?} not recognised; assumed WGS-84"),
                None => "GPSMapDatum absent; assumed WGS-84".to_string(),
            });
            if !outside_china_box(lat, lon) {
                source.datum_ambiguity = Some(DatumAmbiguity::PossiblyGcj02);
            }
        }

        // Each value / Ref pair is read as one result, so either half failing drops
        // both under `AuxiliaryTagPolicy::Drop`.
        let mut auxiliary = Auxiliary {
            policy: options.invalid_auxiliary,
            notes: Vec::new(),
        };
        let altitude_ref = self.altitude_ref;
        let with_height = self.altitude.and_then(|meters| {
            height(meters, altitude_ref?)?
                .map(|height| coord.with_height(height))
                .transpose()
        });
        if let Some(with_height) = auxiliary.keep("GPSAltitude/GPSAltitudeRef", with_height)? {
            coord = with_height;
        }
        let accuracy = self.h_positioning_error.and_then(|error| {
            error
                .map(|e| Accuracy::new(AccuracyKind::Bound, Some(Length::from_meters(e)), None))
                .transpose()
        });
        let accuracy = auxiliary.keep("GPSHPositioningError", accuracy)?;
        let dop = auxiliary.keep("GPSDOP", self.dop)?;
        let stamp = auxiliary.keep("GPSTimeStamp", self.stamp.map(Some))?;
        let speed_ref = self.speed_ref;
        let speed = self.speed.and_then(|speed| {
            let factor = present(speed_ref).map(speed_factor).transpose()?;
            Ok(Some((speed, factor)))
        });
        let speed = auxiliary.keep("GPSSpeed/GPSSpeedRef", speed)?;
        let img_direction_ref = self.img_direction_ref;
        let direction = self.img_direction.and_then(|degrees| {
            bearing(
                degrees,
                img_direction_ref,
                "img_direction",
                "img_direction_ref",
            )
        });
        let direction = auxiliary.keep("GPSImgDirection/GPSImgDirectionRef", direction)?;
        let track_ref = self.track_ref;
        let track = self
            .track
            .and_then(|degrees| bearing(degrees, track_ref, "track", "track_ref"));
        let track = auxiliary.keep("GPSTrack/GPSTrackRef", track)?;

        let (time, stamp_note) = stamp.map_or((None, None), |stamp| (stamp.time, stamp.note));
        source.notes.extend(stamp_note);
        let speed_mps = match speed {
            Some((Some(speed), Some(factor))) => Some(speed * factor),
            Some((Some(_), None)) => {
                source
                    .notes
                    .push("GPSSpeed has no GPSSpeedRef, so its unit is unknown; dropped".into());
                None
            }
            Some((None, _)) | None => None,
        };
        source.notes.extend(auxiliary.notes);

        let mut fix = Fix::from_coord(coord);
        fix.accuracy = accuracy;
        fix.timestamp = time;
        fix.source = Some(source);

        Ok(CaptureGeo {
            fix,
            direction,
            speed_mps,
            track,
            dop,
        })
    }
}

/// The [`AuxiliaryTagPolicy`] in force, and a note for each tag it dropped.
struct Auxiliary {
    policy: AuxiliaryTagPolicy,
    notes: Vec<String>,
}

impl Auxiliary {
    /// An auxiliary tag's value. An invalid one is its error under
    /// [`AuxiliaryTagPolicy::Reject`], and `None` with a note naming `tags` under
    /// [`AuxiliaryTagPolicy::Drop`].
    fn keep<T>(&mut self, tags: &str, value: Result<Option<T>>) -> Result<Option<T>> {
        match (value, self.policy) {
            (Ok(value), _) => Ok(value),
            (Err(error), AuxiliaryTagPolicy::Reject) => Err(error),
            (Err(error), AuxiliaryTagPolicy::Drop) => {
                self.notes.push(format!("{tags} invalid, dropped: {error}"));
                Ok(None)
            }
        }
    }
}

/// The tag names and hemisphere letters of one axis.
struct Axis {
    field: &'static str,
    ref_field: &'static str,
    positive: &'static str,
    negative: &'static str,
}

impl Axis {
    const LATITUDE: Self = Self {
        field: "latitude",
        ref_field: "latitude_ref",
        positive: "N",
        negative: "S",
    };
    const LONGITUDE: Self = Self {
        field: "longitude",
        ref_field: "longitude_ref",
        positive: "E",
        negative: "W",
    };
}

/// Signed decimal degrees from degrees, minutes, seconds and a hemisphere letter.
fn signed(components: [f64; 3], reference: Option<&str>, axis: Axis) -> Result<f64> {
    let [degrees, minutes, seconds] = components;
    if minutes >= 60.0 || seconds >= 60.0 {
        return Err(Error::InvalidValue {
            field: axis.field,
            detail: format!("minutes and seconds must be under 60, got {minutes} and {seconds}"),
        });
    }
    let magnitude = degrees + minutes / 60.0 + seconds / 3600.0;
    let letter = reference.map_or("", clean);
    if letter.eq_ignore_ascii_case(axis.positive) {
        Ok(magnitude)
    } else if letter.eq_ignore_ascii_case(axis.negative) {
        Ok(-magnitude)
    } else {
        Err(Error::InvalidValue {
            field: axis.ref_field,
            detail: format!(
                "must be {} or {}, got {letter:?}",
                axis.positive, axis.negative
            ),
        })
    }
}

/// The CRS a `GPSMapDatum` value names, ignoring case and punctuation.
fn crs_for_datum(name: &str) -> Option<Crs> {
    let key: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    match key.as_str() {
        "WGS84" | "WGS1984" | "WORLDGEODETICSYSTEM1984" | "EPSG4326" => Some(Crs::Wgs84),
        "TOKYO" => Some(Crs::Tokyo),
        "GCJ02" => Some(Crs::Gcj02),
        "BD09" => Some(Crs::Bd09),
        _ => None,
    }
}

/// The height a `GPSAltitude` magnitude and `GPSAltitudeRef` describe; the Ref is
/// checked with or without a magnitude. Subtracting from zero keeps a zero magnitude
/// below the surface from reading as `-0.0`.
fn height(meters: Option<f64>, reference: Option<u8>) -> Result<Option<Height>> {
    let surface: fn(f64) -> Height = match reference.unwrap_or(0) {
        0 => Height::Orthometric,
        1 => |m| Height::Orthometric(0.0 - m),
        2 => Height::Ellipsoidal,
        3 => |m| Height::Ellipsoidal(0.0 - m),
        other => {
            return Err(Error::InvalidValue {
                field: "altitude_ref",
                detail: format!("must be 0, 1, 2 or 3, got {other}"),
            });
        }
    };
    Ok(meters.map(surface))
}

fn speed_factor(unit: &str) -> Result<f64> {
    match unit.to_ascii_uppercase().as_str() {
        "K" => Ok(KMH_TO_MPS),
        "M" => Ok(MPH_TO_MPS),
        "N" => Ok(KNOT_TO_MPS),
        _ => Err(Error::InvalidValue {
            field: "speed_ref",
            detail: format!("must be K, M or N, got {unit:?}"),
        }),
    }
}

fn bearing(
    degrees: Option<f64>,
    reference: Option<&str>,
    field: &'static str,
    ref_field: &'static str,
) -> Result<Option<NorthBearing>> {
    let reference = match present(reference).map(str::to_ascii_uppercase).as_deref() {
        None => None,
        Some("T") => Some(NorthReference::True),
        Some("M") => Some(NorthReference::Magnetic),
        Some(other) => {
            return Err(Error::InvalidValue {
                field: ref_field,
                detail: format!("must be T or M, got {other:?}"),
            });
        }
    };
    degrees
        .map(|degrees| NorthBearing::checked(degrees, reference, field))
        .transpose()
}

/// The UTC timestamp of a stamp, and a note when its date could not be read.
pub(super) struct Stamp {
    pub time: Option<SystemTime>,
    pub note: Option<String>,
}

impl Stamp {
    /// Combine a date with a time of day at `offset_minutes` ahead of UTC.
    ///
    /// An out-of-range time is an error; a missing half, or a date that does not
    /// read as a calendar date, is no timestamp.
    pub(super) fn new(
        date: Option<&str>,
        time: Option<[f64; 3]>,
        offset_minutes: i64,
    ) -> Result<Self> {
        if let Some([hours, minutes, seconds]) = time {
            if hours >= 24.0 || minutes >= 60.0 || seconds >= 60.0 {
                return Err(Error::InvalidValue {
                    field: "time_stamp",
                    detail: format!("not a time of day: {hours}:{minutes}:{seconds}"),
                });
            }
        }
        let Some(date) = present(date) else {
            return Ok(Self {
                time: None,
                note: None,
            });
        };
        let Some(days) = days_from_date(date) else {
            return Ok(Self {
                time: None,
                note: Some(format!("date {date:?} unreadable; no timestamp")),
            });
        };
        Ok(Self {
            time: time.and_then(|time| instant(days, time, offset_minutes)),
            note: None,
        })
    }
}

/// Days since the Unix epoch of a `YYYY:MM:DD` or `YYYY-MM-DD` date.
fn days_from_date(date: &str) -> Option<i64> {
    let mut parts = date.split([':', '-']);
    let year = fixed_digits(parts.next()?, 4)?;
    let month = fixed_digits(parts.next()?, 2)?;
    let day = fixed_digits(parts.next()?, 2)?;
    if parts.next().is_some() || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=month_days)
        .contains(&day)
        .then(|| days_from_civil(year, month, day))
}

fn fixed_digits(text: &str, width: usize) -> Option<i64> {
    (text.len() == width && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// The instant of a time of day on a day, taking the fraction apart from the whole
/// seconds so a fractional-seconds rational keeps its nanoseconds.
fn instant(
    days: i64,
    [hours, minutes, seconds]: [f64; 3],
    offset_minutes: i64,
) -> Option<SystemTime> {
    let hours_minutes = hours * 3600.0 + minutes * 60.0;
    let whole = hours_minutes.floor() + seconds.floor();
    let fraction = hours_minutes.fract() + seconds.fract();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "whole is below 90 000 and an integer"
    )]
    let total = days * 86_400 - offset_minutes * 60 + whole as i64;
    let at_second = if total >= 0 {
        SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(total.unsigned_abs()))?
    } else {
        SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(total.unsigned_abs()))?
    };
    // Rounded to the microsecond, past which a rational's f64 fraction is noise.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "fraction is in [0, 2), so the product fits and is nonnegative"
    )]
    let micros = (fraction * 1e6).round() as u64;
    at_second.checked_add(Duration::from_micros(micros))
}
