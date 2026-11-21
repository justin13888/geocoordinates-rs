//! Angle normalization: longitude wrap, latitude clamp, and degree
//! normalization.

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
