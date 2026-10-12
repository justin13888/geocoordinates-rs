//! Angle encodings (DMS, DDM), their conversions to and from signed decimal
//! degrees, and the angle normalization helpers.

use geocoordinates as gc;

use crate::GeoError;

// --- Angle encodings (mirror of `gc::angle`) ---

/// North/South or East/West hemisphere sign — mirror of [`gc::angle::Hemisphere`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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

/// Which axis an angle represents — mirror of [`gc::angle::Axis`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Axis {
    /// Latitude (selects N/S).
    Latitude,
    /// Longitude (selects E/W).
    Longitude,
}

/// Degrees / minutes / seconds — mirror of [`gc::angle::Dms`].
///
/// Unvalidated until used: every function taking one applies
/// [`gc::angle::Dms::new`]'s range checks first (see [`dms_validate`]).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Dms {
    /// Whole degrees (non-negative; sign carried by `hemisphere`).
    pub degrees: u16,
    /// Whole minutes `[0, 60)`.
    pub minutes: u8,
    /// Seconds `[0, 60)`.
    pub seconds: f64,
    /// Hemisphere providing the sign.
    pub hemisphere: Hemisphere,
}

/// Degrees / decimal minutes — mirror of [`gc::angle::Ddm`].
///
/// Unvalidated until used: every function taking one applies
/// [`gc::angle::Ddm::new`]'s range checks first (see [`ddm_validate`]).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Ddm {
    /// Whole degrees (non-negative; sign carried by `hemisphere`).
    pub degrees: u16,
    /// Decimal minutes `[0, 60)`.
    pub minutes: f64,
    /// Hemisphere providing the sign.
    pub hemisphere: Hemisphere,
}

impl From<gc::angle::Hemisphere> for Hemisphere {
    fn from(h: gc::angle::Hemisphere) -> Self {
        match h {
            gc::angle::Hemisphere::North => Hemisphere::North,
            gc::angle::Hemisphere::South => Hemisphere::South,
            gc::angle::Hemisphere::East => Hemisphere::East,
            gc::angle::Hemisphere::West => Hemisphere::West,
        }
    }
}

impl From<Hemisphere> for gc::angle::Hemisphere {
    fn from(h: Hemisphere) -> Self {
        match h {
            Hemisphere::North => gc::angle::Hemisphere::North,
            Hemisphere::South => gc::angle::Hemisphere::South,
            Hemisphere::East => gc::angle::Hemisphere::East,
            Hemisphere::West => gc::angle::Hemisphere::West,
        }
    }
}

impl From<gc::angle::Axis> for Axis {
    fn from(a: gc::angle::Axis) -> Self {
        match a {
            gc::angle::Axis::Latitude => Axis::Latitude,
            gc::angle::Axis::Longitude => Axis::Longitude,
        }
    }
}

impl From<Axis> for gc::angle::Axis {
    fn from(a: Axis) -> Self {
        match a {
            Axis::Latitude => gc::angle::Axis::Latitude,
            Axis::Longitude => gc::angle::Axis::Longitude,
        }
    }
}

impl From<gc::angle::Dms> for Dms {
    fn from(d: gc::angle::Dms) -> Self {
        Dms {
            degrees: d.degrees(),
            minutes: d.minutes(),
            seconds: d.seconds(),
            hemisphere: d.hemisphere().into(),
        }
    }
}

impl TryFrom<Dms> for gc::angle::Dms {
    type Error = GeoError;

    fn try_from(d: Dms) -> Result<Self, GeoError> {
        gc::angle::Dms::new(d.degrees, d.minutes, d.seconds, d.hemisphere.into())
            .map_err(GeoError::from)
    }
}

impl From<gc::angle::Ddm> for Ddm {
    fn from(d: gc::angle::Ddm) -> Self {
        Ddm {
            degrees: d.degrees(),
            minutes: d.minutes(),
            hemisphere: d.hemisphere().into(),
        }
    }
}

impl TryFrom<Ddm> for gc::angle::Ddm {
    type Error = GeoError;

    fn try_from(d: Ddm) -> Result<Self, GeoError> {
        gc::angle::Ddm::new(d.degrees, d.minutes, d.hemisphere.into()).map_err(GeoError::from)
    }
}

// --- Angle conversions ---

/// Decompose signed decimal degrees on the given axis into
/// degrees/minutes/seconds.
///
/// # Errors
/// `GeoError::InvalidValue` (field `"decimal degrees"`) for a non-finite value
/// or one beyond ±90° (latitude) / ±180° (longitude).
#[uniffi::export]
pub fn dms_from_degrees(value: f64, axis: Axis) -> Result<Dms, GeoError> {
    Ok(gc::angle::Dms::from_degrees(value, axis.into())?.into())
}

/// Decompose signed decimal degrees on the given axis into
/// degrees/decimal-minutes.
///
/// # Errors
/// `GeoError::InvalidValue` (field `"decimal degrees"`) for a non-finite value
/// or one beyond ±90° (latitude) / ±180° (longitude).
#[uniffi::export]
pub fn ddm_from_degrees(value: f64, axis: Axis) -> Result<Ddm, GeoError> {
    Ok(gc::angle::Ddm::from_degrees(value, axis.into())?.into())
}

/// Degrees/minutes/seconds → signed decimal degrees (signed by hemisphere),
/// exact up to `f64` rounding.
///
/// # Errors
/// `GeoError::InvalidValue` naming the out-of-range component (see
/// [`dms_validate`]).
#[uniffi::export]
pub fn dms_to_degrees(dms: Dms) -> Result<f64, GeoError> {
    Ok(gc::angle::Dms::try_from(dms)?.to_degrees())
}

/// Degrees/decimal-minutes → signed decimal degrees (signed by hemisphere),
/// exact up to `f64` rounding.
///
/// # Errors
/// `GeoError::InvalidValue` naming the out-of-range component (see
/// [`ddm_validate`]).
#[uniffi::export]
pub fn ddm_to_degrees(ddm: Ddm) -> Result<f64, GeoError> {
    Ok(gc::angle::Ddm::try_from(ddm)?.to_degrees())
}

/// Degrees/minutes/seconds → degrees/decimal-minutes (preserves hemisphere).
///
/// # Errors
/// `GeoError::InvalidValue` naming the out-of-range component (see
/// [`dms_validate`]).
#[uniffi::export]
pub fn dms_to_ddm(dms: Dms) -> Result<Ddm, GeoError> {
    Ok(gc::angle::Dms::try_from(dms)?.to_ddm().into())
}

/// Degrees/decimal-minutes → degrees/minutes/seconds (preserves hemisphere).
///
/// # Errors
/// `GeoError::InvalidValue` naming the out-of-range component (see
/// [`ddm_validate`]).
#[uniffi::export]
pub fn ddm_to_dms(ddm: Ddm) -> Result<Dms, GeoError> {
    Ok(gc::angle::Ddm::try_from(ddm)?.to_dms().into())
}

/// Check a DMS record against [`gc::angle::Dms::new`]'s ranges: minutes and
/// seconds in `[0, 60)`, and at most 90° (North/South) or 180° (East/West).
///
/// # Errors
/// `GeoError::InvalidValue` naming `degrees`, `minutes`, or `seconds`.
#[uniffi::export]
pub fn dms_validate(dms: Dms) -> Result<(), GeoError> {
    gc::angle::Dms::try_from(dms).map(|_| ())
}

/// Check a DDM record against [`gc::angle::Ddm::new`]'s ranges: minutes in
/// `[0, 60)`, and at most 90° (North/South) or 180° (East/West).
///
/// # Errors
/// `GeoError::InvalidValue` naming `degrees` or `minutes`.
#[uniffi::export]
pub fn ddm_validate(ddm: Ddm) -> Result<(), GeoError> {
    gc::angle::Ddm::try_from(ddm).map(|_| ())
}

/// The numeric sign a hemisphere applies (`-1.0` for South/West, else `+1.0`).
#[uniffi::export]
pub fn hemisphere_sign(hemisphere: Hemisphere) -> f64 {
    gc::angle::Hemisphere::from(hemisphere).sign()
}

// --- Angle normalization helpers ---

/// Wrap a longitude into the half-open range `[-180, 180)` (so `180` → `-180`).
#[uniffi::export]
pub fn wrap_longitude(lon_deg: f64) -> f64 {
    gc::angle::wrap_longitude(lon_deg)
}

/// Clamp a latitude into the closed range `[-90, 90]`.
#[uniffi::export]
pub fn clamp_latitude(lat_deg: f64) -> f64 {
    gc::angle::clamp_latitude(lat_deg)
}

/// Normalize an angle (degrees) into the half-open range `[0, 360)`; the result
/// is never `360.0` or `-0.0`.
#[uniffi::export]
pub fn normalize_degrees(deg: f64) -> f64 {
    gc::angle::normalize_degrees(deg)
}
