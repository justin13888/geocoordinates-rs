//! Rhumb-line (loxodrome / constant-bearing) routines on the datum ellipsoid.
//!
//! [`rhumb_distance`], [`rhumb_bearing`], and [`rhumb_destination`] follow a
//! line of constant bearing on the ellipsoid of the inputs' own [`Crs`]
//! ([`Ellipsoid::for_crs`]): a Tokyo-datum pair is measured on Bessel 1841, a
//! NAD27 pair on Clarke 1866. GCJ-02 and BD-09 are obfuscated renderings with
//! no ellipsoid, so each of these rejects them with [`Error::CrsMismatch`];
//! convert them to WGS-84 first. They are implemented here directly and
//! re-exported from [`super::geodesic`].
//!
//! A rhumb line is straight in isometric latitude `ψ` and longitude `λ`, so its
//! bearing is `atan2(Δλ, Δψ)`, and its length is the meridian arc it climbs,
//! `ΔM`, divided by the cosine of that bearing. The isometric latitude is
//! closed-form; the meridian arc and its inverse use the 4th-order Krüger
//! `n`-series that UTM uses, so the results agree with GeographicLib's
//! `Rhumb` (Karney 2014) to well under a millimeter for any line that stays
//! clear of the poles. They are not exact.
//!
//! **Poles.** A rhumb line of any bearing other than due north or south
//! spirals into a pole without ever reaching it, so:
//!
//! - measured to or from a pole, a rhumb line is the meridian:
//!   [`rhumb_distance`] is the meridian arc, and [`rhumb_bearing`] is 0°
//!   (heading for a pole north of the start) or 180°, whatever the longitudes;
//! - [`rhumb_destination`] rejects a non-meridional bearing whose `distance`
//!   would carry it to or past a pole (to within about 6 µm), and any
//!   non-meridional bearing from a pole, with [`Error::InvalidValue`];
//! - a meridional (0° or 180°) [`rhumb_destination`] passes over a pole and
//!   carries on down the far meridian, `start.lon() + 180°`, as many times as
//!   its `distance` reaches.

use core::f64::consts::{FRAC_PI_2, PI};

use super::ecef::ellipsoid_of;
#[cfg(doc)]
use super::ellipsoid::Ellipsoid;
use super::geodesic::{finish, validate_bearing_and_distance, validate_pair};
use super::series::KrugerSeries;
use crate::angle::{clamp_latitude, normalize_degrees};
use crate::coord::{Coordinate, Crs, LatLon};
use crate::error::{Error, Result};
use crate::units::Length;

/// How close, in rectifying latitude, a non-meridional rhumb line may come to
/// a pole before it counts as reaching it: 1e-12 rad, about 6 µm of meridian.
/// The meridian-arc series is good to well under that, so a line ending
/// within it is indistinguishable from one ending on the pole.
const POLE_TOLERANCE: f64 = 1e-12;

/// Rhumb-line (loxodrome / constant-bearing) distance from `a` to `b`, on the
/// ellipsoid of their shared [`Crs`]. Across the antimeridian the line takes
/// the shorter way round.
///
/// Not exact: the meridian arc uses the 4th-order Krüger `n`-series, which
/// agrees with GeographicLib's `Rhumb` (Karney 2014) to well under a
/// millimeter for any line that stays clear of the poles. To or from a pole
/// the line is the meridian, so the
/// distance is the meridian arc between the two latitudes.
///
/// # Errors
///
/// [`Error::OutOfRange`] for an out-of-range position, and
/// [`Error::CrsMismatch`] when the points' CRSs differ or are GCJ-02 / BD-09
/// (obfuscated renderings with no ellipsoid; convert them to WGS-84 first).
pub fn rhumb_distance(a: &impl LatLon, b: &impl LatLon) -> Result<Length> {
    let series = series_for_pair(a, b)?;
    let (phi1, phi2) = (a.lat().to_radians(), b.lat().to_radians());
    if touches_a_pole(a, b) {
        let d_mu = rectifying_latitude(&series, phi2) - rectifying_latitude(&series, phi1);
        return Ok(Length::from_meters(series.rect_a * d_mu.abs()));
    }
    let (psi1, psi2) = (
        series.isometric_latitude(phi1),
        series.isometric_latitude(phi2),
    );
    let q = meridian_scale(&series, psi1, psi2);
    Ok(Length::from_meters(
        q * (psi2 - psi1).hypot(shortest_d_lon(a, b)),
    ))
}

/// Rhumb-line (loxodrome / constant) bearing from `a` to `b`, in degrees
/// `[0, 360)`, on the ellipsoid of their shared [`Crs`].
///
/// To a pole the bearing is 0° (north pole) or 180° (south pole); from a pole
/// it is 180° (north pole) or 0° (south pole), whatever the longitudes,
/// because only a meridian reaches a pole.
///
/// # Errors
///
/// As for [`rhumb_distance`], and [`Error::InvalidValue`] (`field: "points"`)
/// for coincident points (including one pole named at two longitudes), which
/// have no bearing, as for [`initial_bearing`](super::geodesic::initial_bearing).
pub fn rhumb_bearing(a: &impl LatLon, b: &impl LatLon) -> Result<f64> {
    let series = series_for_pair(a, b)?;
    if a.lat() == b.lat() && (a.lat().abs() == 90.0 || shortest_d_lon(a, b) == 0.0) {
        return Err(Error::InvalidValue {
            field: "points",
            detail: "coincident points have no bearing".into(),
        });
    }
    if touches_a_pole(a, b) {
        // Not coincident, so north to the north pole or from the south pole.
        return Ok(if b.lat() == 90.0 || a.lat() == -90.0 {
            0.0
        } else {
            180.0
        });
    }
    let d_psi = series.isometric_latitude(b.lat().to_radians())
        - series.isometric_latitude(a.lat().to_radians());
    Ok(normalize_degrees(
        shortest_d_lon(a, b).atan2(d_psi).to_degrees(),
    ))
}

/// The point reached from `start` by traveling `distance` along a constant
/// `bearing_deg` rhumb line, on the ellipsoid of `start.crs()`. Returns a
/// coordinate in `start.crs()` with no height.
///
/// Not exact: the meridian arc and its inverse use the 4th-order Krüger
/// `n`-series, which agrees with GeographicLib's `Rhumb` (Karney 2014) to well
/// under a millimeter for any line that stays clear of the poles. A meridional line (bearing 0° or 180°) passes over a
/// pole onto the far meridian, `start.lon() + 180°`.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when `start` is GCJ-02 or BD-09;
/// [`Error::InvalidValue`] for a non-finite bearing or a non-finite or
/// negative distance; [`Error::InvalidValue`] (`field: "bearing"`) for a
/// bearing other than 0° or 180° from a pole; and [`Error::InvalidValue`]
/// (`field: "distance"`) when a bearing other than 0° or 180° would carry the
/// line to or past a pole, which it only approaches in the limit.
pub fn rhumb_destination(
    start: &Coordinate,
    bearing_deg: f64,
    distance: Length,
) -> Result<Coordinate> {
    let series = series_for(start.crs())?;
    validate_bearing_and_distance(bearing_deg, distance)?;
    let bearing = normalize_degrees(bearing_deg);
    let meters = distance.meters();
    let phi1 = start.lat().to_radians();
    let mu1 = rectifying_latitude(&series, phi1);

    if bearing == 0.0 || bearing == 180.0 {
        // Travel northward, in a frame mirrored for a southward line.
        let north = bearing == 0.0;
        let from = if north { mu1 } else { -mu1 };
        let (mu2, over_a_pole) = travel_north(from + meters / series.rect_a);
        let mu2 = if north { mu2 } else { -mu2 };
        let half_turn = if over_a_pole { 180.0 } else { 0.0 };
        return finish(
            latitude_deg_of(&series, mu2),
            start.lon() + half_turn,
            start.crs(),
        );
    }
    if start.lat().abs() == 90.0 {
        return Err(Error::InvalidValue {
            field: "bearing",
            detail: "only a meridional (0° or 180°) rhumb line leaves a pole".into(),
        });
    }
    let (sin_a, cos_a) = bearing.to_radians().sin_cos();
    let mu2 = mu1 + meters * cos_a / series.rect_a;
    // Only a line heading poleward can reach a pole: a start already within
    // the tolerance band may still leave it southward, or stay put.
    if mu2.abs() >= FRAC_PI_2 - POLE_TOLERANCE && mu2.abs() > mu1.abs() {
        return Err(Error::InvalidValue {
            field: "distance",
            detail: "a rhumb line that is not a meridian spirals into a pole without \
                     reaching it; this distance would carry it to or past the pole"
                .into(),
        });
    }
    let lat2 = latitude_deg_of(&series, mu2);
    let q = meridian_scale(
        &series,
        series.isometric_latitude(phi1),
        series.isometric_latitude(lat2.to_radians()),
    );
    let d_lon = (meters * sin_a / q).to_degrees();
    finish(lat2, start.lon() + d_lon, start.crs())
}

/// The Krüger series of `crs`'s ellipsoid, or [`Error::CrsMismatch`] for
/// GCJ-02 and BD-09, which have none.
fn series_for(crs: Crs) -> Result<KrugerSeries> {
    Ok(KrugerSeries::new(ellipsoid_of(crs)?))
}

/// Validates a pair (each in range, one shared CRS) and returns the series of
/// that CRS's ellipsoid.
fn series_for_pair(a: &impl LatLon, b: &impl LatLon) -> Result<KrugerSeries> {
    validate_pair(a, b)?;
    series_for(a.crs())
}

/// Whether either end of the pair is a pole, where only a meridian arrives.
fn touches_a_pole(a: &impl LatLon, b: &impl LatLon) -> bool {
    a.lat().abs() == 90.0 || b.lat().abs() == 90.0
}

/// Rectifying latitude `μ` of geodetic latitude `phi` (radians).
fn rectifying_latitude(series: &KrugerSeries, phi: f64) -> f64 {
    series.rectifying_from_conformal(series.isometric_latitude(phi).sinh().atan())
}

/// Geodetic latitude, in degrees, of rectifying latitude `mu` (radians), with
/// the series' round-off at a pole clamped back into `[-90, 90]`.
fn latitude_deg_of(series: &KrugerSeries, mu: f64) -> f64 {
    let chi = series.conformal_from_rectifying(mu);
    clamp_latitude(series.geodetic_from_conformal(chi).to_degrees())
}

/// Where a northward meridian that has climbed to rectifying latitude `mu`
/// (from a start in `[-π/2, π/2]`) ends up: its rectifying latitude in
/// `[-π/2, π/2]`, and whether it is on the far meridian, which it is after an
/// odd number of poles passed. Passing the north pole turns it south and the
/// south pole north again, so a full turn comes home. A line that ends
/// exactly on a pole has not passed it, and keeps the meridian it arrived on.
fn travel_north(mu: f64) -> (f64, bool) {
    // Poles strictly passed (none while short of the north pole), and the arc
    // run since the last of them (or since the south pole, if none).
    let past = mu - FRAC_PI_2;
    let poles = (past / PI).ceil().max(0.0);
    let since = past - (poles - 1.0) * PI;
    if poles % 2.0 == 1.0 {
        (FRAC_PI_2 - since, true)
    } else {
        (since - FRAC_PI_2, false)
    }
}

/// The east-west scale `q = ΔM / Δψ` of a rhumb line between isometric
/// latitudes `psi1` and `psi2`: meters of meridian arc per unit of isometric
/// latitude, and so also meters per radian of longitude along the line. Both
/// differences are taken in closed form (no subtraction of nearly equal
/// arcs), so `q` stays accurate on an almost east-west line; on an exactly
/// east-west one it is its limit, the radius of the parallel `ν·cos φ`.
fn meridian_scale(series: &KrugerSeries, psi1: f64, psi2: f64) -> f64 {
    let (t1, t2) = (psi1.sinh(), psi2.sinh());
    let d_psi = psi2 - psi1;
    if d_psi == 0.0 {
        // dM/dψ = A · dμ/dχ · dχ/dψ, with dχ/dψ = sech ψ = cos χ.
        let chi = t1.atan();
        let mut d_mu_d_chi = 1.0;
        for (j, alpha) in (1..=4u8).zip(&series.alpha) {
            let j = f64::from(j);
            d_mu_d_chi += 2.0 * j * alpha * (2.0 * j * chi).cos();
        }
        return series.rect_a * chi.cos() * d_mu_d_chi;
    }
    let (chi1, chi2) = (t1.atan(), t2.atan());
    // tan Δχ = (t₂ − t₁) / (1 + t₁t₂), with sinh ψ₂ − sinh ψ₁ in product form.
    let d_chi = (2.0 * (0.5 * (psi1 + psi2)).cosh() * (0.5 * d_psi).sinh()).atan2(1.0 + t1 * t2);
    // Δμ = Δχ + Σ αⱼ (sin 2jχ₂ − sin 2jχ₁), each difference in product form.
    let mut d_mu = d_chi;
    for (j, alpha) in (1..=4u8).zip(&series.alpha) {
        let j = f64::from(j);
        d_mu += 2.0 * alpha * (j * (chi1 + chi2)).cos() * (j * d_chi).sin();
    }
    series.rect_a * d_mu / d_psi
}

/// The shorter signed longitude difference `b − a`, in radians (antimeridian-safe).
fn shortest_d_lon(a: &impl LatLon, b: &impl LatLon) -> f64 {
    let mut d_lon = (b.lon() - a.lon()).to_radians();
    if d_lon.abs() > PI {
        d_lon -= d_lon.signum() * 2.0 * PI;
    }
    d_lon
}

#[cfg(test)]
mod tests;
