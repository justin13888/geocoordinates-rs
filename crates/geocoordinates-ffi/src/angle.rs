//! Angle encodings (decimal degrees, DMS, DDM), their conversions, and the
//! angle normalization helpers.

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

/// Decimal degrees — mirror of the `gc::angle::Dd` newtype.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Dd {
    /// The signed angle in decimal degrees.
    pub value: f64,
}

/// Degrees / minutes / seconds — mirror of [`gc::angle::Dms`].
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

impl From<gc::angle::Dd> for Dd {
    fn from(d: gc::angle::Dd) -> Self {
        Dd { value: d.0 }
    }
}

impl From<Dd> for gc::angle::Dd {
    fn from(d: Dd) -> Self {
        gc::angle::Dd(d.value)
    }
}

impl From<gc::angle::Dms> for Dms {
    fn from(d: gc::angle::Dms) -> Self {
        Dms {
            degrees: d.degrees,
            minutes: d.minutes,
            seconds: d.seconds,
            hemisphere: d.hemisphere.into(),
        }
    }
}

impl From<Dms> for gc::angle::Dms {
    fn from(d: Dms) -> Self {
        gc::angle::Dms {
            degrees: d.degrees,
            minutes: d.minutes,
            seconds: d.seconds,
            hemisphere: d.hemisphere.into(),
        }
    }
}

impl From<gc::angle::Ddm> for Ddm {
    fn from(d: gc::angle::Ddm) -> Self {
        Ddm {
            degrees: d.degrees,
            minutes: d.minutes,
            hemisphere: d.hemisphere.into(),
        }
    }
}

impl From<Ddm> for gc::angle::Ddm {
    fn from(d: Ddm) -> Self {
        gc::angle::Ddm {
            degrees: d.degrees,
            minutes: d.minutes,
            hemisphere: d.hemisphere.into(),
        }
    }
}

// --- Angle conversions ---

/// Decimal degrees → degrees/minutes/seconds for the given axis.
#[uniffi::export]
pub fn dd_to_dms(dd: Dd, axis: Axis) -> Result<Dms, GeoError> {
    gc::angle::Dd::from(dd)
        .try_to_dms(axis.into())
        .map(Into::into)
        .map_err(Into::into)
}

/// Decimal degrees → degrees/decimal-minutes for the given axis.
#[uniffi::export]
pub fn dd_to_ddm(dd: Dd, axis: Axis) -> Result<Ddm, GeoError> {
    gc::angle::Dd::from(dd)
        .try_to_ddm(axis.into())
        .map(Into::into)
        .map_err(Into::into)
}

/// Degrees/minutes/seconds → degrees/decimal-minutes (preserves hemisphere).
#[uniffi::export]
pub fn dms_to_ddm(dms: Dms) -> Result<Ddm, GeoError> {
    gc::angle::Dms::from(dms)
        .try_to_ddm()
        .map(Into::into)
        .map_err(Into::into)
}

/// Degrees/decimal-minutes → degrees/minutes/seconds (preserves hemisphere).
#[uniffi::export]
pub fn ddm_to_dms(ddm: Ddm) -> Result<Dms, GeoError> {
    gc::angle::Ddm::from(ddm)
        .try_to_dms()
        .map(Into::into)
        .map_err(Into::into)
}

/// Degrees/minutes/seconds → decimal degrees (signed by hemisphere).
#[uniffi::export]
pub fn dms_to_dd(dms: Dms) -> Result<Dd, GeoError> {
    gc::angle::Dms::from(dms)
        .try_to_dd()
        .map(Into::into)
        .map_err(Into::into)
}

/// Degrees/decimal-minutes → decimal degrees (signed by hemisphere).
#[uniffi::export]
pub fn ddm_to_dd(ddm: Ddm) -> Result<Dd, GeoError> {
    gc::angle::Ddm::from(ddm)
        .try_to_dd()
        .map(Into::into)
        .map_err(Into::into)
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

/// Normalize an angle (degrees) into `[0, 360)`.
#[uniffi::export]
pub fn normalize_degrees(deg: f64) -> f64 {
    gc::angle::normalize_degrees(deg)
}
