//! Length units and conversions.
//!
//! Angle normalization helpers (longitude wrap, latitude clamp, degree
//! normalization) live in [`crate::angle`], alongside the DD/DMS/DDM encodings.

use core::ops::{Add, Mul, Sub};

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
    /// The zero length.
    pub const ZERO: Length = Length { meters: 0.0 };

    /// Construct from meters.
    #[must_use]
    pub fn from_meters(meters: f64) -> Self {
        Self { meters }
    }

    /// Value in meters.
    #[must_use]
    pub fn meters(&self) -> f64 {
        self.meters
    }

    // --- Unit conversions: released in 0.2 (see ROADMAP.md) ---
    /*
    /// Construct from a value in the given unit.
    #[must_use]
    pub fn from_unit(value: f64, unit: LengthUnit) -> Self {
        todo!("scale `value` by the unit's meter factor")
    }

    /// Value in the requested unit.
    #[must_use]
    pub fn to_unit(&self, unit: LengthUnit) -> f64 {
        todo!("divide meters by the unit's meter factor")
    }
    */
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

impl Add for Length {
    type Output = Length;
    fn add(self, rhs: Length) -> Length {
        Length {
            meters: self.meters + rhs.meters,
        }
    }
}

impl Sub for Length {
    type Output = Length;
    fn sub(self, rhs: Length) -> Length {
        Length {
            meters: self.meters - rhs.meters,
        }
    }
}

impl Mul<f64> for Length {
    type Output = Length;
    fn mul(self, rhs: f64) -> Length {
        Length {
            meters: self.meters * rhs,
        }
    }
}
