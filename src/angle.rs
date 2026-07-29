//! Angle encodings: decimal degrees (DD), degrees-minutes-seconds (DMS), and
//! degrees-decimal-minutes (DDM), plus angle-normalization helpers (longitude
//! wrap, latitude clamp, degree normalization).
//!
//! DDM is what NMEA and marine/aviation use and is frequently forgotten.
//! Conversions between valid encodings are **exact** (pure arithmetic). Public
//! records can be constructed directly, so fallible named methods validate
//! their documented component ranges before converting.

use crate::error::{Error, Result};

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

impl Hemisphere {
    /// The numeric sign this hemisphere applies: `-1.0` for the negative
    /// hemispheres (South / West), `+1.0` for the positive ones (North / East).
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

impl Dd {
    /// Convert to DMS for the given axis (latitude or longitude selects the
    /// hemisphere letters).
    ///
    /// `seconds` is kept full-precision and is **not** pre-rounded, so a round
    /// trip back through [`Dms::try_to_dd`] is exact. Rounding — and the 60″ carry it
    /// can imply — is the formatter's responsibility (a later release).
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] for a non-finite or out-of-axis-range
    /// value.
    pub fn try_to_dms(self, axis: Axis) -> Result<Dms> {
        validate_dd(self.0, axis)?;
        let magnitude = self.0.abs();
        let degrees = magnitude.trunc();
        let rem_minutes = (magnitude - degrees) * 60.0;
        let minutes = rem_minutes.trunc();
        let seconds = (rem_minutes - minutes) * 60.0;
        Ok(Dms {
            degrees: degrees as u16,
            minutes: minutes as u8,
            seconds,
            hemisphere: hemisphere_for(axis, self.0),
        })
    }

    /// Convert to DDM for the given axis.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] for a non-finite or out-of-axis-range
    /// value.
    pub fn try_to_ddm(self, axis: Axis) -> Result<Ddm> {
        validate_dd(self.0, axis)?;
        let magnitude = self.0.abs();
        let degrees = magnitude.trunc();
        let minutes = (magnitude - degrees) * 60.0;
        Ok(Ddm {
            degrees: degrees as u16,
            minutes,
            hemisphere: hemisphere_for(axis, self.0),
        })
    }
}

impl Dms {
    /// Validate the DMS component and axis ranges.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] when the record is not a canonical
    /// latitude or longitude.
    pub fn validate(&self) -> Result<()> {
        validate_components(
            self.degrees,
            f64::from(self.minutes),
            self.seconds,
            axis_for_hemisphere(self.hemisphere),
        )
    }

    /// Convert to degrees-decimal-minutes, preserving the hemisphere.
    ///
    /// # Errors
    /// Returns an error when this record violates the documented DMS ranges.
    pub fn try_to_ddm(self) -> Result<Ddm> {
        self.validate()?;
        Ok(Ddm {
            degrees: self.degrees,
            minutes: f64::from(self.minutes) + self.seconds / 60.0,
            hemisphere: self.hemisphere,
        })
    }

    /// Convert to signed decimal degrees.
    ///
    /// # Errors
    /// Returns an error when this record violates the documented DMS ranges.
    pub fn try_to_dd(self) -> Result<Dd> {
        self.validate()?;
        let magnitude =
            f64::from(self.degrees) + f64::from(self.minutes) / 60.0 + self.seconds / 3600.0;
        Ok(Dd(magnitude * self.hemisphere.sign()))
    }
}

impl Ddm {
    /// Validate the DDM component and axis ranges.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] when the record is not a canonical
    /// latitude or longitude.
    pub fn validate(&self) -> Result<()> {
        validate_components(
            self.degrees,
            self.minutes,
            0.0,
            axis_for_hemisphere(self.hemisphere),
        )
    }

    /// Convert to degrees-minutes-seconds, preserving the hemisphere.
    ///
    /// # Errors
    /// Returns an error when this record violates the documented DDM ranges.
    pub fn try_to_dms(self) -> Result<Dms> {
        self.validate()?;
        let whole = self.minutes.trunc();
        Ok(Dms {
            degrees: self.degrees,
            minutes: whole as u8,
            seconds: (self.minutes - whole) * 60.0,
            hemisphere: self.hemisphere,
        })
    }

    /// Convert to signed decimal degrees.
    ///
    /// # Errors
    /// Returns an error when this record violates the documented DDM ranges.
    pub fn try_to_dd(self) -> Result<Dd> {
        self.validate()?;
        let magnitude = f64::from(self.degrees) + self.minutes / 60.0;
        Ok(Dd(magnitude * self.hemisphere.sign()))
    }
}

impl TryFrom<Dms> for Dd {
    type Error = Error;

    fn try_from(dms: Dms) -> Result<Self> {
        dms.try_to_dd()
    }
}

impl TryFrom<Ddm> for Dd {
    type Error = Error;

    fn try_from(ddm: Ddm) -> Result<Self> {
        ddm.try_to_dd()
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
/// edge, so `wrap_longitude(180.0) == -180.0`. Finite input is expected
/// (a non-finite input propagates as `NaN`).
#[must_use]
pub fn wrap_longitude(lon_deg: f64) -> f64 {
    let east = lon_deg.rem_euclid(360.0); // [0, 360)
    if east >= 180.0 { east - 360.0 } else { east }
}

/// Clamp a latitude into the closed range `[-90, 90]`.
///
/// Both poles are included. Finite input is expected.
#[must_use]
pub fn clamp_latitude(lat_deg: f64) -> f64 {
    lat_deg.clamp(-90.0, 90.0)
}

/// Normalize an angle (degrees) into `[0, 360)`.
///
/// Use for bearings/azimuths as well — a bearing is just an angle in `[0, 360)`.
/// Finite input is expected.
#[must_use]
pub fn normalize_degrees(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    /// Spread of signed values × axes used for round-trip checks. Includes
    /// `0.0`/`-0.0`, the poles/antimeridian, and a high-precision fraction.
    const SAMPLES: &[f64] = &[
        0.0,
        -0.0,
        1.0,
        -1.0,
        40.712_775_3,
        -74.006_0,
        89.999_999,
        -89.999_999,
        179.999_999,
        -179.999_999,
        45.123_456_789,
        -123.456_789,
    ];

    #[test]
    fn dd_dms_round_trip_is_exact() {
        for &v in SAMPLES {
            for axis in [Axis::Latitude, Axis::Longitude] {
                if axis == Axis::Latitude && v.abs() > 90.0 {
                    continue;
                }
                let back = Dd(v).try_to_dms(axis).and_then(Dms::try_to_dd).unwrap().0;
                assert_close(back, v, 1e-12);
            }
        }
    }

    #[test]
    fn dd_ddm_round_trip_is_exact() {
        for &v in SAMPLES {
            for axis in [Axis::Latitude, Axis::Longitude] {
                if axis == Axis::Latitude && v.abs() > 90.0 {
                    continue;
                }
                let back = Dd(v).try_to_ddm(axis).and_then(Ddm::try_to_dd).unwrap().0;
                assert_close(back, v, 1e-12);
            }
        }
    }

    #[test]
    fn to_dms_decomposes_components() {
        let dms = Dd(40.712_775_3).try_to_dms(Axis::Latitude).unwrap();
        assert_eq!(dms.degrees, 40);
        assert_eq!(dms.minutes, 42);
        assert_close(dms.seconds, 45.991_08, 1e-3);
        assert_eq!(dms.hemisphere, Hemisphere::North);
    }

    #[test]
    fn negative_zero_is_positive_hemisphere() {
        assert_eq!(
            Dd(-0.0).try_to_dms(Axis::Latitude).unwrap().hemisphere,
            Hemisphere::North
        );
        assert_eq!(
            Dd(-0.0).try_to_ddm(Axis::Longitude).unwrap().hemisphere,
            Hemisphere::East
        );
        // A positive zero behaves identically.
        assert_eq!(
            Dd(0.0).try_to_dms(Axis::Longitude).unwrap().hemisphere,
            Hemisphere::East
        );
    }

    #[test]
    fn axis_selects_hemisphere_letters() {
        assert_eq!(
            Dd(10.0).try_to_dms(Axis::Latitude).unwrap().hemisphere,
            Hemisphere::North
        );
        assert_eq!(
            Dd(-10.0).try_to_dms(Axis::Latitude).unwrap().hemisphere,
            Hemisphere::South
        );
        assert_eq!(
            Dd(10.0).try_to_dms(Axis::Longitude).unwrap().hemisphere,
            Hemisphere::East
        );
        assert_eq!(
            Dd(-74.006).try_to_dms(Axis::Longitude).unwrap().hemisphere,
            Hemisphere::West
        );
    }

    #[test]
    fn dms_ddm_inter_conversions_preserve_hemisphere() {
        let dms = Dms {
            degrees: 40,
            minutes: 42,
            seconds: 46.0,
            hemisphere: Hemisphere::North,
        };
        let ddm = dms.try_to_ddm().unwrap();
        assert_eq!(ddm.degrees, 40);
        assert_close(ddm.minutes, 42.0 + 46.0 / 60.0, 1e-12);
        assert_eq!(ddm.hemisphere, Hemisphere::North);

        // Ddm -> Dms recovers whole minutes and seconds.
        let dms2 = ddm.try_to_dms().unwrap();
        assert_eq!(dms2.degrees, 40);
        assert_eq!(dms2.minutes, 42);
        assert_close(dms2.seconds, 46.0, 1e-9);
        assert_eq!(dms2.hemisphere, Hemisphere::North);
    }

    #[test]
    fn hemisphere_sign() {
        assert_close(Hemisphere::North.sign(), 1.0, 0.0);
        assert_close(Hemisphere::East.sign(), 1.0, 0.0);
        assert_close(Hemisphere::South.sign(), -1.0, 0.0);
        assert_close(Hemisphere::West.sign(), -1.0, 0.0);
    }

    #[test]
    fn wrap_longitude_is_half_open() {
        // The antimeridian normalizes to the western edge.
        assert_close(wrap_longitude(180.0), -180.0, 1e-12);
        assert_close(wrap_longitude(-180.0), -180.0, 1e-12);
        assert_close(wrap_longitude(540.0), -180.0, 1e-12);
        assert_close(wrap_longitude(190.0), -170.0, 1e-12);
        assert_close(wrap_longitude(-190.0), 170.0, 1e-12);
        assert_close(wrap_longitude(0.0), 0.0, 1e-12);
        assert_close(wrap_longitude(179.999), 179.999, 1e-12);
        // Already-in-range values are unchanged.
        assert_close(wrap_longitude(-73.5), -73.5, 1e-12);
    }

    #[test]
    fn clamp_latitude_includes_poles() {
        assert_close(clamp_latitude(90.0), 90.0, 1e-12);
        assert_close(clamp_latitude(-90.0), -90.0, 1e-12);
        assert_close(clamp_latitude(91.0), 90.0, 1e-12);
        assert_close(clamp_latitude(-90.5), -90.0, 1e-12);
        assert_close(clamp_latitude(45.0), 45.0, 1e-12);
    }

    #[test]
    fn normalize_degrees_is_zero_to_360() {
        assert_close(normalize_degrees(0.0), 0.0, 1e-12);
        assert_close(normalize_degrees(-0.0), 0.0, 1e-12);
        assert_close(normalize_degrees(360.0), 0.0, 1e-12);
        assert_close(normalize_degrees(-1.0), 359.0, 1e-12);
        assert_close(normalize_degrees(720.5), 0.5, 1e-12);
        assert_close(normalize_degrees(45.0), 45.0, 1e-12);
    }

    #[test]
    fn invalid_angle_records_are_rejected() {
        assert!(Dd(f64::NAN).try_to_dms(Axis::Latitude).is_err());
        assert!(Dd(91.0).try_to_ddm(Axis::Latitude).is_err());
        assert!(
            Dms {
                degrees: 40,
                minutes: 60,
                seconds: 0.0,
                hemisphere: Hemisphere::North,
            }
            .try_to_dd()
            .is_err()
        );
        assert!(
            Ddm {
                degrees: 180,
                minutes: 0.1,
                hemisphere: Hemisphere::East,
            }
            .try_to_dd()
            .is_err()
        );
    }
}
