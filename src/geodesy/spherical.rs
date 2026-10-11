//! Great-circle routines on the IUGG mean sphere.
//!
//! [`haversine_distance`], the cross/along-track pair
//! ([`cross_track_distance`] / [`along_track_distance`]), and the great-circle
//! [`intersection`] model the Earth as a sphere of radius
//! `MEAN_EARTH_RADIUS_M`, so they are approximate by construction. They are
//! implemented here directly and re-exported from [`super::geodesic`].

use core::f64::consts::PI;

use super::geodesic::{validate_pair, validate_wgs84};
use crate::angle::wrap_longitude;
use crate::coord::{Coordinate, LatLon};
use crate::error::{Error, Result};
use crate::units::Length;

/// IUGG mean Earth radius R1 = (2a + b) / 3, in meters, used by every
/// spherical routine: haversine, rhumb, cross/along-track, and intersection.
pub(super) const MEAN_EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Cheap spherical (haversine) distance — approximate, named for clarity.
///
/// Uses the IUGG mean Earth radius. The `atan2` form is numerically robust for
/// the full range of separations, including near-antipodal points.
pub fn haversine_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    validate_pair(a, b)?;
    let lat1 = a.lat().to_radians();
    let lat2 = b.lat().to_radians();
    let d_lat = (b.lat() - a.lat()).to_radians();
    let d_lon = (b.lon() - a.lon()).to_radians();

    let h = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * h.sqrt().atan2((1.0 - h).max(0.0).sqrt());
    Ok(Length::from_meters(MEAN_EARTH_RADIUS_M * c))
}

/// Signed perpendicular distance from `point` to the great-circle path
/// `start` → `end` (positive to the right, negative to the left).
pub fn cross_track_distance(
    point: &impl LatLon,
    start: &impl LatLon,
    end: &impl LatLon,
) -> Result<Length> {
    validate_pair(point, start)?;
    validate_pair(start, end)?;
    let d13 = angular_distance_rad(start, point);
    let bearing_diff = spherical_bearing_rad(start, point) - spherical_bearing_rad(start, end);
    let xt = (d13.sin() * bearing_diff.sin()).asin();
    Ok(Length::from_meters(xt * MEAN_EARTH_RADIUS_M))
}

/// Distance from `start` to the foot of the perpendicular from `point` onto the
/// great-circle path `start` → `end` (the along-track component; negative when
/// the foot lies behind `start`).
pub fn along_track_distance(
    point: &impl LatLon,
    start: &impl LatLon,
    end: &impl LatLon,
) -> Result<Length> {
    validate_pair(point, start)?;
    validate_pair(start, end)?;
    let d13 = angular_distance_rad(start, point);
    let bearing_diff = spherical_bearing_rad(start, point) - spherical_bearing_rad(start, end);
    let xt = (d13.sin() * bearing_diff.sin()).asin();
    let at = (d13.cos() / xt.cos()).clamp(-1.0, 1.0).acos();
    let signed = at * bearing_diff.cos().signum();
    Ok(Length::from_meters(signed * MEAN_EARTH_RADIUS_M))
}

/// Intersection of two great circles, each given by a point and an initial
/// bearing, on the mean sphere. `None` when the paths are parallel/coincident
/// or ambiguous. Carries `a.crs`.
pub fn intersection(
    a: &Coordinate,
    bearing_a_deg: f64,
    b: &Coordinate,
    bearing_b_deg: f64,
) -> Result<Option<Coordinate>> {
    validate_pair(a, b)?;
    validate_wgs84(a)?;
    if !bearing_a_deg.is_finite() || !bearing_b_deg.is_finite() {
        return Err(Error::InvalidValue {
            field: "bearing",
            detail: "must be finite".into(),
        });
    }
    let (phi1, lam1) = (a.lat.to_radians(), a.lon.to_radians());
    let (phi2, lam2) = (b.lat.to_radians(), b.lon.to_radians());
    let theta13 = bearing_a_deg.to_radians();
    let theta23 = bearing_b_deg.to_radians();
    let d_phi = phi2 - phi1;
    let d_lam = lam2 - lam1;

    let delta12 = 2.0
        * ((d_phi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (d_lam / 2.0).sin().powi(2))
            .sqrt()
            .asin();
    if delta12 < 1e-12 {
        return Ok(Some(*a)); // coincident endpoints
    }

    let theta_a = ((phi2.sin() - phi1.sin() * delta12.cos()) / (delta12.sin() * phi1.cos()))
        .clamp(-1.0, 1.0)
        .acos();
    let theta_b = ((phi1.sin() - phi2.sin() * delta12.cos()) / (delta12.sin() * phi2.cos()))
        .clamp(-1.0, 1.0)
        .acos();
    let (theta12, theta21) = if d_lam.sin() > 0.0 {
        (theta_a, 2.0 * PI - theta_b)
    } else {
        (2.0 * PI - theta_a, theta_b)
    };

    let alpha1 = theta13 - theta12;
    let alpha2 = theta21 - theta23;
    if alpha1.sin().abs() < 1e-12 && alpha2.sin().abs() < 1e-12 {
        return Ok(None); // same great circle — infinitely many intersections
    }
    if alpha1.sin() * alpha2.sin() < 0.0 {
        return Ok(None); // ambiguous (the antipodal intersection)
    }

    let cos_alpha3 = -alpha1.cos() * alpha2.cos() + alpha1.sin() * alpha2.sin() * delta12.cos();
    let delta13 = (delta12.sin() * alpha1.sin() * alpha2.sin())
        .atan2(alpha2.cos() + alpha1.cos() * cos_alpha3);
    let phi3 = (phi1.sin() * delta13.cos() + phi1.cos() * delta13.sin() * theta13.cos())
        .clamp(-1.0, 1.0)
        .asin();
    let d_lam13 =
        (theta13.sin() * delta13.sin() * phi1.cos()).atan2(delta13.cos() - phi1.sin() * phi3.sin());
    let lon3 = wrap_longitude((lam1 + d_lam13).to_degrees());
    Ok(Some(Coordinate::new(phi3.to_degrees(), lon3, a.crs)))
}

/// Initial bearing on a sphere, in radians.
fn spherical_bearing_rad(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let phi1 = a.lat().to_radians();
    let phi2 = b.lat().to_radians();
    let d_lon = (b.lon() - a.lon()).to_radians();
    let y = d_lon.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * d_lon.cos();
    y.atan2(x)
}

/// Angular distance on a sphere, in radians (haversine).
fn angular_distance_rad(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let phi1 = a.lat().to_radians();
    let phi2 = b.lat().to_radians();
    let d_phi = (b.lat() - a.lat()).to_radians();
    let d_lon = (b.lon() - a.lon()).to_radians();
    let h = (d_phi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (d_lon / 2.0).sin().powi(2);
    2.0 * h.sqrt().atan2((1.0 - h).max(0.0).sqrt())
}

#[cfg(test)]
mod tests;
