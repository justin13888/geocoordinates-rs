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
    /// trip back through [`Dd::from`] is exact. Rounding — and the 60″ carry it
    /// can imply — is the formatter's responsibility (see [`crate::format`]).
    #[must_use]
    pub fn to_dms(self, axis: Axis) -> Dms {
        let magnitude = self.0.abs();
        let degrees = magnitude.trunc();
        let rem_minutes = (magnitude - degrees) * 60.0;
        let minutes = rem_minutes.trunc();
        let seconds = (rem_minutes - minutes) * 60.0;
        Dms {
            degrees: degrees as u16,
            minutes: minutes as u8,
            seconds,
            hemisphere: hemisphere_for(axis, self.0),
        }
    }

    /// Convert to DDM for the given axis.
    #[must_use]
    pub fn to_ddm(self, axis: Axis) -> Ddm {
        let magnitude = self.0.abs();
        let degrees = magnitude.trunc();
        let minutes = (magnitude - degrees) * 60.0;
        Ddm {
            degrees: degrees as u16,
            minutes,
            hemisphere: hemisphere_for(axis, self.0),
        }
    }
}

impl Dms {
    /// Convert to degrees-decimal-minutes, preserving the hemisphere.
    #[must_use]
    pub fn to_ddm(self) -> Ddm {
        Ddm {
            degrees: self.degrees,
            minutes: f64::from(self.minutes) + self.seconds / 60.0,
            hemisphere: self.hemisphere,
        }
    }
}

impl Ddm {
    /// Convert to degrees-minutes-seconds, preserving the hemisphere.
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

impl From<Dms> for Dd {
    fn from(dms: Dms) -> Self {
        let magnitude =
            f64::from(dms.degrees) + f64::from(dms.minutes) / 60.0 + dms.seconds / 3600.0;
        Dd(magnitude * dms.hemisphere.sign())
    }
}

impl From<Ddm> for Dd {
    fn from(ddm: Ddm) -> Self {
        let magnitude = f64::from(ddm.degrees) + ddm.minutes / 60.0;
        Dd(magnitude * ddm.hemisphere.sign())
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

// --- Angle normalization helpers: released with the angles-and-units milestone (see ROADMAP.md) ---
/*
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
*/

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
                let back = Dd::from(Dd(v).to_dms(axis)).0;
                assert_close(back, v, 1e-12);
            }
        }
    }

    #[test]
    fn dd_ddm_round_trip_is_exact() {
        for &v in SAMPLES {
            for axis in [Axis::Latitude, Axis::Longitude] {
                let back = Dd::from(Dd(v).to_ddm(axis)).0;
                assert_close(back, v, 1e-12);
            }
        }
    }

    #[test]
    fn to_dms_decomposes_components() {
        let dms = Dd(40.712_775_3).to_dms(Axis::Latitude);
        assert_eq!(dms.degrees, 40);
        assert_eq!(dms.minutes, 42);
        assert_close(dms.seconds, 45.991_08, 1e-3);
        assert_eq!(dms.hemisphere, Hemisphere::North);
    }

    #[test]
    fn negative_zero_is_positive_hemisphere() {
        assert_eq!(
            Dd(-0.0).to_dms(Axis::Latitude).hemisphere,
            Hemisphere::North
        );
        assert_eq!(
            Dd(-0.0).to_ddm(Axis::Longitude).hemisphere,
            Hemisphere::East
        );
        // A positive zero behaves identically.
        assert_eq!(Dd(0.0).to_dms(Axis::Longitude).hemisphere, Hemisphere::East);
    }

    #[test]
    fn axis_selects_hemisphere_letters() {
        assert_eq!(
            Dd(10.0).to_dms(Axis::Latitude).hemisphere,
            Hemisphere::North
        );
        assert_eq!(
            Dd(-10.0).to_dms(Axis::Latitude).hemisphere,
            Hemisphere::South
        );
        assert_eq!(
            Dd(10.0).to_dms(Axis::Longitude).hemisphere,
            Hemisphere::East
        );
        assert_eq!(
            Dd(-74.006).to_dms(Axis::Longitude).hemisphere,
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
        let ddm = dms.to_ddm();
        assert_eq!(ddm.degrees, 40);
        assert_close(ddm.minutes, 42.0 + 46.0 / 60.0, 1e-12);
        assert_eq!(ddm.hemisphere, Hemisphere::North);

        // Ddm -> Dms recovers whole minutes and seconds.
        let dms2 = ddm.to_dms();
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
}
