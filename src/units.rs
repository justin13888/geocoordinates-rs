//! Length units and conversions.
//!
//! Angle normalization helpers (longitude wrap, latitude clamp, degree
//! normalization) live in [`crate::angle`], alongside the DD/DMS/DDM encodings.

use core::ops::{Add, Mul, Sub};

/// A length, stored in meters, with conversions to common units.
///
/// Distinguishes the **US survey foot** from the **international foot** — a
/// recurring source of meter-scale errors over large distances.
///
/// Infallible: any `f64` is stored as given, and `+`, `-`, and `* f64` are
/// sugar over arithmetic on [`meters`](Self::meters).
///
/// ```
/// use geocoordinates::{Length, LengthUnit};
///
/// let leg = Length::from_unit(2.0, LengthUnit::Kilometer);
/// let total = leg + Length::from_meters(500.0) - Length::ZERO;
/// assert_eq!(total.meters(), 2500.0);
/// assert_eq!((total * 2.0).to_unit(LengthUnit::Kilometer), 5.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Length {
    meters: f64,
}

impl Length {
    /// The zero length.
    ///
    /// ```
    /// use geocoordinates::Length;
    ///
    /// assert_eq!(Length::ZERO.meters(), 0.0);
    /// assert_eq!(Length::from_meters(3.0) + Length::ZERO, Length::from_meters(3.0));
    /// ```
    pub const ZERO: Length = Length { meters: 0.0 };

    /// Construct from meters.
    ///
    /// ```
    /// use geocoordinates::Length;
    ///
    /// assert_eq!(Length::from_meters(42.0).meters(), 42.0);
    /// ```
    #[must_use]
    pub fn from_meters(meters: f64) -> Self {
        Self { meters }
    }

    /// Value in meters.
    ///
    /// ```
    /// use geocoordinates::{Length, LengthUnit};
    ///
    /// assert_eq!(Length::from_unit(1.0, LengthUnit::NauticalMile).meters(), 1852.0);
    /// ```
    #[must_use]
    pub fn meters(&self) -> f64 {
        self.meters
    }

    /// Construct from a value in the given unit.
    ///
    /// ```
    /// use geocoordinates::{Length, LengthUnit};
    ///
    /// assert_eq!(Length::from_unit(10.0, LengthUnit::Foot).meters(), 3.048);
    /// ```
    #[must_use]
    pub fn from_unit(value: f64, unit: LengthUnit) -> Self {
        Self::from_meters(value * unit.meters_per_unit())
    }

    /// Value in the requested unit.
    ///
    /// ```
    /// use geocoordinates::{Length, LengthUnit};
    ///
    /// let marathon = Length::from_meters(42_195.0);
    /// assert_eq!(marathon.to_unit(LengthUnit::Kilometer), 42.195);
    /// assert!((marathon.to_unit(LengthUnit::Foot) - 138_435.039).abs() < 1e-3);
    /// ```
    #[must_use]
    pub fn to_unit(&self, unit: LengthUnit) -> f64 {
        self.meters / unit.meters_per_unit()
    }
}

/// Supported length units.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every variant,
/// so adding one here is a deliberate, compile-forcing change on both sides.
///
/// ```
/// use geocoordinates::{Length, LengthUnit};
///
/// // A million feet: the survey foot is ~0.61 m longer in total.
/// let intl = Length::from_unit(1e6, LengthUnit::Foot).meters();
/// let survey = Length::from_unit(1e6, LengthUnit::UsSurveyFoot).meters();
/// assert!((survey - intl - 0.6096).abs() < 1e-3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl LengthUnit {
    /// Meters in one of this unit. The US survey foot keeps its defining
    /// `1200/3937` ratio rather than a pre-divided literal.
    #[must_use]
    fn meters_per_unit(self) -> f64 {
        match self {
            LengthUnit::Meter => 1.0,
            LengthUnit::Kilometer => 1000.0,
            LengthUnit::Foot => 0.3048,
            LengthUnit::UsSurveyFoot => 1200.0 / 3937.0,
            LengthUnit::NauticalMile => 1852.0,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    const UNITS: &[LengthUnit] = &[
        LengthUnit::Meter,
        LengthUnit::Kilometer,
        LengthUnit::Foot,
        LengthUnit::UsSurveyFoot,
        LengthUnit::NauticalMile,
    ];

    #[test]
    fn unit_round_trip() {
        for &unit in UNITS {
            for &v in &[0.0, 1.0, 1234.5, -9.75, 1e6] {
                assert_close(Length::from_unit(v, unit).to_unit(unit), v, 1e-9);
            }
        }
    }

    #[test]
    fn known_factors() {
        assert_close(
            Length::from_unit(1.0, LengthUnit::Meter).meters(),
            1.0,
            1e-12,
        );
        assert_close(
            Length::from_unit(1.0, LengthUnit::Kilometer).meters(),
            1000.0,
            1e-12,
        );
        assert_close(
            Length::from_unit(1.0, LengthUnit::Foot).meters(),
            0.3048,
            1e-12,
        );
        assert_close(
            Length::from_unit(1.0, LengthUnit::NauticalMile).meters(),
            1852.0,
            1e-12,
        );
    }

    #[test]
    fn survey_foot_differs_from_international_foot() {
        let intl = Length::from_unit(1.0, LengthUnit::Foot).meters();
        let survey = Length::from_unit(1.0, LengthUnit::UsSurveyFoot).meters();
        assert!(
            (intl - survey).abs() > 1e-9,
            "survey foot must differ from the international foot"
        );
        // Over a million feet the gap is meter-scale (~0.6096 m).
        let gap = (Length::from_unit(1e6, LengthUnit::UsSurveyFoot).meters()
            - Length::from_unit(1e6, LengthUnit::Foot).meters())
        .abs();
        assert_close(gap, 0.6096, 1e-3);
    }
}
