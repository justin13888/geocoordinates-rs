//! Distance, bearing, and geodesic problems.
//!
//! Ellipsoidal (Karney, on WGS-84) [`geodesic_distance`], [`initial_bearing`] /
//! [`final_bearing`], and the position producers ([`destination`], [`midpoint`],
//! [`intermediate`], [`intersection`]) delegate to
//! [`geographiclib-rs`](https://docs.rs/geographiclib-rs). The spherical
//! [`haversine_distance`], rhumb/loxodrome ([`rhumb_distance`] /
//! [`rhumb_bearing`] / [`rhumb_destination`]), and cross/along-track
//! ([`cross_track_distance`] / [`along_track_distance`]) routines are
//! implemented here directly.
//!
//! The model is explicit in the function name, so the accuracy is obvious
//! without reading docs. Measurement functions (distances, bearings) take any
//! [`LatLon`], so they work on `Coordinate` and the per-datum newtypes alike;
//! every multi-point operation rejects mixed CRS values. Producer functions
//! take `&Coordinate`, require WGS-84 inputs, and return WGS-84 coordinates, so
//! an obfuscated GCJ-02 / BD-09 position cannot silently enter true-datum math.

use core::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

use crate::angle::{normalize_degrees, wrap_longitude};
use crate::coord::{Coordinate, Crs, LatLon};
use crate::error::{Error, Result};
use crate::units::Length;

/// IUGG mean Earth radius R1 = (2a + b) / 3, in meters, used by the spherical
/// (rhumb, cross/along-track, intersection) routines.
const MEAN_EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Ellipsoidal (Karney geodesic) distance between two points, measured on the
/// **WGS-84** ellipsoid whatever their shared [`Crs`].
///
/// Preferred over Vincenty, which fails to converge for near-antipodal points.
/// Karney's algorithm is accurate to round-off (nanometers) on WGS-84, so for
/// two WGS-84 points the result is exact for practical purposes. For other
/// systems it is not:
///
/// - **Classic datums** (NAD27, Tokyo, Pulkovo-1942) sit on other ellipsoids,
///   and measuring them on WGS-84 is off by up to about 1.3e-4 of the distance
///   (≈ 0.13 m per km, for Tokyo's Bessel 1841).
/// - **GCJ-02 / BD-09** are offset from true positions by an amount that
///   varies across the map, so the result carries the difference of the two
///   points' offsets. Convert both to WGS-84 first for a true distance.
pub fn geodesic_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    validate_pair(a, b)?;
    let s12: f64 = Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Ok(Length::from_meters(s12))
}

/// Cheap spherical (haversine) distance — approximate, named for clarity.
///
/// Uses the IUGG mean Earth radius. The `atan2` form is numerically robust for
/// the full range of separations, including near-antipodal points.
pub fn haversine_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    validate_pair(a, b)?;
    /// IUGG mean Earth radius R1 = (2a + b) / 3, in meters.
    const MEAN_EARTH_RADIUS_M: f64 = 6_371_008.8;

    let lat1 = a.lat().to_radians();
    let lat2 = b.lat().to_radians();
    let d_lat = (b.lat() - a.lat()).to_radians();
    let d_lon = (b.lon() - a.lon()).to_radians();

    let h = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * h.sqrt().atan2((1.0 - h).max(0.0).sqrt());
    Ok(Length::from_meters(MEAN_EARTH_RADIUS_M * c))
}

/// Initial bearing (forward azimuth) from `a` to `b`, in degrees `[0, 360)`.
pub fn initial_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    validate_pair(a, b)?;
    // geographiclib-rs returns the 3-tuple as (azi1, azi2, a12).
    let (azi1, _, _): (f64, f64, f64) =
        Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Ok(normalize_degrees(azi1))
}

/// Final bearing (azimuth on arrival) of the geodesic from `a` to `b`, in
/// degrees `[0, 360)`. Differs from [`initial_bearing`] on the ellipsoid.
pub fn final_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    validate_pair(a, b)?;
    // The 3-tuple is (azi1, azi2, a12); the final azimuth is the second element.
    let (_, azi2, _): (f64, f64, f64) =
        Geodesic::wgs84().inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Ok(normalize_degrees(azi2))
}

/// Direct geodesic problem: the point reached from `start` by traveling
/// `distance` along `bearing_deg` (exact, Karney). Carries `start.crs`.
pub fn destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Result<Coordinate> {
    validate_wgs84(start)?;
    validate_bearing_and_distance(bearing_deg, distance)?;
    let (lat2, lon2): (f64, f64) =
        Geodesic::wgs84().direct(start.lat, start.lon, bearing_deg, distance.meters());
    Ok(Coordinate::new(lat2, lon2, start.crs))
}

/// The geodesic midpoint between `a` and `b`. Carries `a.crs`.
pub fn midpoint(a: &Coordinate, b: &Coordinate) -> Result<Coordinate> {
    intermediate(a, b, 0.5)
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the geodesic
/// from `a` to `b`. Carries `a.crs`.
pub fn intermediate(a: &Coordinate, b: &Coordinate, fraction: f64) -> Result<Coordinate> {
    validate_pair(a, b)?;
    validate_wgs84(a)?;
    if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
        return Err(Error::InvalidValue {
            field: "fraction",
            detail: "must be finite and in [0, 1]".into(),
        });
    }
    let g = Geodesic::wgs84();
    // The 4-tuple is (s12, azi1, azi2, a12); travel `fraction · s12` along azi1.
    let (s12, azi1, _, _): (f64, f64, f64, f64) = g.inverse(a.lat, a.lon, b.lat, b.lon);
    let (lat, lon): (f64, f64) = g.direct(a.lat, a.lon, azi1, s12 * fraction);
    Ok(Coordinate::new(lat, lon, a.crs))
}

/// Rhumb-line (loxodrome / constant-bearing) distance, on a sphere.
pub fn rhumb_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    validate_pair(a, b)?;
    let phi1 = a.lat().to_radians();
    let phi2 = b.lat().to_radians();
    let d_phi = phi2 - phi1;
    let d_lon = shortest_d_lon(a, b);
    let d_psi = stretched_lat_diff(phi1, phi2);
    let q = if d_psi.abs() > 1e-12 {
        d_phi / d_psi
    } else {
        phi1.cos()
    };
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
/// `bearing_deg` rhumb line. Carries `start.crs`.
pub fn rhumb_destination(
    start: &Coordinate,
    bearing_deg: f64,
    distance: Length,
) -> Result<Coordinate> {
    validate_wgs84(start)?;
    validate_bearing_and_distance(bearing_deg, distance)?;
    let theta = bearing_deg.to_radians();
    let delta = distance.meters() / MEAN_EARTH_RADIUS_M;
    let phi1 = start.lat.to_radians();
    let d_phi = delta * theta.cos();
    let mut phi2 = phi1 + d_phi;
    // Crossing a pole flips to the far side.
    let crossed_pole = phi2.abs() > FRAC_PI_2;
    if crossed_pole {
        phi2 = if phi2 > 0.0 { PI - phi2 } else { -PI - phi2 };
    }
    let d_psi = stretched_lat_diff(phi1, phi2);
    let q = if d_psi.abs() > 1e-12 {
        d_phi / d_psi
    } else {
        phi1.cos()
    };
    let d_lon = (delta * theta.sin() / q).to_degrees();
    Ok(Coordinate::new(
        phi2.to_degrees(),
        wrap_longitude(start.lon + d_lon + if crossed_pole { 180.0 } else { 0.0 }),
        start.crs,
    ))
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
/// bearing. `None` when the paths are parallel/coincident or ambiguous. Carries
/// `a.crs`.
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

fn validate_position(position: &impl LatLon) -> Result<()> {
    Coordinate::new(position.lat(), position.lon(), position.crs()).validate()
}

fn validate_pair(a: &impl LatLon, b: &impl LatLon) -> Result<()> {
    validate_position(a)?;
    validate_position(b)?;
    if a.crs() != b.crs() {
        return Err(Error::CrsMismatch {
            expected: a.crs(),
            found: b.crs(),
        });
    }
    Ok(())
}

fn validate_wgs84(position: &Coordinate) -> Result<()> {
    position.validate()?;
    if position.crs != Crs::Wgs84 {
        return Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: position.crs,
        });
    }
    Ok(())
}

fn validate_bearing_and_distance(bearing_deg: f64, distance: Length) -> Result<()> {
    if !bearing_deg.is_finite() {
        return Err(Error::InvalidValue {
            field: "bearing",
            detail: "must be finite".into(),
        });
    }
    if !distance.meters().is_finite() || distance.meters() < 0.0 {
        return Err(Error::InvalidValue {
            field: "distance",
            detail: "must be finite and nonnegative".into(),
        });
    }
    Ok(())
}

/// The shorter signed longitude difference `b − a`, in radians (antimeridian-safe).
fn shortest_d_lon(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let mut d_lon = (b.lon() - a.lon()).to_radians();
    if d_lon.abs() > PI {
        d_lon -= d_lon.signum() * 2.0 * PI;
    }
    d_lon
}

/// The stretched-latitude (Mercator `ψ`) difference between two latitudes.
fn stretched_lat_diff(phi1: f64, phi2: f64) -> f64 {
    ((phi2 / 2.0 + FRAC_PI_4).tan() / (phi1 / 2.0 + FRAC_PI_4).tan()).ln()
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

// Polygon and line ops — ellipsoidal area/perimeter, point-in-polygon, centroid,
// convex hull, buffers, bounding boxes, line densification, and simplification —
// are out of scope for this crate: use the `geo` crate directly for them. This
// module owns only the point-to-point geodesic ops above.

#[cfg(test)]
mod tests;
