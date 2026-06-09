//! Length units and angle normalization helpers.

/// A length, stored in meters, with conversions to common units.
///
/// Distinguishes the **US survey foot** from the **international foot** — a
/// recurring source of meter-scale errors over large distances.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Length {
    meters: f64,
}

impl Length {
    /// Construct from meters.
    #[must_use]
    pub fn from_meters(meters: f64) -> Self {
        Self { meters }
    }

    /// Construct from a value in the given unit.
    #[must_use]
    pub fn from_unit(value: f64, unit: LengthUnit) -> Self {
        todo!("scale `value` by the unit's meter factor")
    }

    /// Value in meters.
    #[must_use]
    pub fn meters(&self) -> f64 {
        self.meters
    }

    /// Value in the requested unit.
    #[must_use]
    pub fn to_unit(&self, unit: LengthUnit) -> f64 {
        todo!("divide meters by the unit's meter factor")
    }
}

/// Supported length units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum LengthUnit {
    /// Meter (SI).
    Meter,
    /// Kilometer.
    Kilometer,
    /// International foot (exactly 0.3048 m).
    Foot,
    /// US survey foot (1200/3937 m) — distinct from the international foot.
    UsSurveyFoot,
    /// Nautical mile (1852 m).
    NauticalMile,
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
#[must_use]
pub fn normalize_degrees(deg: f64) -> f64 {
    todo!()
}

/// Normalize a bearing (degrees) into `[0, 360)`.
#[must_use]
pub fn normalize_bearing(bearing_deg: f64) -> f64 {
    todo!()
}
