//! Angle encodings: degrees-minutes-seconds (DMS) and degrees-decimal-minutes
//! (DDM), plus angle-normalization helpers (longitude wrap, latitude clamp,
//! degree normalization).
//!
//! DDM is what NMEA and marine/aviation use and is frequently forgotten.
//! Signed decimal degrees are plain `f64`; the axis they describe is always
//! stated at the conversion ([`Dms::from_degrees`], [`Ddm::from_degrees`]).
//!
//! [`Dms`] and [`Ddm`] are **valid by construction**: their fields are private,
//! and every constructor (including serde deserialization) rejects a component
//! outside its documented range. Conversions between them and to decimal
//! degrees are therefore infallible, and exact up to `f64` rounding (pure
//! arithmetic, no approximation).

use crate::error::{Error, Result};

/// North/South or East/West hemisphere sign.
///
/// ```
/// use geocoordinates::angle::{Axis, Dms, Hemisphere};
///
/// assert_eq!(Dms::from_degrees(-33.9, Axis::Latitude)?.hemisphere(), Hemisphere::South);
/// assert_eq!(Dms::from_degrees(-33.9, Axis::Longitude)?.hemisphere(), Hemisphere::West);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hemisphere {
    /// North (latitude, positive).
    North,
    /// South (latitude, negative).
    South,
    /// East (longitude, positive).
    East,
    /// West (longitude, negative).
    West,
}

/// Degrees, minutes, seconds, e.g. `40° 42′ 46″ N`.
///
/// Valid by construction — build one with [`Dms::new`] or
/// [`Dms::from_degrees`]. Minutes and seconds lie in `[0, 60)`, and the value
/// is at most 90° for a North/South hemisphere or 180° for East/West.
///
/// ```
/// use geocoordinates::angle::{Dms, Hemisphere};
///
/// let lat = Dms::new(40, 42, 46.0, Hemisphere::North)?;
/// assert!((lat.to_degrees() - 40.712_777_8).abs() < 1e-7);
/// // Out-of-range components never become a `Dms`.
/// assert!(Dms::new(91, 0, 0.0, Hemisphere::North).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "DmsRecord"))]
pub struct Dms {
    degrees: u16,
    minutes: u8,
    seconds: f64,
    hemisphere: Hemisphere,
}

/// Degrees and decimal minutes, e.g. `40° 42.766′ N` (NMEA/marine).
///
/// Valid by construction — build one with [`Ddm::new`] or
/// [`Ddm::from_degrees`]. Minutes lie in `[0, 60)`, and the value is at most
/// 90° for a North/South hemisphere or 180° for East/West.
///
/// ```
/// use geocoordinates::angle::{Ddm, Hemisphere};
///
/// let lon = Ddm::new(74, 0.36, Hemisphere::West)?;
/// assert!((lon.to_degrees() + 74.006).abs() < 1e-12);
/// assert!(Ddm::new(181, 0.0, Hemisphere::East).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "DdmRecord"))]
pub struct Ddm {
    degrees: u16,
    minutes: f64,
    hemisphere: Hemisphere,
}

/// Unvalidated serde shape of [`Dms`]; deserialization goes through
/// [`Dms::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct DmsRecord {
    degrees: u16,
    minutes: u8,
    seconds: f64,
    hemisphere: Hemisphere,
}

#[cfg(feature = "serde")]
impl TryFrom<DmsRecord> for Dms {
    type Error = Error;

    fn try_from(r: DmsRecord) -> Result<Self> {
        Dms::new(r.degrees, r.minutes, r.seconds, r.hemisphere)
    }
}

/// Unvalidated serde shape of [`Ddm`]; deserialization goes through
/// [`Ddm::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct DdmRecord {
    degrees: u16,
    minutes: f64,
    hemisphere: Hemisphere,
}

#[cfg(feature = "serde")]
impl TryFrom<DdmRecord> for Ddm {
    type Error = Error;

    fn try_from(r: DdmRecord) -> Result<Self> {
        Ddm::new(r.degrees, r.minutes, r.hemisphere)
    }
}

impl Hemisphere {
    /// The numeric sign this hemisphere applies: `-1.0` for the negative
    /// hemispheres (South / West), `+1.0` for the positive ones (North / East).
    ///
    /// ```
    /// use geocoordinates::angle::Hemisphere;
    ///
    /// assert_eq!(Hemisphere::East.sign(), 1.0);
    /// assert_eq!(Hemisphere::South.sign(), -1.0);
    /// ```
    #[must_use]
    pub fn sign(self) -> f64 {
        match self {
            Hemisphere::North | Hemisphere::East => 1.0,
            Hemisphere::South | Hemisphere::West => -1.0,
        }
    }
}

/// The hemisphere for a signed value on the given axis.
///
/// Treats `-0.0` as non-negative (North / East), so a zeroed component never
/// reads as South / West.
fn hemisphere_for(axis: Axis, value: f64) -> Hemisphere {
    match (axis, value >= 0.0) {
        (Axis::Latitude, true) => Hemisphere::North,
        (Axis::Latitude, false) => Hemisphere::South,
        (Axis::Longitude, true) => Hemisphere::East,
        (Axis::Longitude, false) => Hemisphere::West,
    }
}

impl Dms {
    /// Build a DMS angle from its components.
    ///
    /// The hemisphere selects the axis: North/South allows at most 90°,
    /// East/West at most 180°.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] naming `degrees`, `minutes`, or
    /// `seconds` when that component is out of range: minutes or seconds
    /// outside `[0, 60)` or non-finite, or a value beyond the axis maximum.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::angle::{Dms, Hemisphere};
    ///
    /// // 180° is the longitude maximum, so it carries no minutes or seconds.
    /// assert!(Dms::new(180, 0, 0.0, Hemisphere::West).is_ok());
    /// assert!(matches!(
    ///     Dms::new(180, 0, 0.5, Hemisphere::West),
    ///     Err(Error::InvalidValue { field: "degrees", .. })
    /// ));
    /// assert!(matches!(
    ///     Dms::new(12, 30, 60.0, Hemisphere::North),
    ///     Err(Error::InvalidValue { field: "seconds", .. })
    /// ));
    /// ```
    pub fn new(degrees: u16, minutes: u8, seconds: f64, hemisphere: Hemisphere) -> Result<Self> {
        validate_components(
            degrees,
            f64::from(minutes),
            seconds,
            axis_for_hemisphere(hemisphere),
        )?;
        Ok(Dms {
            degrees,
            minutes,
            seconds,
            hemisphere,
        })
    }

    /// Decompose signed decimal degrees on the given axis (latitude or
    /// longitude selects the hemisphere letters).
    ///
    /// `seconds` is kept full-precision and is **not** pre-rounded, so a round
    /// trip back through [`Dms::to_degrees`] is exact up to `f64` rounding.
    /// Rounding — and the 60″ carry it can imply — is the formatter's
    /// responsibility (see [`crate::format`]). `-0.0` reads as North / East.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] (field `"decimal degrees"`) for a
    /// non-finite value or one beyond ±90° (latitude) / ±180° (longitude).
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::angle::{Axis, Dms, Hemisphere};
    ///
    /// let dms = Dms::from_degrees(-74.006, Axis::Longitude)?;
    /// assert_eq!((dms.degrees(), dms.minutes(), dms.hemisphere()), (74, 0, Hemisphere::West));
    /// assert!((dms.seconds() - 21.6).abs() < 1e-9);
    /// // 120 is a valid longitude but not a valid latitude.
    /// assert!(matches!(
    ///     Dms::from_degrees(120.0, Axis::Latitude),
    ///     Err(Error::InvalidValue { field: "decimal degrees", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_degrees(value: f64, axis: Axis) -> Result<Self> {
        validate_dd(value, axis)?;
        let magnitude = value.abs();
        let degrees = magnitude.trunc();
        let rem_minutes = (magnitude - degrees) * 60.0;
        let minutes = rem_minutes.trunc();
        let seconds = (rem_minutes - minutes) * 60.0;
        Dms::new(
            degrees as u16,
            minutes as u8,
            seconds,
            hemisphere_for(axis, value),
        )
    }

    /// Whole degrees (non-negative; the sign is carried by
    /// [`hemisphere`](Self::hemisphere)).
    ///
    /// ```
    /// use geocoordinates::angle::{Dms, Hemisphere};
    ///
    /// assert_eq!(Dms::new(33, 51, 54.0, Hemisphere::South)?.degrees(), 33);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn degrees(&self) -> u16 {
        self.degrees
    }

    /// Whole minutes, in `[0, 60)`.
    ///
    /// ```
    /// use geocoordinates::angle::{Dms, Hemisphere};
    ///
    /// assert_eq!(Dms::new(33, 51, 54.0, Hemisphere::South)?.minutes(), 51);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn minutes(&self) -> u8 {
        self.minutes
    }

    /// Seconds, in `[0, 60)`.
    ///
    /// ```
    /// use geocoordinates::angle::{Dms, Hemisphere};
    ///
    /// assert_eq!(Dms::new(33, 51, 54.5, Hemisphere::South)?.seconds(), 54.5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn seconds(&self) -> f64 {
        self.seconds
    }

    /// The hemisphere providing the sign (and the axis).
    ///
    /// ```
    /// use geocoordinates::angle::{Axis, Dms, Hemisphere};
    ///
    /// // `-0.0` reads as North, so a zeroed latitude never prints as South.
    /// assert_eq!(Dms::from_degrees(-0.0, Axis::Latitude)?.hemisphere(), Hemisphere::North);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn hemisphere(&self) -> Hemisphere {
        self.hemisphere
    }

    /// Signed decimal degrees, exact up to `f64` rounding.
    ///
    /// ```
    /// use geocoordinates::angle::{Dms, Hemisphere};
    ///
    /// assert_eq!(Dms::new(33, 51, 54.0, Hemisphere::South)?.to_degrees(), -33.865);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_degrees(self) -> f64 {
        let magnitude =
            f64::from(self.degrees) + f64::from(self.minutes) / 60.0 + self.seconds / 3600.0;
        magnitude.copysign(self.hemisphere.sign())
    }

    /// Convert to degrees-decimal-minutes, preserving the hemisphere.
    ///
    /// Exact up to `f64` rounding. Where seconds just below 60 round the
    /// decimal minutes up to exactly 60, the minute carries into the degrees,
    /// so the result is always a valid [`Ddm`].
    ///
    /// ```
    /// use geocoordinates::angle::{Dms, Hemisphere};
    ///
    /// let ddm = Dms::new(40, 42, 45.96, Hemisphere::North)?.to_ddm();
    /// assert_eq!((ddm.degrees(), ddm.hemisphere()), (40, Hemisphere::North));
    /// assert!((ddm.minutes() - 42.766).abs() < 1e-9);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_ddm(self) -> Ddm {
        let minutes = f64::from(self.minutes) + self.seconds / 60.0;
        let (degrees, minutes) = if minutes >= 60.0 {
            (self.degrees + 1, 0.0)
        } else {
            (self.degrees, minutes)
        };
        Ddm {
            degrees,
            minutes,
            hemisphere: self.hemisphere,
        }
    }
}

impl Ddm {
    /// Build a DDM angle from its components.
    ///
    /// The hemisphere selects the axis: North/South allows at most 90°,
    /// East/West at most 180°.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] naming `degrees` or `minutes` when that
    /// component is out of range: minutes outside `[0, 60)` or non-finite, or a
    /// value beyond the axis maximum.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::angle::{Ddm, Hemisphere};
    ///
    /// // NMEA `4042.766,N`: 40 degrees, 42.766 minutes.
    /// assert_eq!(Ddm::new(40, 42.766, Hemisphere::North)?.minutes(), 42.766);
    /// assert!(matches!(
    ///     Ddm::new(40, f64::NAN, Hemisphere::North),
    ///     Err(Error::InvalidValue { field: "minutes", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(degrees: u16, minutes: f64, hemisphere: Hemisphere) -> Result<Self> {
        validate_components(degrees, minutes, 0.0, axis_for_hemisphere(hemisphere))?;
        Ok(Ddm {
            degrees,
            minutes,
            hemisphere,
        })
    }

    /// Decompose signed decimal degrees on the given axis (latitude or
    /// longitude selects the hemisphere letters).
    ///
    /// `minutes` is kept full-precision, so a round trip back through
    /// [`Ddm::to_degrees`] is exact up to `f64` rounding. `-0.0` reads as
    /// North / East.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] (field `"decimal degrees"`) for a
    /// non-finite value or one beyond ±90° (latitude) / ±180° (longitude).
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::angle::{Axis, Ddm, Hemisphere};
    ///
    /// let ddm = Ddm::from_degrees(151.2093, Axis::Longitude)?;
    /// assert_eq!((ddm.degrees(), ddm.hemisphere()), (151, Hemisphere::East));
    /// assert!((ddm.minutes() - 12.558).abs() < 1e-9);
    /// assert!(matches!(
    ///     Ddm::from_degrees(f64::INFINITY, Axis::Longitude),
    ///     Err(Error::InvalidValue { field: "decimal degrees", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_degrees(value: f64, axis: Axis) -> Result<Self> {
        validate_dd(value, axis)?;
        let magnitude = value.abs();
        let degrees = magnitude.trunc();
        let minutes = (magnitude - degrees) * 60.0;
        Ddm::new(degrees as u16, minutes, hemisphere_for(axis, value))
    }

    /// Whole degrees (non-negative; the sign is carried by
    /// [`hemisphere`](Self::hemisphere)).
    ///
    /// ```
    /// use geocoordinates::angle::{Ddm, Hemisphere};
    ///
    /// assert_eq!(Ddm::new(122, 25.164, Hemisphere::West)?.degrees(), 122);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn degrees(&self) -> u16 {
        self.degrees
    }

    /// Decimal minutes, in `[0, 60)`.
    ///
    /// ```
    /// use geocoordinates::angle::{Ddm, Hemisphere};
    ///
    /// assert_eq!(Ddm::new(122, 25.164, Hemisphere::West)?.minutes(), 25.164);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn minutes(&self) -> f64 {
        self.minutes
    }

    /// The hemisphere providing the sign (and the axis).
    ///
    /// ```
    /// use geocoordinates::angle::{Axis, Ddm, Hemisphere};
    ///
    /// assert_eq!(Ddm::from_degrees(-0.5, Axis::Latitude)?.hemisphere(), Hemisphere::South);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn hemisphere(&self) -> Hemisphere {
        self.hemisphere
    }

    /// Signed decimal degrees, exact up to `f64` rounding.
    ///
    /// ```
    /// use geocoordinates::angle::{Ddm, Hemisphere};
    ///
    /// assert_eq!(Ddm::new(0, 30.0, Hemisphere::South)?.to_degrees(), -0.5);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_degrees(self) -> f64 {
        let magnitude = f64::from(self.degrees) + self.minutes / 60.0;
        magnitude.copysign(self.hemisphere.sign())
    }

    /// Convert to degrees-minutes-seconds, preserving the hemisphere.
    ///
    /// Exact up to `f64` rounding: the fractional minute is split off exactly,
    /// and scaling it by 60 stays below 60, so no carry is needed.
    ///
    /// ```
    /// use geocoordinates::angle::{Ddm, Hemisphere};
    ///
    /// let dms = Ddm::new(122, 25.5, Hemisphere::West)?.to_dms();
    /// assert_eq!((dms.degrees(), dms.minutes(), dms.seconds()), (122, 25, 30.0));
    /// assert_eq!(dms.hemisphere(), Hemisphere::West);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn to_dms(self) -> Dms {
        let whole = self.minutes.trunc();
        Dms {
            degrees: self.degrees,
            minutes: whole as u8,
            seconds: (self.minutes - whole) * 60.0,
            hemisphere: self.hemisphere,
        }
    }
}

fn axis_for_hemisphere(hemisphere: Hemisphere) -> Axis {
    match hemisphere {
        Hemisphere::North | Hemisphere::South => Axis::Latitude,
        Hemisphere::East | Hemisphere::West => Axis::Longitude,
    }
}

fn axis_max(axis: Axis) -> f64 {
    match axis {
        Axis::Latitude => 90.0,
        Axis::Longitude => 180.0,
    }
}

fn validate_dd(value: f64, axis: Axis) -> Result<()> {
    if value.is_finite() && value.abs() <= axis_max(axis) {
        return Ok(());
    }
    Err(Error::InvalidValue {
        field: "decimal degrees",
        detail: format!("must be finite and within ±{}°", axis_max(axis)),
    })
}

fn validate_components(degrees: u16, minutes: f64, seconds: f64, axis: Axis) -> Result<()> {
    let max = axis_max(axis);
    if f64::from(degrees) > max || f64::from(degrees) == max && (minutes != 0.0 || seconds != 0.0) {
        return Err(Error::InvalidValue {
            field: "degrees",
            detail: format!("must describe a value no greater than {max}°"),
        });
    }
    if !minutes.is_finite() || !(0.0..60.0).contains(&minutes) {
        return Err(Error::InvalidValue {
            field: "minutes",
            detail: "must be finite and within [0, 60)".to_string(),
        });
    }
    if !seconds.is_finite() || !(0.0..60.0).contains(&seconds) {
        return Err(Error::InvalidValue {
            field: "seconds",
            detail: "must be finite and within [0, 60)".to_string(),
        });
    }
    Ok(())
}

/// Which axis an angle represents — selects N/S vs E/W hemisphere letters.
///
/// ```
/// use geocoordinates::angle::{Axis, Ddm, Hemisphere};
///
/// // The same value reads as North on one axis and East on the other.
/// assert_eq!(Ddm::from_degrees(45.0, Axis::Latitude)?.hemisphere(), Hemisphere::North);
/// assert_eq!(Ddm::from_degrees(45.0, Axis::Longitude)?.hemisphere(), Hemisphere::East);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Axis {
    /// Latitude (N/S).
    Latitude,
    /// Longitude (E/W).
    Longitude,
}

/// Wrap a longitude into the half-open range `[-180, 180)`.
///
/// The range is **half-open**: the antimeridian normalizes to the western
/// edge, so `wrap_longitude(180.0) == -180.0`. A zero result is always
/// `+0.0`, never `-0.0`. Finite input is expected (a non-finite input
/// propagates as `NaN`).
///
/// ```
/// use geocoordinates::angle::wrap_longitude;
///
/// assert_eq!(wrap_longitude(190.0), -170.0);
/// assert_eq!(wrap_longitude(-540.0), -180.0);
/// assert_eq!(wrap_longitude(180.0), -180.0);
/// assert!(wrap_longitude(-0.0).is_sign_positive());
/// ```
#[must_use]
pub fn wrap_longitude(lon_deg: f64) -> f64 {
    let east = lon_deg.rem_euclid(360.0); // [0, 360]
    // `rem_euclid` keeps the sign of a zero remainder (`-0.0`, `-360.0`);
    // map it to `+0.0`.
    if east >= 180.0 {
        east - 360.0
    } else if east == 0.0 {
        0.0
    } else {
        east
    }
}

/// Clamp a latitude into the closed range `[-90, 90]`.
///
/// Both poles are included. Finite input is expected.
///
/// ```
/// use geocoordinates::angle::clamp_latitude;
///
/// assert_eq!(clamp_latitude(90.0), 90.0);
/// assert_eq!(clamp_latitude(-91.5), -90.0);
/// ```
#[must_use]
pub fn clamp_latitude(lat_deg: f64) -> f64 {
    lat_deg.clamp(-90.0, 90.0)
}

/// Normalize an angle (degrees) into the half-open range `[0, 360)`.
///
/// Use for bearings/azimuths as well — a bearing is just an angle in `[0, 360)`.
/// The result is never `360.0` (a tiny negative input such as `-1e-20` maps to
/// `0.0`) and never `-0.0`. Finite input is expected (a non-finite input
/// propagates as `NaN`).
///
/// ```
/// use geocoordinates::angle::normalize_degrees;
///
/// assert_eq!(normalize_degrees(-90.0), 270.0);
/// assert_eq!(normalize_degrees(720.0), 0.0);
/// assert_eq!(normalize_degrees(-1e-20), 0.0);
/// ```
#[must_use]
pub fn normalize_degrees(deg: f64) -> f64 {
    let r = deg.rem_euclid(360.0);
    // `rem_euclid` rounds a tiny negative remainder up to the excluded bound
    // 360.0, and keeps the sign of a zero; both map to `+0.0`.
    if r >= 360.0 || r == 0.0 { 0.0 } else { r }
}

#[cfg(test)]
mod tests;
