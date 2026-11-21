//! EXIF / XMP GPS tag values to a validated [`Fix`] (feature `exif`).
//!
//! This module starts from tag values a reader already extracted: it never parses
//! JPEG, TIFF or HEIF bytes. Hand [`CaptureGeo::from_exif`] the GPS IFD's raw
//! rationals in an [`ExifGps`], or [`CaptureGeo::from_xmp`] the `exif:GPS*` property
//! strings in an [`XmpGps`], and get a [`CaptureGeo`]: the position and its metadata
//! as a [`Fix`], plus the capture direction and motion, which a `Fix` deliberately
//! leaves out.
//!
//! Every item is reached flat at `exif::NAME`.
//!
//! ```
//! use geocoordinates::{Error, Height};
//! use geocoordinates::exif::{CaptureGeo, ExifGps};
//!
//! let mut gps = ExifGps::default();
//! gps.latitude = Some([(51_u32, 1_u32).into(), (45_u32, 1_u32).into(), (4467_u32, 100_u32).into()]);
//! gps.latitude_ref = Some("N".to_string());
//! gps.longitude = Some([(0_u32, 1_u32).into(), (19_u32, 1_u32).into(), (441_u32, 100_u32).into()]);
//! gps.longitude_ref = Some("W".to_string());
//! gps.altitude = Some((27704_u32, 249_u32).into());
//! gps.altitude_ref = Some(0);
//!
//! let capture = CaptureGeo::from_exif(&gps)?;
//! assert!((capture.fix.coord.lat() - 51.762_408).abs() < 1e-6);
//! assert!(capture.fix.coord.lon() < 0.0);
//! assert!(matches!(capture.fix.coord.height(), Some(Height::Orthometric(_))));
//!
//! // A hemisphere letter is never guessed.
//! gps.latitude_ref = Some("Q".to_string());
//! assert!(matches!(
//!     CaptureGeo::from_exif(&gps),
//!     Err(Error::InvalidValue { field: "latitude_ref", .. })
//! ));
//! # Ok::<(), geocoordinates::Error>(())
//! ```

mod build;
mod xmp;

#[cfg(test)]
mod device_tests;
#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod stamp_tests;
#[cfg(test)]
mod tag_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod xmp_tests;

use crate::fix::Fix;
use crate::{Error, ParseErrorKind, Result};

pub use self::xmp::XmpGps;

/// One EXIF rational exactly as stored: `RATIONAL` (`u32 / u32`) or `SRATIONAL`
/// (`i32 / i32`).
///
/// No validity is implied: a zero denominator or a negative value can be held
/// here, and is rejected by [`CaptureGeo::from_exif`] with [`Error::InvalidValue`].
///
/// ```
/// use geocoordinates::exif::ExifRational;
///
/// assert_eq!(ExifRational::from((441_u32, 100_u32)), ExifRational::Unsigned(441, 100));
/// assert_eq!(ExifRational::from((-5_i32, 1_i32)), ExifRational::Signed(-5, 1));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExifRational {
    /// An EXIF `RATIONAL`: numerator, denominator.
    Unsigned(u32, u32),
    /// An EXIF `SRATIONAL`: numerator, denominator.
    Signed(i32, i32),
}

/// Thin sugar over [`ExifRational::Unsigned`].
impl From<(u32, u32)> for ExifRational {
    fn from((numerator, denominator): (u32, u32)) -> Self {
        Self::Unsigned(numerator, denominator)
    }
}

/// Thin sugar over [`ExifRational::Signed`].
impl From<(i32, i32)> for ExifRational {
    fn from((numerator, denominator): (i32, i32)) -> Self {
        Self::Signed(numerator, denominator)
    }
}

impl ExifRational {
    /// The stored `0/0` some devices write in a GPS block that has no fix.
    const fn is_zero_over_zero(self) -> bool {
        matches!(self, Self::Unsigned(0, 0) | Self::Signed(0, 0))
    }

    /// The value as `f64`, rejecting a zero denominator or a negative value.
    fn value(self, field: &'static str) -> Result<f64> {
        let (numerator, denominator) = match self {
            Self::Unsigned(n, d) => (f64::from(n), f64::from(d)),
            Self::Signed(n, d) => (f64::from(n), f64::from(d)),
        };
        if denominator == 0.0 {
            return Err(Error::InvalidValue {
                field,
                detail: format!("zero denominator in {numerator}/0"),
            });
        }
        let value = numerator / denominator;
        if value < 0.0 {
            return Err(Error::InvalidValue {
                field,
                detail: format!("must be nonnegative, got {numerator}/{denominator}"),
            });
        }
        Ok(value)
    }
}

/// The raw values of an EXIF GPS IFD, as a reader extracted them. Every field is
/// optional. Strings may carry EXIF's trailing NUL padding.
///
/// `#[non_exhaustive]`, so a tag can be added without a breaking release: outside
/// this crate an `ExifGps` is built with [`Default`] and its public fields are then
/// set one by one.
///
/// ```
/// use geocoordinates::exif::ExifGps;
///
/// let mut gps = ExifGps::default();
/// gps.map_datum = Some("WGS-84\0".to_string());
/// assert!(gps.latitude.is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct ExifGps {
    /// `GPSLatitude`: degrees, minutes, seconds. Any may be fractional.
    pub latitude: Option<[ExifRational; 3]>,
    /// `GPSLatitudeRef`: `N` or `S`.
    pub latitude_ref: Option<String>,
    /// `GPSLongitude`: degrees, minutes, seconds. Any may be fractional.
    pub longitude: Option<[ExifRational; 3]>,
    /// `GPSLongitudeRef`: `E` or `W`.
    pub longitude_ref: Option<String>,
    /// `GPSAltitude`, a nonnegative magnitude in meters.
    pub altitude: Option<ExifRational>,
    /// `GPSAltitudeRef`: 0 above sea level, 1 below, 2 above the ellipsoid, 3 below it.
    pub altitude_ref: Option<u8>,
    /// `GPSMapDatum`.
    pub map_datum: Option<String>,
    /// `GPSHPositioningError`, in meters.
    pub h_positioning_error: Option<ExifRational>,
    /// `GPSDOP`.
    pub dop: Option<ExifRational>,
    /// `GPSDateStamp`: `YYYY:MM:DD`.
    pub date_stamp: Option<String>,
    /// `GPSTimeStamp`: UTC hours, minutes, seconds. Any may be fractional.
    pub time_stamp: Option<[ExifRational; 3]>,
    /// `GPSImgDirection`, in degrees.
    pub img_direction: Option<ExifRational>,
    /// `GPSImgDirectionRef`: `T` or `M`.
    pub img_direction_ref: Option<String>,
    /// `GPSSpeed`, in the unit of [`speed_ref`](Self::speed_ref).
    pub speed: Option<ExifRational>,
    /// `GPSSpeedRef`: `K` (km/h), `M` (mph) or `N` (knots).
    pub speed_ref: Option<String>,
    /// `GPSTrack`, in degrees.
    pub track: Option<ExifRational>,
    /// `GPSTrackRef`: `T` or `M`.
    pub track_ref: Option<String>,
}

/// Which north a bearing is measured from.
///
/// ```
/// use geocoordinates::exif::{NorthBearing, NorthReference};
///
/// let bearing = NorthBearing::new(90.0, Some(NorthReference::Magnetic))?;
/// assert_eq!(bearing.reference(), Some(NorthReference::Magnetic));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NorthReference {
    /// True north (`T`).
    True,
    /// Magnetic north (`M`).
    Magnetic,
}

/// A bearing in `[0, 360)` degrees clockwise from north, with the north its source
/// named, if any.
///
/// Valid by construction: [`new`](Self::new) (and, under the `serde` feature,
/// deserialization) rejects anything outside `[0, 360)`.
///
/// ```
/// use geocoordinates::exif::NorthBearing;
///
/// let bearing = NorthBearing::new(23.25, None)?;
/// assert_eq!(bearing.degrees(), 23.25);
/// assert_eq!(bearing.reference(), None);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "NorthBearingRepr"))]
pub struct NorthBearing {
    degrees: f64,
    reference: Option<NorthReference>,
}

/// The unchecked wire shape of a [`NorthBearing`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct NorthBearingRepr {
    degrees: f64,
    reference: Option<NorthReference>,
}

#[cfg(feature = "serde")]
impl TryFrom<NorthBearingRepr> for NorthBearing {
    type Error = Error;

    fn try_from(repr: NorthBearingRepr) -> Result<Self> {
        Self::new(repr.degrees, repr.reference)
    }
}

impl NorthBearing {
    /// Build a bearing; `reference` is `None` when the source does not name a north.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] (field `degrees`) when `degrees` is NaN or
    /// outside `[0, 360)`.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::exif::{NorthBearing, NorthReference};
    ///
    /// assert!(NorthBearing::new(359.5, Some(NorthReference::True)).is_ok());
    /// assert!(matches!(
    ///     NorthBearing::new(360.0, None),
    ///     Err(Error::InvalidValue { field: "degrees", .. })
    /// ));
    /// ```
    pub fn new(degrees: f64, reference: Option<NorthReference>) -> Result<Self> {
        Self::checked(degrees, reference, "degrees")
    }

    /// [`new`](Self::new), naming the tag that failed.
    pub(crate) fn checked(
        degrees: f64,
        reference: Option<NorthReference>,
        field: &'static str,
    ) -> Result<Self> {
        if (0.0..360.0).contains(&degrees) {
            Ok(Self { degrees, reference })
        } else {
            Err(Error::InvalidValue {
                field,
                detail: format!("must be in [0, 360) degrees, got {degrees}"),
            })
        }
    }

    /// Degrees clockwise from north, in `[0, 360)`.
    ///
    /// ```
    /// use geocoordinates::exif::NorthBearing;
    ///
    /// assert_eq!(NorthBearing::new(45.0, None)?.degrees(), 45.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn degrees(self) -> f64 {
        self.degrees
    }

    /// The north this bearing is measured from; `None` when the source named none.
    ///
    /// ```
    /// use geocoordinates::exif::{NorthBearing, NorthReference};
    ///
    /// let bearing = NorthBearing::new(45.0, Some(NorthReference::True))?;
    /// assert_eq!(bearing.reference(), Some(NorthReference::True));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn reference(self) -> Option<NorthReference> {
        self.reference
    }
}

/// What to do with a position of exactly 0° latitude, 0° longitude, which is a real
/// place but also what some software writes for "no fix".
///
/// ```
/// use geocoordinates::exif::NullIslandPolicy;
///
/// assert_eq!(NullIslandPolicy::default(), NullIslandPolicy::Accept);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NullIslandPolicy {
    /// Treat 0°, 0° as any other position.
    #[default]
    Accept,
    /// Reject 0°, 0° with [`ParseErrorKind::NoFix`].
    Reject,
}

/// What to do with an invalid *auxiliary* tag: anything but the position, which is
/// `GPSLatitude`, `GPSLongitude` and their Refs.
///
/// The auxiliary tags are `GPSAltitude` / `GPSAltitudeRef`, `GPSHPositioningError`,
/// `GPSDOP`, `GPSTimeStamp`, `GPSImgDirection` / `GPSImgDirectionRef`, `GPSSpeed` /
/// `GPSSpeedRef` and `GPSTrack` / `GPSTrackRef`, and their XMP `exif:GPS*` forms. A
/// position error fails the conversion under either policy.
///
/// ```
/// use geocoordinates::exif::AuxiliaryTagPolicy;
///
/// assert_eq!(AuxiliaryTagPolicy::default(), AuxiliaryTagPolicy::Reject);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AuxiliaryTagPolicy {
    /// Fail the whole conversion with the tag's error, so a malformed file is seen.
    #[default]
    Reject,
    /// Omit the invalid tag, and the other half of its value / Ref pair, from the
    /// result, keep the position, and record why in one `fix.source.notes` entry.
    Drop,
}

/// Options for [`CaptureGeo::from_exif_with`] and [`CaptureGeo::from_xmp_with`].
///
/// `#[non_exhaustive]`: build one from [`Default`] and the `const` `with_*` builders.
///
/// ```
/// use geocoordinates::exif::{AuxiliaryTagPolicy, ExifOptions, NullIslandPolicy};
///
/// let options = ExifOptions::default()
///     .with_null_island(NullIslandPolicy::Reject)
///     .with_invalid_auxiliary(AuxiliaryTagPolicy::Drop);
/// assert_eq!(options.null_island, NullIslandPolicy::Reject);
/// assert_eq!(options.invalid_auxiliary, AuxiliaryTagPolicy::Drop);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct ExifOptions {
    /// What to do with a position of exactly 0°, 0°.
    pub null_island: NullIslandPolicy,
    /// What to do with an invalid auxiliary tag.
    pub invalid_auxiliary: AuxiliaryTagPolicy,
}

impl ExifOptions {
    /// Set [`null_island`](Self::null_island).
    ///
    /// ```
    /// use geocoordinates::exif::{ExifOptions, NullIslandPolicy};
    ///
    /// const STRICT: ExifOptions = ExifOptions::new().with_null_island(NullIslandPolicy::Reject);
    /// assert_eq!(STRICT.null_island, NullIslandPolicy::Reject);
    /// ```
    #[must_use]
    pub const fn with_null_island(mut self, null_island: NullIslandPolicy) -> Self {
        self.null_island = null_island;
        self
    }

    /// Set [`invalid_auxiliary`](Self::invalid_auxiliary).
    ///
    /// ```
    /// use geocoordinates::exif::{AuxiliaryTagPolicy, ExifOptions};
    ///
    /// const LENIENT: ExifOptions =
    ///     ExifOptions::new().with_invalid_auxiliary(AuxiliaryTagPolicy::Drop);
    /// assert_eq!(LENIENT.invalid_auxiliary, AuxiliaryTagPolicy::Drop);
    /// ```
    #[must_use]
    pub const fn with_invalid_auxiliary(mut self, invalid_auxiliary: AuxiliaryTagPolicy) -> Self {
        self.invalid_auxiliary = invalid_auxiliary;
        self
    }

    /// The default options, usable in a `const`.
    ///
    /// ```
    /// use geocoordinates::exif::ExifOptions;
    ///
    /// assert_eq!(ExifOptions::new(), ExifOptions::default());
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            null_island: NullIslandPolicy::Accept,
            invalid_auxiliary: AuxiliaryTagPolicy::Reject,
        }
    }
}

/// A photo's capture position and metadata, with its direction and motion.
///
/// `#[non_exhaustive]`, so a field can be added without a breaking release; it is
/// produced by [`CaptureGeo::from_exif`] and [`CaptureGeo::from_xmp`].
///
/// ```
/// use geocoordinates::exif::{CaptureGeo, XmpGps};
///
/// let mut gps = XmpGps::default();
/// gps.latitude = Some("51,45.0N".to_string());
/// gps.longitude = Some("0,19.5W".to_string());
/// gps.track = Some("90/1".to_string());
/// let capture = CaptureGeo::from_xmp(&gps)?;
/// assert_eq!(capture.track.map(|t| t.degrees()), Some(90.0));
/// assert_eq!(capture.direction, None);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct CaptureGeo {
    /// The position, height, accuracy, UTC timestamp and provenance.
    ///
    /// `fix.accuracy` is a [`Bound`](crate::fix::AccuracyKind::Bound) from
    /// `GPSHPositioningError`. A DOP gives no accuracy: it has no meters to report
    /// and none are invented, so it stays in [`dop`](Self::dop).
    pub fix: Fix,
    /// The direction the camera faced (`GPSImgDirection`).
    pub direction: Option<NorthBearing>,
    /// Ground speed in meters per second (`GPSSpeed`). `None` when `GPSSpeedRef` is
    /// absent, since the unit is then unknown; `fix.source.notes` says so.
    pub speed_mps: Option<f64>,
    /// The direction of travel (`GPSTrack`).
    pub track: Option<NorthBearing>,
    /// The dilution of precision (`GPSDOP`), unitless.
    pub dop: Option<f64>,
}

impl CaptureGeo {
    /// Convert GPS IFD tag values with the default [`ExifOptions`].
    ///
    /// See [`from_exif_with`](Self::from_exif_with) for the rules and errors.
    ///
    /// # Errors
    /// As [`from_exif_with`](Self::from_exif_with).
    ///
    /// ```
    /// use geocoordinates::exif::{CaptureGeo, ExifGps};
    /// use geocoordinates::{Error, ParseErrorKind};
    ///
    /// // A photo with no GPS block has no fix.
    /// assert!(matches!(
    ///     CaptureGeo::from_exif(&ExifGps::default()),
    ///     Err(Error::Parse { kind: ParseErrorKind::NoFix, .. })
    /// ));
    /// ```
    pub fn from_exif(gps: &ExifGps) -> Result<Self> {
        Self::from_exif_with(gps, ExifOptions::default())
    }

    /// Convert GPS IFD tag values to a [`CaptureGeo`].
    ///
    /// - **Position:** the hemisphere comes only from the Ref tag (`N`/`S`/`E`/`W`,
    ///   case-insensitive, trimmed).
    /// - **Height:** `GPSAltitudeRef` 0 and 1 give [`Height::Orthometric`], 2 and 3
    ///   give [`Height::Ellipsoidal`]; an absent Ref reads as 0, the Exif default.
    /// - **Datum:** `GPSMapDatum` spellings of WGS-84, Tokyo, GCJ-02 and BD-09 set the
    ///   CRS. Empty, absent or unknown means WGS-84 at lowered confidence with a note,
    ///   and inside China's bounding box also flags
    ///   [`DatumAmbiguity::PossiblyGcj02`](crate::fix::DatumAmbiguity::PossiblyGcj02).
    /// - **Timestamp:** needs a date and a time. A partial or unreadable date gives no
    ///   timestamp rather than a guessed one, and an unreadable date adds a note.
    /// - **Auxiliary tags:** an invalid one fails the conversion under
    ///   [`AuxiliaryTagPolicy::Reject`], the default. Under
    ///   [`AuxiliaryTagPolicy::Drop`] it is left out of the result together with the
    ///   other half of its value / Ref pair, one `fix.source.notes` entry names the
    ///   tag and the error, and the position is kept.
    ///
    /// [`Height::Orthometric`]: crate::Height::Orthometric
    /// [`Height::Ellipsoidal`]: crate::Height::Ellipsoidal
    ///
    /// # Errors
    /// - [`Error::Parse`] with [`ParseErrorKind::NoFix`]: latitude or longitude is
    ///   absent, the block is all `0/0` placeholders, or the position is 0°, 0° under
    ///   [`NullIslandPolicy::Reject`].
    /// - [`Error::InvalidValue`] naming the field: a zero denominator or negative
    ///   value, minutes or seconds of 60 or more, a time stamp's hour of 24 or more or
    ///   seconds of 60 (so a UTC leap second is rejected), a bearing outside `[0, 360)`
    ///   (so 360 is rejected), an absent or unrecognised latitude / longitude Ref, or
    ///   an unrecognised other Ref, with or without the tag it qualifies. Under
    ///   [`AuxiliaryTagPolicy::Drop`], only a latitude or longitude field.
    /// - [`Error::OutOfRange`]: the position is beyond ±90° / ±180°.
    ///
    /// Every position error is checked before any auxiliary one, so when both are
    /// present the position error is the one returned.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::exif::{
    ///     AuxiliaryTagPolicy, CaptureGeo, ExifGps, ExifOptions, NullIslandPolicy,
    /// };
    ///
    /// let mut gps = ExifGps::default();
    /// let zero = (0_u32, 1_u32).into();
    /// gps.latitude = Some([zero, zero, zero]);
    /// gps.latitude_ref = Some("N".to_string());
    /// gps.longitude = Some([zero, zero, zero]);
    /// gps.longitude_ref = Some("E".to_string());
    ///
    /// assert!(CaptureGeo::from_exif_with(&gps, ExifOptions::default()).is_ok());
    /// let strict = ExifOptions::default().with_null_island(NullIslandPolicy::Reject);
    /// assert!(CaptureGeo::from_exif_with(&gps, strict).is_err());
    ///
    /// gps.longitude = Some([zero, (60_u32, 1_u32).into(), zero]);
    /// assert!(matches!(
    ///     CaptureGeo::from_exif_with(&gps, ExifOptions::default()),
    ///     Err(Error::InvalidValue { field: "longitude", .. })
    /// ));
    ///
    /// // An unset `0/0` altitude fails the default, and is dropped under `Drop`.
    /// gps.longitude = Some([zero, zero, zero]);
    /// gps.altitude = Some((0_u32, 0_u32).into());
    /// assert!(matches!(
    ///     CaptureGeo::from_exif_with(&gps, ExifOptions::default()),
    ///     Err(Error::InvalidValue { field: "altitude", .. })
    /// ));
    /// let lenient = ExifOptions::default().with_invalid_auxiliary(AuxiliaryTagPolicy::Drop);
    /// let capture = CaptureGeo::from_exif_with(&gps, lenient)?;
    /// assert_eq!(capture.fix.coord.height(), None);
    /// assert!(capture.fix.source.is_some_and(|s| s.notes.iter().any(|n| n.contains("GPSAltitude"))));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn from_exif_with(gps: &ExifGps, options: ExifOptions) -> Result<Self> {
        let (Some(latitude), Some(longitude)) = (gps.latitude, gps.longitude) else {
            return Err(no_fix("GPSLatitude and GPSLongitude are both required"));
        };
        if latitude
            .iter()
            .chain(&longitude)
            .all(|r| r.is_zero_over_zero())
        {
            return Err(no_fix("GPSLatitude and GPSLongitude are all 0/0"));
        }
        let tags = build::Tags {
            raw: format!(
                "GPSLatitude={} {}; GPSLongitude={} {}",
                render(latitude),
                gps.latitude_ref.as_deref().map_or("", build::clean),
                render(longitude),
                gps.longitude_ref.as_deref().map_or("", build::clean),
            ),
            latitude: triple(latitude, "latitude")?,
            latitude_ref: gps.latitude_ref.as_deref(),
            longitude: triple(longitude, "longitude")?,
            longitude_ref: gps.longitude_ref.as_deref(),
            altitude: single(gps.altitude, "altitude"),
            altitude_ref: Ok(gps.altitude_ref),
            map_datum: gps.map_datum.as_deref(),
            h_positioning_error: single(gps.h_positioning_error, "h_positioning_error"),
            dop: single(gps.dop, "dop"),
            stamp: gps
                .time_stamp
                .map(|time| triple(time, "time_stamp"))
                .transpose()
                .and_then(|time| build::Stamp::new(gps.date_stamp.as_deref(), time, 0)),
            img_direction: single(gps.img_direction, "img_direction"),
            img_direction_ref: gps.img_direction_ref.as_deref(),
            speed: single(gps.speed, "speed"),
            speed_ref: gps.speed_ref.as_deref(),
            track: single(gps.track, "track"),
            track_ref: gps.track_ref.as_deref(),
        };
        tags.build(options)
    }
}

/// A [`ParseErrorKind::NoFix`] error.
fn no_fix(detail: &str) -> Error {
    Error::Parse {
        kind: ParseErrorKind::NoFix,
        detail: detail.to_string(),
    }
}

fn triple(values: [ExifRational; 3], field: &'static str) -> Result<[f64; 3]> {
    Ok([
        values[0].value(field)?,
        values[1].value(field)?,
        values[2].value(field)?,
    ])
}

/// An optional rational's value; an error is left for [`build::Tags::build`] to
/// apply the [`AuxiliaryTagPolicy`] to.
fn single(value: Option<ExifRational>, field: &'static str) -> Result<Option<f64>> {
    value.map(|r| r.value(field)).transpose()
}

/// Render three rationals as stored, for [`RawSource::raw`](crate::fix::RawSource::raw).
fn render(values: [ExifRational; 3]) -> String {
    let one = |r: &ExifRational| match *r {
        ExifRational::Unsigned(n, d) => format!("{n}/{d}"),
        ExifRational::Signed(n, d) => format!("{n}/{d}"),
    };
    format!(
        "{},{},{}",
        one(&values[0]),
        one(&values[1]),
        one(&values[2])
    )
}
