//! Length units and conversions. The surface-tagged
//! [`Height`](crate::Height) lives with [`Coordinate`](crate::Coordinate).
//!
//! Angle normalization helpers (longitude wrap, latitude clamp, degree
//! normalization) live in [`crate::angle`], alongside the DD/DMS/DDM encodings.

use core::ops::{Add, Mul, Sub};

use crate::error::{Error, Result};

/// A length, stored in meters, with conversions to common units.
///
/// Distinguishes the **US survey foot** from the **international foot** — a
/// recurring source of meter-scale errors over large distances.
///
/// Infallible: any `f64` is stored as given, and `+`, `-`, and `* f64` are
/// sugar over arithmetic on [`meters`](Self::meters).
///
/// Under the `serde` feature a `Length` is a bare number of meters (`5.0`,
/// not `{"meters":5.0}`), and every serialized field holding one carries the
/// unit in its key, which ends in `_m` (`horizontal_m`, `semi_major_m`).
///
/// ```
/// use geocoordinates::{Length, LengthUnit};
///
/// let leg = Length::from_unit(2.0, LengthUnit::Kilometer);
/// let total = leg + Length::from_meters(500.0) - Length::ZERO;
/// assert_eq!(total.meters(), 2500.0);
/// assert_eq!((total * 2.0).to_unit(LengthUnit::Kilometer), 5.0);
/// # #[cfg(feature = "serde")]
/// # {
/// assert_eq!(serde_json::to_string(&total).unwrap(), "2500.0");
/// assert_eq!(serde_json::from_str::<Length>("2500.0").unwrap(), total);
/// // The old wrapped shape no longer reads.
/// assert!(serde_json::from_str::<Length>(r#"{"meters":2500.0}"#).is_err());
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
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
    pub const fn from_meters(meters: f64) -> Self {
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
    pub const fn meters(self) -> f64 {
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
    pub fn to_unit(self, unit: LengthUnit) -> f64 {
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
///
/// // Statute miles and yards are the exact international definitions.
/// assert_eq!(Length::from_unit(1.0, LengthUnit::Mile).meters(), 1609.344);
/// assert_eq!(Length::from_unit(1.0, LengthUnit::Yard).meters(), 0.9144);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Nautical mile (exactly 1852 m).
    NauticalMile,
    /// International statute mile (exactly 1609.344 m, 5280 international
    /// feet).
    Mile,
    /// International yard (exactly 0.9144 m, 3 international feet).
    Yard,
}

impl LengthUnit {
    /// Meters in one of this unit. The US survey foot keeps its defining
    /// `1200/3937` ratio rather than a pre-divided literal.
    ///
    /// ```
    /// use geocoordinates::LengthUnit;
    ///
    /// assert_eq!(LengthUnit::NauticalMile.meters_per_unit(), 1852.0);
    /// assert_eq!(LengthUnit::UsSurveyFoot.meters_per_unit(), 1200.0 / 3937.0);
    /// ```
    #[must_use]
    pub const fn meters_per_unit(self) -> f64 {
        match self {
            LengthUnit::Meter => 1.0,
            LengthUnit::Kilometer => 1000.0,
            LengthUnit::Foot => 0.3048,
            LengthUnit::UsSurveyFoot => 1200.0 / 3937.0,
            LengthUnit::NauticalMile => 1852.0,
            LengthUnit::Mile => 1609.344,
            LengthUnit::Yard => 0.9144,
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

/// `value` if it is finite, else [`Error::InvalidValue`] naming `field`.
pub(crate) fn finite(value: f64, field: &'static str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidValue {
            field,
            detail: "must be finite".into(),
        })
    }
}

/// Days from 1970-01-01 to a proleptic Gregorian date (Hinnant's algorithm),
/// negative before the epoch. `month` is 1–12 and `day` 1–31; the caller has
/// already checked the day against its month.
#[cfg(any(feature = "exif", feature = "nmea"))]
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y.rem_euclid(400);
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    /// Every date from 1900 to 2099 is the day count reached by walking the
    /// calendar one day at a time from 1970-01-01, both ways.
    #[cfg(any(feature = "exif", feature = "nmea"))]
    #[test]
    fn days_from_civil_counts_every_day_from_the_epoch() {
        let month_days = |year: i64, month: i64| match month {
            2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        let mut expected = 0;
        for year in 1970..2100 {
            for month in 1..=12 {
                for day in 1..=month_days(year, month) {
                    assert_eq!(
                        days_from_civil(year, month, day),
                        expected,
                        "{year}-{month}-{day}"
                    );
                    expected += 1;
                }
            }
        }
        let mut expected = 0;
        for year in (1900..1970).rev() {
            for month in (1..=12).rev() {
                for day in (1..=month_days(year, month)).rev() {
                    expected -= 1;
                    assert_eq!(
                        days_from_civil(year, month, day),
                        expected,
                        "{year}-{month}-{day}"
                    );
                }
            }
        }
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }

    const UNITS: &[LengthUnit] = &[
        LengthUnit::Meter,
        LengthUnit::Kilometer,
        LengthUnit::Foot,
        LengthUnit::UsSurveyFoot,
        LengthUnit::NauticalMile,
        LengthUnit::Mile,
        LengthUnit::Yard,
    ];

    /// A `Length` is a bare JSON number of meters, both ways.
    #[cfg(feature = "serde")]
    #[test]
    fn length_serializes_as_bare_meters() {
        let length = Length::from_unit(1.0, LengthUnit::Kilometer);
        assert_eq!(
            serde_json::to_value(length).unwrap(),
            serde_json::json!(1000.0)
        );
        assert_eq!(serde_json::from_str::<Length>("1000").unwrap(), length);
        assert!(serde_json::from_str::<Length>(r#"{"meters":1000.0}"#).is_err());
    }

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
        assert_eq!(Length::from_unit(1.0, LengthUnit::Mile).meters(), 1609.344);
        assert_eq!(Length::from_unit(1.0, LengthUnit::Yard).meters(), 0.9144);
        // 1 mile = 1760 yards = 5280 feet, exactly as defined.
        assert_close(
            Length::from_unit(1760.0, LengthUnit::Yard).to_unit(LengthUnit::Mile),
            1.0,
            1e-12,
        );
        assert_close(
            Length::from_unit(5280.0, LengthUnit::Foot).to_unit(LengthUnit::Mile),
            1.0,
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
