//! EXIF / XMP GPS tag values to a validated capture position.
//!
//! The tag values cross as plain records with every field optional. A rational
//! crosses as the [`ExifRational`] enum, and a three-rational tag as a named record,
//! since tuples and arrays cannot cross the boundary.

use geocoordinates as gc;

use crate::{Fix, GeoError};

/// One EXIF rational exactly as stored — mirror of [`gc::exif::ExifRational`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ExifRational {
    /// An EXIF `RATIONAL`.
    Unsigned {
        /// Numerator.
        numerator: u32,
        /// Denominator.
        denominator: u32,
    },
    /// An EXIF `SRATIONAL`.
    Signed {
        /// Numerator.
        numerator: i32,
        /// Denominator.
        denominator: i32,
    },
}

impl From<ExifRational> for gc::exif::ExifRational {
    fn from(rational: ExifRational) -> Self {
        match rational {
            ExifRational::Unsigned {
                numerator,
                denominator,
            } => Self::Unsigned(numerator, denominator),
            ExifRational::Signed {
                numerator,
                denominator,
            } => Self::Signed(numerator, denominator),
        }
    }
}

/// `GPSLatitude` / `GPSLongitude` as stored. Any component may be fractional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ExifDms {
    /// Degrees.
    pub degrees: ExifRational,
    /// Minutes.
    pub minutes: ExifRational,
    /// Seconds.
    pub seconds: ExifRational,
}

impl From<ExifDms> for [gc::exif::ExifRational; 3] {
    fn from(dms: ExifDms) -> Self {
        [dms.degrees.into(), dms.minutes.into(), dms.seconds.into()]
    }
}

/// `GPSTimeStamp` as stored: a UTC time of day. Any component may be fractional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ExifTime {
    /// Hours.
    pub hours: ExifRational,
    /// Minutes.
    pub minutes: ExifRational,
    /// Seconds.
    pub seconds: ExifRational,
}

impl From<ExifTime> for [gc::exif::ExifRational; 3] {
    fn from(time: ExifTime) -> Self {
        [time.hours.into(), time.minutes.into(), time.seconds.into()]
    }
}

/// The raw values of an EXIF GPS IFD — mirror of [`gc::exif::ExifGps`]. Every field
/// is optional. No checks apply until [`parse_exif`] reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct ExifGps {
    /// `GPSLatitude`.
    #[uniffi(default)]
    pub latitude: Option<ExifDms>,
    /// `GPSLatitudeRef`: `N` or `S`.
    #[uniffi(default)]
    pub latitude_ref: Option<String>,
    /// `GPSLongitude`.
    #[uniffi(default)]
    pub longitude: Option<ExifDms>,
    /// `GPSLongitudeRef`: `E` or `W`.
    #[uniffi(default)]
    pub longitude_ref: Option<String>,
    /// `GPSAltitude`, a nonnegative magnitude in meters.
    #[uniffi(default)]
    pub altitude: Option<ExifRational>,
    /// `GPSAltitudeRef`: 0 above sea level, 1 below, 2 above the ellipsoid, 3 below it.
    #[uniffi(default)]
    pub altitude_ref: Option<u8>,
    /// `GPSMapDatum`.
    #[uniffi(default)]
    pub map_datum: Option<String>,
    /// `GPSHPositioningError`, in meters.
    #[uniffi(default)]
    pub h_positioning_error: Option<ExifRational>,
    /// `GPSDOP`.
    #[uniffi(default)]
    pub dop: Option<ExifRational>,
    /// `GPSDateStamp`: `YYYY:MM:DD`.
    #[uniffi(default)]
    pub date_stamp: Option<String>,
    /// `GPSTimeStamp`.
    #[uniffi(default)]
    pub time_stamp: Option<ExifTime>,
    /// `GPSImgDirection`, in degrees.
    #[uniffi(default)]
    pub img_direction: Option<ExifRational>,
    /// `GPSImgDirectionRef`: `T` or `M`.
    #[uniffi(default)]
    pub img_direction_ref: Option<String>,
    /// `GPSSpeed`, in the unit of `speed_ref`.
    #[uniffi(default)]
    pub speed: Option<ExifRational>,
    /// `GPSSpeedRef`: `K` (km/h), `M` (mph) or `N` (knots).
    #[uniffi(default)]
    pub speed_ref: Option<String>,
    /// `GPSTrack`, in degrees.
    #[uniffi(default)]
    pub track: Option<ExifRational>,
    /// `GPSTrackRef`: `T` or `M`.
    #[uniffi(default)]
    pub track_ref: Option<String>,
}

impl From<ExifGps> for gc::exif::ExifGps {
    fn from(gps: ExifGps) -> Self {
        let mut out = Self::default();
        out.latitude = gps.latitude.map(Into::into);
        out.latitude_ref = gps.latitude_ref;
        out.longitude = gps.longitude.map(Into::into);
        out.longitude_ref = gps.longitude_ref;
        out.altitude = gps.altitude.map(Into::into);
        out.altitude_ref = gps.altitude_ref;
        out.map_datum = gps.map_datum;
        out.h_positioning_error = gps.h_positioning_error.map(Into::into);
        out.dop = gps.dop.map(Into::into);
        out.date_stamp = gps.date_stamp;
        out.time_stamp = gps.time_stamp.map(Into::into);
        out.img_direction = gps.img_direction.map(Into::into);
        out.img_direction_ref = gps.img_direction_ref;
        out.speed = gps.speed.map(Into::into);
        out.speed_ref = gps.speed_ref;
        out.track = gps.track.map(Into::into);
        out.track_ref = gps.track_ref;
        out
    }
}

/// The XMP `exif:GPS*` property values — mirror of [`gc::exif::XmpGps`]. Every field
/// is optional. No checks apply until [`parse_xmp`] reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct XmpGps {
    /// `exif:GPSLatitude`: `DD,MM.mmmmK` or `DD,MM,SSK`.
    #[uniffi(default)]
    pub latitude: Option<String>,
    /// `exif:GPSLongitude`: `DD,MM.mmmmK` or `DD,MM,SSK`.
    #[uniffi(default)]
    pub longitude: Option<String>,
    /// `exif:GPSAltitude`: a rational or decimal, in meters.
    #[uniffi(default)]
    pub altitude: Option<String>,
    /// `exif:GPSAltitudeRef`: `0`, `1`, `2` or `3`.
    #[uniffi(default)]
    pub altitude_ref: Option<String>,
    /// `exif:GPSMapDatum`.
    #[uniffi(default)]
    pub map_datum: Option<String>,
    /// `exif:GPSHPositioningError`: a rational or decimal, in meters.
    #[uniffi(default)]
    pub h_positioning_error: Option<String>,
    /// `exif:GPSDOP`: a rational or decimal.
    #[uniffi(default)]
    pub dop: Option<String>,
    /// `exif:GPSTimeStamp`: `YYYY-MM-DDThh:mm:ss[.s]` and a `Z` or `±hh:mm` zone.
    #[uniffi(default)]
    pub time_stamp: Option<String>,
    /// `exif:GPSImgDirection`: a rational or decimal, in degrees.
    #[uniffi(default)]
    pub img_direction: Option<String>,
    /// `exif:GPSImgDirectionRef`: `T` or `M`.
    #[uniffi(default)]
    pub img_direction_ref: Option<String>,
    /// `exif:GPSSpeed`: a rational or decimal.
    #[uniffi(default)]
    pub speed: Option<String>,
    /// `exif:GPSSpeedRef`: `K`, `M` or `N`.
    #[uniffi(default)]
    pub speed_ref: Option<String>,
    /// `exif:GPSTrack`: a rational or decimal, in degrees.
    #[uniffi(default)]
    pub track: Option<String>,
    /// `exif:GPSTrackRef`: `T` or `M`.
    #[uniffi(default)]
    pub track_ref: Option<String>,
}

impl From<XmpGps> for gc::exif::XmpGps {
    fn from(gps: XmpGps) -> Self {
        let mut out = Self::default();
        out.latitude = gps.latitude;
        out.longitude = gps.longitude;
        out.altitude = gps.altitude;
        out.altitude_ref = gps.altitude_ref;
        out.map_datum = gps.map_datum;
        out.h_positioning_error = gps.h_positioning_error;
        out.dop = gps.dop;
        out.time_stamp = gps.time_stamp;
        out.img_direction = gps.img_direction;
        out.img_direction_ref = gps.img_direction_ref;
        out.speed = gps.speed;
        out.speed_ref = gps.speed_ref;
        out.track = gps.track;
        out.track_ref = gps.track_ref;
        out
    }
}

/// Which north a bearing is measured from — mirror of [`gc::exif::NorthReference`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NorthReference {
    /// True north (`T`).
    True,
    /// Magnetic north (`M`).
    Magnetic,
}

impl From<gc::exif::NorthReference> for NorthReference {
    fn from(reference: gc::exif::NorthReference) -> Self {
        match reference {
            gc::exif::NorthReference::True => Self::True,
            gc::exif::NorthReference::Magnetic => Self::Magnetic,
        }
    }
}

/// A bearing with its north — mirror of [`gc::exif::NorthBearing`]. Output only: no
/// export takes one, so it has no `_new` or `_validate`.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct NorthBearing {
    /// Degrees clockwise from north, in `[0, 360)`.
    pub bearing_deg: f64,
    /// The north this bearing is measured from; absent when the source named none.
    pub reference: Option<NorthReference>,
}

impl From<gc::exif::NorthBearing> for NorthBearing {
    fn from(bearing: gc::exif::NorthBearing) -> Self {
        Self {
            bearing_deg: bearing.degrees(),
            reference: bearing.reference().map(Into::into),
        }
    }
}

/// What to do with a position of exactly 0°, 0° — mirror of
/// [`gc::exif::NullIslandPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NullIslandPolicy {
    /// Treat 0°, 0° as any other position.
    Accept,
    /// Reject 0°, 0° as `GeoError::Parse` of kind `NoFix`.
    Reject,
}

/// Options for [`parse_exif_with`] and [`parse_xmp_with`] — mirror of
/// [`gc::exif::ExifOptions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ExifOptions {
    /// What to do with a position of exactly 0°, 0°.
    pub null_island: NullIslandPolicy,
}

impl From<ExifOptions> for gc::exif::ExifOptions {
    fn from(options: ExifOptions) -> Self {
        Self::new().with_null_island(match options.null_island {
            NullIslandPolicy::Accept => gc::exif::NullIslandPolicy::Accept,
            NullIslandPolicy::Reject => gc::exif::NullIslandPolicy::Reject,
        })
    }
}

/// A photo's capture position, direction and motion — mirror of
/// [`gc::exif::CaptureGeo`]. Output only: no export takes one.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CaptureGeo {
    /// The position, height, accuracy, UTC timestamp and provenance. A DOP gives no
    /// accuracy, since it has no meters to report; it stays in `dop`.
    pub fix: Fix,
    /// The direction the camera faced (`GPSImgDirection`).
    pub direction: Option<NorthBearing>,
    /// Ground speed in meters per second; absent when `GPSSpeedRef` is, as the unit
    /// is then unknown.
    pub speed_mps: Option<f64>,
    /// The direction of travel (`GPSTrack`).
    pub track: Option<NorthBearing>,
    /// The dilution of precision (`GPSDOP`), unitless.
    pub dop: Option<f64>,
}

impl From<gc::exif::CaptureGeo> for CaptureGeo {
    fn from(capture: gc::exif::CaptureGeo) -> Self {
        Self {
            fix: capture.fix.into(),
            direction: capture.direction.map(Into::into),
            speed_mps: capture.speed_mps,
            track: capture.track.map(Into::into),
            dop: capture.dop,
        }
    }
}

/// Read GPS IFD tag values, accepting a position of 0°, 0° — the FFI form of the core
/// `exif::from_exif`.
///
/// # Errors
/// As [`parse_exif_with`].
#[uniffi::export]
pub fn parse_exif(gps: ExifGps) -> Result<CaptureGeo, GeoError> {
    Ok(gc::exif::from_exif(&gps.into())?.into())
}

/// Read GPS IFD tag values under `options` — the FFI form of the core
/// `exif::from_exif_with`.
///
/// # Errors
/// `GeoError::Parse` of kind `NoFix` when latitude or longitude is absent, the block
/// is all `0/0`, or the position is 0°, 0° under `NullIslandPolicy::Reject`;
/// `GeoError::InvalidValue` naming the tag for a zero denominator, a negative value,
/// minutes or seconds of 60 or more, an hour of 24 or more, a bearing outside
/// `[0, 360)`, or a bad Ref; `GeoError::OutOfRange` beyond ±90° / ±180°.
#[uniffi::export]
pub fn parse_exif_with(gps: ExifGps, options: ExifOptions) -> Result<CaptureGeo, GeoError> {
    Ok(gc::exif::from_exif_with(&gps.into(), options.into())?.into())
}

/// Read XMP `exif:GPS*` properties, accepting a position of 0°, 0° — the FFI form of
/// the core `exif::from_xmp`.
///
/// # Errors
/// As [`parse_xmp_with`].
#[uniffi::export]
pub fn parse_xmp(gps: XmpGps) -> Result<CaptureGeo, GeoError> {
    Ok(gc::exif::from_xmp(&gps.into())?.into())
}

/// Read XMP `exif:GPS*` properties under `options` — the FFI form of the core
/// `exif::from_xmp_with`.
///
/// # Errors
/// As [`parse_exif_with`], and `GeoError::Parse` of kind `Syntax` for a coordinate or
/// number that does not read in its XMP form.
#[uniffi::export]
pub fn parse_xmp_with(gps: XmpGps, options: ExifOptions) -> Result<CaptureGeo, GeoError> {
    Ok(gc::exif::from_xmp_with(&gps.into(), options.into())?.into())
}
