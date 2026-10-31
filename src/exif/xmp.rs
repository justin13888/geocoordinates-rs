//! The XMP `exif:GPS*` string forms, reduced to the same tags as the IFD path.

use super::build::{Stamp, Tags, clean, present};
use super::{CaptureGeo, ExifOptions, no_fix};
use crate::{Error, ParseErrorKind, Result};

/// The XMP `exif:GPS*` property values, as a reader extracted them. Every field is
/// optional.
///
/// `#[non_exhaustive]`, so a property can be added without a breaking release:
/// outside this crate an `XmpGps` is built with [`Default`] and its public fields
/// are then set one by one.
///
/// ```
/// use geocoordinates::exif::XmpGps;
///
/// let mut gps = XmpGps::default();
/// gps.latitude = Some("51,45.7445N".to_string());
/// assert!(gps.longitude.is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct XmpGps {
    /// `exif:GPSLatitude`: `DD,MM.mmmmK` or `DD,MM,SSK`, with `K` being `N` or `S`.
    pub latitude: Option<String>,
    /// `exif:GPSLongitude`: `DD,MM.mmmmK` or `DD,MM,SSK`, with `K` being `E` or `W`.
    pub longitude: Option<String>,
    /// `exif:GPSAltitude`: a rational (`27704/249`) or decimal, in meters.
    pub altitude: Option<String>,
    /// `exif:GPSAltitudeRef`: `0`, `1`, `2` or `3`, as for
    /// [`ExifGps::altitude_ref`](super::ExifGps::altitude_ref).
    pub altitude_ref: Option<String>,
    /// `exif:GPSMapDatum`.
    pub map_datum: Option<String>,
    /// `exif:GPSHPositioningError`: a rational or decimal, in meters.
    pub h_positioning_error: Option<String>,
    /// `exif:GPSDOP`: a rational or decimal.
    pub dop: Option<String>,
    /// `exif:GPSTimeStamp`: `YYYY-MM-DDThh:mm:ss[.s]` and a `Z` or `±hh:mm` zone.
    pub time_stamp: Option<String>,
    /// `exif:GPSImgDirection`: a rational or decimal, in degrees.
    pub img_direction: Option<String>,
    /// `exif:GPSImgDirectionRef`: `T` or `M`.
    pub img_direction_ref: Option<String>,
    /// `exif:GPSSpeed`: a rational or decimal.
    pub speed: Option<String>,
    /// `exif:GPSSpeedRef`: `K`, `M` or `N`.
    pub speed_ref: Option<String>,
    /// `exif:GPSTrack`: a rational or decimal, in degrees.
    pub track: Option<String>,
    /// `exif:GPSTrackRef`: `T` or `M`.
    pub track_ref: Option<String>,
}

/// Convert XMP GPS properties with the default [`ExifOptions`].
///
/// See [`from_xmp_with`] for the rules and errors.
///
/// # Errors
/// As [`from_xmp_with`].
///
/// ```
/// use geocoordinates::exif::{XmpGps, from_xmp};
/// use geocoordinates::{Error, ParseErrorKind};
///
/// let mut gps = XmpGps::default();
/// gps.latitude = Some("51,45,44.67N".to_string());
/// gps.longitude = Some("0,19,4.41W".to_string());
/// assert!((from_xmp(&gps)?.fix.coord.lat() - 51.762_408).abs() < 1e-6);
///
/// gps.longitude = Some("nowhere".to_string());
/// assert!(matches!(
///     from_xmp(&gps),
///     Err(Error::Parse { kind: ParseErrorKind::Syntax, .. })
/// ));
/// # Ok::<(), Error>(())
/// ```
pub fn from_xmp(gps: &XmpGps) -> Result<CaptureGeo> {
    from_xmp_with(gps, ExifOptions::default())
}

/// Convert XMP GPS properties to a [`CaptureGeo`], under the rules of
/// [`from_exif_with`](super::from_exif_with). The same values give the same result
/// by either path, apart from [`RawSource::raw`](crate::fix::RawSource::raw).
///
/// A blank property reads as absent, so a blank latitude or longitude is
/// [`ParseErrorKind::NoFix`]. A `time_stamp` missing its date, seconds or zone gives
/// no timestamp, with a note.
///
/// # Errors
/// As [`from_exif_with`](super::from_exif_with); [`Error::Parse`] with
/// [`ParseErrorKind::Syntax`] for a coordinate or number that does not read in its
/// XMP form, which includes `NaN` and infinities; and [`Error::InvalidValue`] naming
/// the field for a rational whose quotient overflows to infinity, or a `time_stamp`
/// zone outside `±23:59`.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::exif::{ExifOptions, XmpGps, from_xmp_with};
///
/// let mut gps = XmpGps::default();
/// gps.latitude = Some("51,60.0N".to_string());
/// gps.longitude = Some("0,19.5W".to_string());
/// assert!(matches!(
///     from_xmp_with(&gps, ExifOptions::default()),
///     Err(Error::InvalidValue { field: "latitude", .. })
/// ));
/// ```
pub fn from_xmp_with(gps: &XmpGps, options: ExifOptions) -> Result<CaptureGeo> {
    let (Some(latitude), Some(longitude)) = (
        present(gps.latitude.as_deref()),
        present(gps.longitude.as_deref()),
    ) else {
        return Err(no_fix("GPSLatitude and GPSLongitude are both required"));
    };
    let (latitude_components, latitude_ref) = coordinate(latitude, "latitude")?;
    let (longitude_components, longitude_ref) = coordinate(longitude, "longitude")?;
    let tags = Tags {
        raw: format!("GPSLatitude={latitude}; GPSLongitude={longitude}"),
        latitude: latitude_components,
        latitude_ref: Some(latitude_ref),
        longitude: longitude_components,
        longitude_ref: Some(longitude_ref),
        altitude: number(gps.altitude.as_deref(), "altitude")?,
        altitude_ref: altitude_ref(gps.altitude_ref.as_deref())?,
        map_datum: gps.map_datum.as_deref(),
        h_positioning_error: number(gps.h_positioning_error.as_deref(), "h_positioning_error")?,
        dop: number(gps.dop.as_deref(), "dop")?,
        stamp: stamp(gps.time_stamp.as_deref())?,
        img_direction: number(gps.img_direction.as_deref(), "img_direction")?,
        img_direction_ref: gps.img_direction_ref.as_deref(),
        speed: number(gps.speed.as_deref(), "speed")?,
        speed_ref: gps.speed_ref.as_deref(),
        track: number(gps.track.as_deref(), "track")?,
        track_ref: gps.track_ref.as_deref(),
    };
    tags.build(options)
}

fn syntax(field: &str, value: &str) -> Error {
    Error::Parse {
        kind: ParseErrorKind::Syntax,
        detail: format!("{field}: cannot read {value:?}"),
    }
}

/// Split `DD,MM.mmmmK` or `DD,MM,SSK` into degrees, minutes, seconds and the letter.
fn coordinate<'a>(value: &'a str, field: &str) -> Result<([f64; 3], &'a str)> {
    let split = value
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
        .ok_or_else(|| syntax(field, value))?;
    let (numbers, letter) = value.split_at(split);
    let parts: Vec<&str> = numbers.split(',').collect();
    let read = |part: &str| decimal(part).ok_or_else(|| syntax(field, value));
    match parts[..] {
        [degrees, minutes] => Ok(([read(degrees)?, read(minutes)?, 0.0], letter)),
        [degrees, minutes, seconds] => {
            Ok(([read(degrees)?, read(minutes)?, read(seconds)?], letter))
        }
        _ => Err(syntax(field, value)),
    }
}

/// An unsigned decimal: digits and a point, finite. `f64::from_str` alone would also
/// take `NaN`, `inf`, signs and exponents; it does reject an empty string, a lone
/// point and a second point.
fn decimal(text: &str) -> Option<f64> {
    if !text.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    text.parse().ok().filter(|v: &f64| v.is_finite())
}

/// A blank-as-absent `n/d` or decimal property.
fn number(value: Option<&str>, field: &'static str) -> Result<Option<f64>> {
    let Some(value) = value.map(clean).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let Some((numerator, denominator)) = value.split_once('/') else {
        return decimal(value).map(Some).ok_or_else(|| syntax(field, value));
    };
    let numerator = decimal(numerator).ok_or_else(|| syntax(field, value))?;
    let denominator = decimal(denominator).ok_or_else(|| syntax(field, value))?;
    if denominator == 0.0 {
        return Err(Error::InvalidValue {
            field,
            detail: format!("zero denominator in {value}"),
        });
    }
    let quotient = numerator / denominator;
    if !quotient.is_finite() {
        return Err(Error::InvalidValue {
            field,
            detail: format!("{value} overflows to infinity"),
        });
    }
    Ok(Some(quotient))
}

fn altitude_ref(value: Option<&str>) -> Result<Option<u8>> {
    let Some(value) = value.map(clean).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    value.parse().map(Some).map_err(|_| Error::InvalidValue {
        field: "altitude_ref",
        detail: format!("must be 0, 1, 2 or 3, got {value:?}"),
    })
}

/// Read `YYYY-MM-DDThh:mm:ss[.s]` with a zone. A value missing a part gives no
/// timestamp, with a note; a part out of range is the same error as the IFD path.
fn stamp(value: Option<&str>) -> Result<Stamp> {
    let Some(value) = value.map(clean).filter(|v| !v.is_empty()) else {
        return Stamp::new(None, None, 0);
    };
    let Some((date, time, offset)) = date_time_zone(value) else {
        return Ok(Stamp {
            time: None,
            note: Some(format!("time stamp {value:?} is partial; no timestamp")),
        });
    };
    Stamp::new(Some(date), Some(time), offset?)
}

/// The date, time of day and zone of a stamp; `None` when a part is missing, and an
/// error in the zone's place when the zone is out of range.
fn date_time_zone(value: &str) -> Option<(&str, [f64; 3], Result<i64>)> {
    let (date, rest) = value.split_once('T')?;
    let (clock, offset) = if let Some(clock) = rest.strip_suffix(['Z', 'z']) {
        (clock, Ok(0))
    } else {
        let at = rest.rfind(['+', '-'])?;
        let (clock, zone) = rest.split_at(at);
        (clock, zone_minutes(zone)?)
    };
    let mut parts = clock.split(':').map(decimal);
    let time = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some((date, time, offset))
}

/// `+hh:mm` or `-hh:mm` as signed minutes ahead of UTC.
fn zone_minutes(zone: &str) -> Option<Result<i64>> {
    let (sign, digits) = zone.split_at(1);
    let (hours, minutes) = digits.split_once(':')?;
    let hours: i64 = (hours.len() == 2).then(|| hours.parse().ok())??;
    let minutes: i64 = (minutes.len() == 2).then(|| minutes.parse().ok())??;
    if hours >= 24 || minutes >= 60 {
        return Some(Err(Error::InvalidValue {
            field: "time_stamp",
            detail: format!("zone must be within ±23:59, got {zone}"),
        }));
    }
    let total = hours * 60 + minutes;
    Some(Ok(if sign == "-" { -total } else { total }))
}
