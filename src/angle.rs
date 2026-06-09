//! Angle encodings: decimal degrees (DD), degrees-minutes-seconds (DMS), and
//! degrees-decimal-minutes (DDM), plus angle-normalization helpers (longitude
//! wrap, latitude clamp, degree normalization).
//!
//! DDM is what NMEA and marine/aviation use and is frequently forgotten.
//! Conversions between these encodings are **exact** (pure arithmetic), so they
//! implement [`From`].

/// North/South or East/West hemisphere sign.
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

/// Decimal degrees, e.g. `40.7128`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dd(pub f64);

/// Degrees, minutes, seconds, e.g. `40° 42′ 46″ N`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// Degrees and decimal minutes, e.g. `40° 42.766′ N` (NMEA/marine).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ddm {
    /// Whole degrees (non-negative; sign carried by `hemisphere`).
    pub degrees: u16,
    /// Decimal minutes `[0, 60)`.
    pub minutes: f64,
    /// Hemisphere providing the sign.
    pub hemisphere: Hemisphere,
}

impl Dd {
    /// Convert to DMS for the given axis (latitude or longitude selects the
    /// hemisphere letters).
    #[must_use]
    pub fn to_dms(self, axis: Axis) -> Dms {
        todo!()
    }

    /// Convert to DDM for the given axis.
    #[must_use]
    pub fn to_ddm(self, axis: Axis) -> Ddm {
        todo!()
    }
}

impl Dms {
    /// Convert to degrees-decimal-minutes, preserving the hemisphere.
    #[must_use]
    pub fn to_ddm(self) -> Ddm {
        todo!("minutes = self.minutes + self.seconds/60")
    }
}

impl Ddm {
    /// Convert to degrees-minutes-seconds, preserving the hemisphere.
    #[must_use]
    pub fn to_dms(self) -> Dms {
        todo!("seconds = fract(minutes)*60; minutes = trunc(minutes)")
    }
}

impl From<Dms> for Dd {
    fn from(dms: Dms) -> Self {
        todo!("deg + min/60 + sec/3600, signed by hemisphere")
    }
}

impl From<Ddm> for Dd {
    fn from(ddm: Ddm) -> Self {
        todo!("deg + min/60, signed by hemisphere")
    }
}

/// Which axis an angle represents — selects N/S vs E/W hemisphere letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Latitude (N/S).
    Latitude,
    /// Longitude (E/W).
    Longitude,
}

/// Wrap a longitude into the half-open range `[-180, 180)`.
#[must_use]
pub fn wrap_longitude(lon_deg: f64) -> f64 {
    todo!("normalize longitude across the antimeridian")
}

/// Clamp a latitude into `[-90, 90]`.
#[must_use]
pub fn clamp_latitude(lat_deg: f64) -> f64 {
    todo!("clamp latitude to the poles")
}

/// Normalize an angle (degrees) into `[0, 360)`.
///
/// Use for bearings/azimuths as well — a bearing is just an angle in `[0, 360)`.
#[must_use]
pub fn normalize_degrees(deg: f64) -> f64 {
    todo!("deg.rem_euclid(360.0)")
}
