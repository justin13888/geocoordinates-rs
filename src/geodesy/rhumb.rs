//! Rhumb-line (loxodrome / constant-bearing) routines on the IUGG mean sphere.
//!
//! [`rhumb_distance`], [`rhumb_bearing`], and [`rhumb_destination`] follow a
//! line of constant bearing, using the stretched (Mercator `ψ`) latitude, on a
//! sphere of radius `MEAN_EARTH_RADIUS_M`. They are implemented here directly
//! and re-exported from [`super::geodesic`].

use core::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use super::geodesic::{validate_bearing_and_distance, validate_pair, validate_wgs84};
use super::spherical::MEAN_EARTH_RADIUS_M;
use crate::angle::{normalize_degrees, wrap_longitude};
use crate::coord::{Coordinate, LatLon};
use crate::error::Result;
use crate::units::Length;

/// Rhumb-line (loxodrome / constant-bearing) distance, on a sphere.
pub fn rhumb_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    validate_pair(a, b)?;
    let phi1 = a.lat().to_radians();
    let phi2 = b.lat().to_radians();
    let d_phi = phi2 - phi1;
    let d_lon = shortest_d_lon(a, b);
    let q = east_west_scale(d_phi, stretched_lat_diff(phi1, phi2), phi1);
    let dist = (d_phi * d_phi + q * q * d_lon * d_lon).sqrt() * MEAN_EARTH_RADIUS_M;
    Ok(Length::from_meters(dist))
}

/// Rhumb-line (loxodrome / constant) bearing from `a` to `b`, in degrees
/// `[0, 360)`.
pub fn rhumb_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    validate_pair(a, b)?;
    let d_psi = stretched_lat_diff(a.lat().to_radians(), b.lat().to_radians());
    Ok(normalize_degrees(
        shortest_d_lon(a, b).atan2(d_psi).to_degrees(),
    ))
}

/// The point reached from `start` by traveling `distance` along a constant
/// `bearing_deg` rhumb line. Carries `start.crs()`.
pub fn rhumb_destination(
    start: &Coordinate,
    bearing_deg: f64,
    distance: Length,
) -> Result<Coordinate> {
    validate_wgs84(start)?;
    validate_bearing_and_distance(bearing_deg, distance)?;
    let theta = bearing_deg.to_radians();
    let delta = distance.meters() / MEAN_EARTH_RADIUS_M;
    let phi1 = start.lat().to_radians();
    let d_phi = delta * theta.cos();
    let mut phi2 = phi1 + d_phi;
    // Crossing a pole flips to the far side.
    let crossed_pole = phi2.abs() > FRAC_PI_2;
    if crossed_pole {
        // ±π − φ₂, taking the sign of the pole crossed (φ₂ ≠ 0 here).
        phi2 = PI.copysign(phi2) - phi2;
    }
    let q = east_west_scale(d_phi, stretched_lat_diff(phi1, phi2), phi1);
    let d_lon = (delta * theta.sin() / q).to_degrees();
    Coordinate::new(
        phi2.to_degrees(),
        wrap_longitude(start.lon() + d_lon + if crossed_pole { 180.0 } else { 0.0 }),
        start.crs(),
    )
}

/// The shorter signed longitude difference `b − a`, in radians (antimeridian-safe).
fn shortest_d_lon(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let mut d_lon = (b.lon() - a.lon()).to_radians();
    if d_lon.abs() > PI {
        d_lon -= d_lon.signum() * 2.0 * PI;
    }
    d_lon
}

/// The east-west scale `q = Δφ / Δψ` of a rhumb line, falling back to `cos φ₁`
/// when `|Δψ| ≤ 1e-12`: on an (almost) east-west line the ratio is 0/0, and
/// `cos φ₁` is its limit.
fn east_west_scale(d_phi: f64, d_psi: f64, phi1: f64) -> f64 {
    if d_psi.abs() > 1e-12 {
        d_phi / d_psi
    } else {
        phi1.cos()
    }
}

/// The stretched-latitude (Mercator `ψ`) difference between two latitudes.
fn stretched_lat_diff(phi1: f64, phi2: f64) -> f64 {
    ((phi2 / 2.0 + FRAC_PI_4).tan() / (phi1 / 2.0 + FRAC_PI_4).tan()).ln()
}

#[cfg(test)]
mod tests;
