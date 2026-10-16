//! Ellipsoidal (Karney) distance, bearing, and geodesic position problems,
//! re-exported flat from [`geodesy`](super); the module docs there describe
//! the CRS, sphere, and pole conventions.

use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

use super::ecef::ellipsoid_of;
#[cfg(doc)]
use super::ellipsoid::Ellipsoid;
use crate::angle::{normalize_degrees, wrap_longitude};
use crate::coord::{Coordinate, Crs, LatLon};
use crate::error::{Error, Result};
use crate::units::Length;

/// Ellipsoidal (Karney geodesic) distance between two points, measured on the
/// ellipsoid of their shared [`Crs`].
///
/// Preferred over Vincenty, which fails to converge for near-antipodal points.
/// Karney's algorithm is accurate to round-off (nanometers), so the result is
/// exact for practical purposes on WGS-84 and on each classic datum's own
/// ellipsoid (NAD27 on Clarke 1866, Tokyo on Bessel 1841, Pulkovo-1942 on
/// Krasovsky 1940).
///
/// # Errors
///
/// [`Error::OutOfRange`] for an out-of-range position, and
/// [`Error::CrsMismatch`] when the points' CRSs differ or are GCJ-02 / BD-09
/// (obfuscated renderings with no ellipsoid; convert them to WGS-84 first).
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, china::Wgs84, geodesy::geodesic_distance};
///
/// // Equator to north pole: a quarter meridian of WGS 84.
/// let (equator, pole) = (Wgs84::new(0.0, 0.0)?, Wgs84::new(90.0, 0.0)?);
/// let quarter = geodesic_distance(&equator, &pole)?.meters();
/// assert!((quarter - 10_001_965.729).abs() < 1e-3);
/// // Mixed datums are refused rather than silently measured.
/// let nad27 = Coordinate::new(90.0, 0.0, Crs::Nad27)?;
/// assert!(matches!(
///     geodesic_distance(&equator, &nad27),
///     Err(Error::CrsMismatch { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn geodesic_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    let g = geodesic_for_pair(a, b)?;
    let s12: f64 = g.inverse(a.lat(), a.lon(), b.lat(), b.lon());
    Ok(Length::from_meters(s12))
}

/// Initial bearing (forward azimuth) of the geodesic from `a` to `b`, in
/// degrees `[0, 360)`, on the ellipsoid of their shared [`Crs`].
///
/// From a pole the bearing is taken against the meridian of `a`'s longitude
/// (see the [module docs](crate::geodesy)).
///
/// # Errors
///
/// As for [`geodesic_distance`], and [`Error::InvalidValue`]
/// (`field: "points"`) when `a` and `b` are the same place, which has no
/// bearing.
///
/// ```
/// use geocoordinates::{Coordinate, Error, geodesy::initial_bearing};
///
/// let a = Coordinate::wgs84(0.0, 0.0)?;
/// // Due west across the equator wraps to 270°, never −90°.
/// let west = initial_bearing(&a, &Coordinate::wgs84(0.0, -10.0)?)?;
/// assert!((west - 270.0).abs() < 1e-9);
/// assert!(matches!(
///     initial_bearing(&a, &a),
///     Err(Error::InvalidValue { field: "points", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn initial_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    let (azi1, _) = azimuths(a, b)?;
    Ok(normalize_degrees(azi1))
}

/// Final bearing (azimuth on arrival) of the geodesic from `a` to `b`, in
/// degrees `[0, 360)`, on the ellipsoid of their shared [`Crs`]. Differs from
/// [`initial_bearing`] on the ellipsoid.
///
/// Arriving at a pole, the bearing is taken against the meridian of `b`'s
/// longitude (see the [module docs](crate::geodesy)).
///
/// # Errors
///
/// As for [`initial_bearing`].
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, geodesy::{final_bearing, initial_bearing}};
///
/// // A geodesic heading north-east bends east as it climbs.
/// let (a, b) = (Coordinate::wgs84(40.0, -74.0)?, Coordinate::wgs84(51.5, 0.0)?);
/// assert!(final_bearing(&a, &b)? > initial_bearing(&a, &b)? + 50.0);
/// let gcj = Coordinate::new(39.9, 116.4, Crs::Gcj02)?;
/// assert!(matches!(final_bearing(&gcj, &gcj), Err(Error::CrsMismatch { .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn final_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    let (_, azi2) = azimuths(a, b)?;
    Ok(normalize_degrees(azi2))
}

/// Direct geodesic problem: the point reached from `start` by traveling
/// `distance` along `bearing_deg` (exact, Karney), on the ellipsoid of
/// `start.crs()`. Returns a coordinate in `start.crs()` with no height.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when `start` is GCJ-02 or BD-09, and
/// [`Error::InvalidValue`] for a non-finite bearing or a non-finite or
/// negative distance.
///
/// ```
/// use geocoordinates::{Coordinate, Error, Length, geodesy::destination};
///
/// // East from just west of the antimeridian lands wrapped into [-180, 180).
/// let fiji = Coordinate::wgs84(-17.0, 179.9)?;
/// let across = destination(&fiji, 90.0, Length::from_meters(50_000.0))?;
/// assert!(across.lon() < -179.0);
/// assert!(matches!(
///     destination(&fiji, 90.0, Length::from_meters(-1.0)),
///     Err(Error::InvalidValue { field: "distance", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn destination(start: &Coordinate, bearing_deg: f64, distance: Length) -> Result<Coordinate> {
    let g = geodesic_for(start.crs())?;
    validate_bearing_and_distance(bearing_deg, distance)?;
    let (lat2, lon2): (f64, f64) =
        g.direct(start.lat(), start.lon(), bearing_deg, distance.meters());
    finish(lat2, lon2, start.crs())
}

/// The geodesic midpoint between `a` and `b`, on the ellipsoid of their shared
/// CRS. Returns a coordinate in that CRS with no height.
///
/// # Errors
///
/// As for [`intermediate`].
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error, geodesy::midpoint};
///
/// // The shorter way round, across the antimeridian.
/// let m = midpoint(&Coordinate::wgs84(0.0, 170.0)?, &Coordinate::wgs84(0.0, -170.0)?)?;
/// assert!((m.lon().abs() - 180.0).abs() < 1e-9);
/// let tokyo = Coordinate::new(35.0, 139.0, Crs::Tokyo)?;
/// assert!(matches!(
///     midpoint(&Coordinate::wgs84(35.0, 139.0)?, &tokyo),
///     Err(Error::CrsMismatch { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn midpoint(a: &Coordinate, b: &Coordinate) -> Result<Coordinate> {
    intermediate(a, b, 0.5)
}

/// The point a `fraction` (0.0 → `a`, 1.0 → `b`) of the way along the geodesic
/// from `a` to `b`, on the ellipsoid of their shared CRS. Returns a coordinate
/// in that CRS with no height.
///
/// # Errors
///
/// [`Error::CrsMismatch`] as for [`geodesic_distance`], and
/// [`Error::InvalidValue`] when `fraction` is not finite or not in `[0, 1]`.
///
/// ```
/// use geocoordinates::{Coordinate, Error, geodesy::intermediate};
///
/// // A quarter of the way along the equator from 0° to 40° E.
/// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 40.0)?);
/// let p = intermediate(&a, &b, 0.25)?;
/// assert!(p.lat().abs() < 1e-9 && (p.lon() - 10.0).abs() < 1e-9);
/// assert!(matches!(
///     intermediate(&a, &b, 1.5),
///     Err(Error::InvalidValue { field: "fraction", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn intermediate(a: &Coordinate, b: &Coordinate, fraction: f64) -> Result<Coordinate> {
    let g = geodesic_for_pair(a, b)?;
    if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
        return Err(Error::InvalidValue {
            field: "fraction",
            detail: "must be finite and in [0, 1]".into(),
        });
    }
    // The 4-tuple is (s12, azi1, azi2, a12); travel `fraction · s12` along azi1.
    let (s12, azi1, _, _): (f64, f64, f64, f64) = g.inverse(a.lat(), a.lon(), b.lat(), b.lon());
    let (lat, lon): (f64, f64) = g.direct(a.lat(), a.lon(), azi1, s12 * fraction);
    finish(lat, lon, a.crs())
}

/// The forward and arrival azimuths `(azi1, azi2)` of the geodesic from `a` to
/// `b`, in degrees, rejecting a coincident pair (zero geodesic length).
fn azimuths(a: &impl LatLon, b: &impl LatLon) -> Result<(f64, f64)> {
    let g = geodesic_for_pair(a, b)?;
    // The 4-tuple is (s12, azi1, azi2, a12).
    let (s12, azi1, azi2, _): (f64, f64, f64, f64) = g.inverse(a.lat(), a.lon(), b.lat(), b.lon());
    if s12 == 0.0 {
        return Err(Error::InvalidValue {
            field: "points",
            detail: "coincident points have no bearing".into(),
        });
    }
    Ok((azi1, azi2))
}

/// The Karney geodesic on the ellipsoid of `crs`, or [`Error::CrsMismatch`]
/// (expecting WGS-84) for GCJ-02 and BD-09, which have none.
fn geodesic_for(crs: Crs) -> Result<Geodesic> {
    let ellipsoid = ellipsoid_of(crs)?;
    Ok(Geodesic::new(
        ellipsoid.semi_major_m(),
        ellipsoid.flattening(),
    ))
}

/// Validates a pair (each in range, one shared CRS) and returns the geodesic
/// on that CRS's ellipsoid.
fn geodesic_for_pair(a: &impl LatLon, b: &impl LatLon) -> Result<Geodesic> {
    validate_pair(a, b)?;
    geodesic_for(a.crs())
}

/// Every geodesic and great-circle producer's exit: wraps `lon_deg` into
/// `[-180, 180)` and validates the result, so no producer returns an
/// out-of-range or non-finite coordinate. The result has no height.
pub(super) fn finish(lat_deg: f64, lon_deg: f64, crs: Crs) -> Result<Coordinate> {
    Coordinate::new(lat_deg, wrap_longitude(lon_deg), crs)
}

fn validate_position(position: &impl LatLon) -> Result<()> {
    Coordinate::new(position.lat(), position.lon(), position.crs()).map(|_| ())
}

pub(super) fn validate_pair(a: &impl LatLon, b: &impl LatLon) -> Result<()> {
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

pub(super) fn validate_bearing_and_distance(bearing_deg: f64, distance: Length) -> Result<()> {
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

// Polygon and line ops — ellipsoidal area/perimeter, point-in-polygon, centroid,
// convex hull, buffers, bounding boxes, line densification, and simplification —
// are out of scope for this crate: use the `geo` crate directly for them. This
// module and its `spherical` / `rhumb` siblings own only point-to-point ops.

#[cfg(test)]
mod tests;
