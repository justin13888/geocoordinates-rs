//! Ellipsoidal (Karney) cross-track and along-track distance and geodesic
//! intersection, re-exported flat from [`geodesy`](super).
//!
//! Each runs on the ellipsoid of its inputs' own [`Crs`], beside the
//! mean-sphere forms in [`super::spherical`] whose names carry `_spherical`.
//! Both iterations re-centre on the current estimate, after Karney,
//! "Algorithms for geodesics" (2013), §8: the foot of a perpendicular steps
//! along the path until the geodesic to the point meets it at a right angle,
//! and a crossing is located with the gnomonic projection, which maps
//! geodesics through its centre to straight lines.

use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

use super::geodesic::{finish, geodesic_for_pair, validate_pair};
use super::spherical::{coincident_or_antipodal, great_circle_crossings};
#[cfg(doc)]
use crate::coord::Crs;
use crate::coord::{Coordinate, LatLon};
use crate::error::{Error, Result};
use crate::units::Length;

/// Signed perpendicular distance from `point` to the geodesic path `start` →
/// `end`, on the ellipsoid of their shared [`Crs`] (positive to the right,
/// negative to the left).
///
/// The path is the shortest geodesic from `start` through `end`, extended
/// both ways; the distance is the geodesic length from `point` to its nearest
/// foot on it, where the geodesic to `point` meets the path at a right angle.
/// The foot is found by iterating after Karney's gnomonic solution of this
/// interception problem: each step re-centres on the current foot and moves
/// to the point's gnomonic foot about it. The search stops one step after a
/// step moves the foot less than 1 µm, so the distance is exact to a few
/// nanometres (a sweep of 100,000 random WGS-84 triples left every foot square
/// to the path within 16 nm), rather than off by the mean sphere's up to 0.56%
/// ([`cross_track_distance_spherical`](super::cross_track_distance_spherical)).
/// Where `end` is so nearly antipodal to `start` that two shortest geodesics
/// join them (both on the equator, more than about 179.4° apart), the path is
/// the one GeographicLib's inverse solution returns.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when the three CRSs differ or are GCJ-02 / BD-09
/// (obfuscated renderings with no ellipsoid), and [`Error::InvalidValue`]:
/// `field: "path"` when `start` and `end` are the same place or antipodal
/// (within about 6 µm), which fixes no single path, and `field: "point"`
/// when the foot search does not settle within 100 steps.
///
/// ```
/// use geocoordinates::{Coordinate, Error, geodesy::{cross_track_distance, geodesic_distance}};
///
/// // A path east along the equator; meridians meet it at right angles, so a
/// // point 1° north is a meridian arc to its left.
/// let (start, end) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 10.0)?);
/// let point = Coordinate::wgs84(1.0, 5.0)?;
/// let arc = geodesic_distance(&Coordinate::wgs84(0.0, 5.0)?, &point)?.meters();
/// let xt = cross_track_distance(&point, &start, &end)?.meters();
/// assert!((xt + arc).abs() < 1e-6);
/// assert!(matches!(
///     cross_track_distance(&point, &start, &start),
///     Err(Error::InvalidValue { field: "path", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn cross_track_distance(
    point: &impl LatLon,
    start: &impl LatLon,
    end: &impl LatLon,
) -> Result<Length> {
    let foot = foot_on_path(point, start, end)?;
    Ok(Length::from_meters(foot.cross_track_m))
}

/// Distance along the geodesic path `start` → `end` from `start` to the foot
/// of the perpendicular from `point`, on the ellipsoid of their shared
/// [`Crs`] (the along-track component; negative when the foot lies behind
/// `start`).
///
/// The path and the foot are those of [`cross_track_distance`]. The foot is
/// the one nearest `point`, so the result lies within about half the
/// ellipsoid's circumference of `start`.
///
/// **Accuracy.** A few nanometres, except near a point where the
/// perpendiculars from neighbouring feet meet (a pole, for an equatorial
/// path). There the foot slides along the path with no change in distance, so
/// it is defined only to about `1e-9 m / M`, `M` being the geodesic scale of
/// `point` relative to its foot: about 0.6 m for a point 1 cm from the pole,
/// 6 mm at 1 m.
///
/// # Errors
///
/// As for [`cross_track_distance`], and [`Error::InvalidValue`]
/// (`field: "point"`) also when the perpendiculars from neighbouring feet all
/// meet at `point` (geodesic scale under 1e-12, as at a pole for an equatorial
/// path), so every foot is equally near and no along-track distance is
/// defined.
///
/// ```
/// use geocoordinates::{Coordinate, Error, geodesy::along_track_distance};
///
/// // Along the equator, the foot of a point 1° north of 5° E is (0, 5): the
/// // along-track distance is 5° of the equator, a · 5π/180.
/// let (start, end) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 10.0)?);
/// let at = along_track_distance(&Coordinate::wgs84(1.0, 5.0)?, &start, &end)?.meters();
/// assert!((at - 6_378_137.0 * 5f64.to_radians()).abs() < 1e-6);
/// // Every meridian meets the equator at a right angle: the pole has no foot.
/// assert!(matches!(
///     along_track_distance(&Coordinate::wgs84(90.0, 0.0)?, &start, &end),
///     Err(Error::InvalidValue { field: "point", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn along_track_distance(
    point: &impl LatLon,
    start: &impl LatLon,
    end: &impl LatLon,
) -> Result<Length> {
    let foot = foot_on_path(point, start, end)?;
    if perpendiculars_meet(foot.scale) {
        return Err(Error::InvalidValue {
            field: "point",
            detail: "every foot on the path is equally near; no along-track distance".into(),
        });
    }
    Ok(Length::from_meters(foot.along_m))
}

/// The outcome of [`intersection`]: where two geodesic paths cross ahead of
/// both starts on the ellipsoid, or why there is no single such point.
///
/// ```
/// use geocoordinates::{Coordinate, geodesy::{GeodesicIntersection, intersection}};
///
/// // East along the equator and north up the 10° E meridian (both geodesics).
/// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(-10.0, 10.0)?);
/// match intersection(&a, 90.0, &b, 0.0)? {
///     GeodesicIntersection::Crossing(p) => {
///         assert!(p.lat().abs() < 1e-9 && (p.lon() - 10.0).abs() < 1e-9)
///     }
///     other => panic!("expected a crossing, got {other:?}"),
/// }
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GeodesicIntersection {
    /// The paths cross at this point, which each start reaches along its own
    /// path as the shortest geodesic (or which is a start itself). It carries
    /// the inputs' CRS and no height.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, geodesy::{GeodesicIntersection, intersection}};
    ///
    /// // North from (0, 0) and west from (0, 30) meet at the start of the first.
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 30.0)?);
    /// let GeodesicIntersection::Crossing(p) = intersection(&a, 0.0, &b, 270.0)? else {
    ///     panic!("expected a crossing");
    /// };
    /// assert!(p.lat().abs() < 1e-9 && p.lon().abs() < 1e-9);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    Crossing(Coordinate),
    /// Both paths run along the same geodesic (in either direction), so every
    /// point of it is shared and no single crossing exists.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, geodesy::{GeodesicIntersection, intersection}};
    ///
    /// // Both paths run along the equator, in opposite directions.
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 30.0)?);
    /// assert_eq!(intersection(&a, 90.0, &b, 270.0)?, GeodesicIntersection::SameGeodesic);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    SameGeodesic,
    /// The geodesics cross, but at no point that both starts reach forward
    /// along their own paths as the shortest geodesic.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, geodesy::{GeodesicIntersection, intersection}};
    ///
    /// // Two meridians meet only at the poles; one path heads north, the other south.
    /// let (a, b) = (Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 30.0)?);
    /// assert_eq!(intersection(&a, 0.0, &b, 180.0)?, GeodesicIntersection::NoForwardCrossing);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    NoForwardCrossing,
}

/// Where two geodesic paths, each a start point and an initial bearing in
/// degrees, cross ahead of both starts, on the ellipsoid of their shared
/// [`Crs`].
///
/// The great-circle crossings on the mean sphere seed Karney's gnomonic
/// iteration: each step projects both paths gnomonically about the current
/// estimate, where geodesics through the centre are straight lines, and
/// re-centres on the lines' crossing. It converges linearly, by about the
/// flattening each step, and stops once both geodesics pass within 0.1 µm of
/// the estimate, so the crossing lies on both to about 0.1 µm (at most
/// 0.11 µm over 50,000 random WGS-84 crossings), rather than off by the mean
/// sphere's error ([`intersection_spherical`](super::intersection_spherical)).
/// Along the geodesics it is placed to about 0.1 µm over the sine of the
/// angle at which they meet, which only very shallow crossings make
/// noticeable. A crossing counts as ahead of a start when the shortest
/// geodesic from that start to it leaves along the start's bearing, so a path
/// more than about half way round the ellipsoid no longer reaches it.
///
/// At a pole the bearing is taken against the meridian of the start's own
/// longitude (see the [module docs](crate::geodesy)). The result is a
/// [`GeodesicIntersection::Crossing`] in `a.crs()` with no height, or names why
/// there is no single crossing.
///
/// # Errors
///
/// [`Error::CrsMismatch`] when the CRSs differ or are GCJ-02 / BD-09, and
/// [`Error::InvalidValue`]: `field: "bearing"` for a non-finite bearing, and
/// `field: "paths"` when the iteration cannot locate a crossing that the
/// mean sphere places ahead of both starts (reported rather than guessed;
/// none was met in a sweep of 200,000 random WGS-84 path pairs).
///
/// ```
/// use geocoordinates::{
///     Coordinate, Error, Length,
///     geodesy::{GeodesicIntersection, cross_track_distance, destination, intersection},
/// };
///
/// // North-east from New York and north-west from Lisbon cross over the Atlantic.
/// let (nyc, lisbon) = (Coordinate::wgs84(40.7, -74.0)?, Coordinate::wgs84(38.7, -9.1)?);
/// let GeodesicIntersection::Crossing(p) = intersection(&nyc, 45.0, &lisbon, 315.0)? else {
///     panic!("expected a crossing");
/// };
/// // The crossing lies on both geodesics, to well under a micrometre.
/// let ahead_a = destination(&nyc, 45.0, Length::from_meters(1e6))?;
/// let ahead_b = destination(&lisbon, 315.0, Length::from_meters(1e6))?;
/// assert!(cross_track_distance(&p, &nyc, &ahead_a)?.meters().abs() < 1e-6);
/// assert!(cross_track_distance(&p, &lisbon, &ahead_b)?.meters().abs() < 1e-6);
/// assert!(matches!(
///     intersection(&nyc, f64::NAN, &lisbon, 315.0),
///     Err(Error::InvalidValue { field: "bearing", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn intersection(
    a: &Coordinate,
    bearing_a_deg: f64,
    b: &Coordinate,
    bearing_b_deg: f64,
) -> Result<GeodesicIntersection> {
    let g = geodesic_for_pair(a, b)?;
    if !bearing_a_deg.is_finite() || !bearing_b_deg.is_finite() {
        return Err(Error::InvalidValue {
            field: "bearing",
            detail: "must be finite".into(),
        });
    }
    let path_a = Path::new(&g, a, bearing_a_deg);
    let path_b = Path::new(&g, b, bearing_b_deg);
    if path_a.carries(b, bearing_b_deg) {
        return Ok(GeodesicIntersection::SameGeodesic);
    }
    // The mean sphere's crossings seed the iteration first, then each start:
    // where the paths meet at a shallow angle the sphere's crossing can lie
    // far from the ellipsoid's, and a start near the other path is closer.
    // A seed the sphere places ahead of both starts that cannot be refined
    // leaves the answer unknown; so do the starts when the sphere sees one
    // great circle, and so offers no seed of its own.
    let sphere = great_circle_crossings(a, bearing_a_deg, b, bearing_b_deg);
    let seeds = sphere
        .into_iter()
        .flatten()
        .map(|x| (x.lat, x.lon, x.forward))
        .chain([a, b].map(|p| (p.lat(), p.lon(), sphere.is_none())));
    let refined = seeds.map(|(lat, lon, forward_on_sphere)| {
        let crossing = gnomonic_crossing(&path_a, &path_b, lat, lon)
            .map(|(lat, lon)| (lat, lon, path_a.ahead(lat, lon) && path_b.ahead(lat, lon)));
        (crossing, forward_on_sphere)
    });
    match first_crossing_ahead(refined)? {
        Some((lat, lon)) => finish(lat, lon, a.crs()).map(GeodesicIntersection::Crossing),
        None => Ok(GeodesicIntersection::NoForwardCrossing),
    }
}

/// The first refined crossing ahead of both starts, drawing seeds lazily.
/// Each item is a seed's refinement, `(lat, lon, ahead of both)` or `None`
/// when it did not settle, and whether the sphere placed the seed ahead of
/// both starts. `Ok(None)` when no seed yields one, and [`Error::InvalidValue`]
/// (`field: "paths"`) when a seed the sphere placed ahead could not be
/// refined, which leaves the answer unknown.
fn first_crossing_ahead(
    refined: impl IntoIterator<Item = (Option<(f64, f64, bool)>, bool)>,
) -> Result<Option<(f64, f64)>> {
    let mut unresolved = false;
    for (crossing, forward_on_sphere) in refined {
        match crossing {
            Some((lat, lon, true)) => return Ok(Some((lat, lon))),
            Some(_) => {}
            None => unresolved |= forward_on_sphere,
        }
    }
    if unresolved {
        Err(Error::InvalidValue {
            field: "paths",
            detail: "the crossing ahead on the mean sphere could not be located".into(),
        })
    } else {
        Ok(None)
    }
}

/// A geodesic path: a start and the azimuth it leaves on, on one ellipsoid.
struct Path<'g> {
    g: &'g Geodesic,
    lat: f64,
    lon: f64,
    azi: f64,
}

/// The foot of the perpendicular from a point onto a [`Path`].
struct Foot {
    /// Signed distance along the path from its start to the foot.
    along_m: f64,
    /// Signed geodesic distance from the foot to the point, positive to the
    /// right of the path.
    cross_track_m: f64,
    /// Geodesic scale of the point relative to the foot: zero where the
    /// perpendiculars from neighbouring feet meet at the point.
    scale: f64,
}

/// Iteration caps. Both searches converge linearly, by a factor of about the
/// flattening each step, so a handful of steps reach 1 µm from any seed they
/// converge from at all.
const FOOT_ITERATIONS: usize = 100;
const CROSSING_ITERATIONS: usize = 50;

/// Whether the perpendiculars from neighbouring feet meet at the point: its
/// geodesic scale relative to the foot is under 1e-12, about 6 µm of
/// separation per meter of path, matching the spherical along-track's pole
/// tolerance.
fn perpendiculars_meet(scale: f64) -> bool {
    scale < 1e-12
}

/// Whether a length is under 1 µm, the searches' stopping distance.
fn negligible(length_m: f64) -> bool {
    length_m.abs() < 1e-6
}

/// Whether the geodesic to the point already leaves the foot square to the
/// path to round-off: `|cos γ|` within 4 ε.
fn square_to_round_off(cos_gamma: f64) -> bool {
    cos_gamma.abs() <= 4.0 * f64::EPSILON
}

/// Whether a point at `cross_track_m` from a path, where the path's azimuth
/// is `sin_angle` off a bearing's line, lies on that line: within 10 µm, and
/// within 1e-10 rad.
fn on_line(cross_track_m: f64, sin_angle: f64) -> bool {
    cross_track_m.abs() < 1e-5 && sin_angle.abs() < 1e-10
}

/// Whether a point `along_m` ahead of a start and `lateral_m` off its
/// direction (both measured on the shortest geodesic to it) lies ahead along
/// the path: no more than 10 µm behind, and within 1 µm of the path.
fn is_ahead(along_m: f64, lateral_m: f64) -> bool {
    along_m > -1e-5 && lateral_m < 1e-6
}

impl<'g> Path<'g> {
    fn new(g: &'g Geodesic, start: &impl LatLon, azi: f64) -> Self {
        Self {
            g,
            lat: start.lat(),
            lon: start.lon(),
            azi,
        }
    }

    /// The position `(lat, lon, azimuth)` reached `s` meters along the path
    /// (behind the start when negative).
    fn at(&self, s: f64) -> (f64, f64, f64) {
        self.g.direct(self.lat, self.lon, self.azi, s)
    }

    /// The nearest foot of the perpendicular from `(lat, lon)`.
    ///
    /// Each step measures the geodesic from the current foot to the point
    /// (length `s`, reduced length `m`, scale `M`, and angle `γ` off the path)
    /// and moves the foot by `a · atan2(m cos γ / a, M)`: the gnomonic foot of
    /// the point about the current one, read back on a sphere of radius `a`,
    /// the semi-major axis. That is exact on a sphere; on the ellipsoid its fixed point (`cos γ =
    /// 0`) is still the exact foot, reached linearly with a ratio near the
    /// flattening.
    ///
    /// It stops one step after a step under 1 µm, or once `cos γ` is at round-off,
    /// where the step `≈ m cos γ / M` is round-off magnified by `m / M`: there
    /// the foot is only as well defined as about `1e-9 m / M` along the path.
    fn foot(&self, lat: f64, lon: f64) -> Result<Foot> {
        self.foot_from(lat, lon, 0.0)
    }

    /// [`Path::foot`], searching from `along` meters along the path rather
    /// than from the start, so a caller tracking a moving point stays with the
    /// foot it found before.
    fn foot_from(&self, lat: f64, lon: f64, mut along: f64) -> Result<Foot> {
        let radius = self.g.equatorial_radius();
        // Set once a step is under 1 µm: that step is still taken, and the
        // foot it reaches (off by about the flattening times it) returned. A
        // point on the path is as far from a foot as the foot is misplaced,
        // so stopping a step short would cost the whole step.
        let mut settled = false;
        for _ in 0..FOOT_ITERATIONS {
            let (foot_lat, foot_lon, foot_azi) = self.at(along);
            let (s, azi, _, m, scale, _, _): (f64, f64, f64, f64, f64, f64, f64) =
                self.g.inverse(foot_lat, foot_lon, lat, lon);
            let gamma = (azi - foot_azi).to_radians();
            if settled || square_to_round_off(gamma.cos()) {
                return Ok(Foot {
                    along_m: along,
                    // `+ 0.0` turns a -0.0 (a point on the path) into +0.0.
                    cross_track_m: s.copysign(gamma.sin()) + 0.0,
                    scale,
                });
            }
            let step = radius * (m * gamma.cos() / radius).atan2(scale);
            along += step;
            settled = negligible(step);
        }
        Err(Error::InvalidValue {
            field: "point",
            detail: "no single nearest foot on the path".into(),
        })
    }

    /// Whether the path, extended both ways, runs through `p` along
    /// `bearing_deg` or its reverse: `p` within 10 µm of the path, and the
    /// path's azimuth at the foot within 1e-10 rad of the bearing's line.
    fn carries(&self, p: &impl LatLon, bearing_deg: f64) -> bool {
        let Ok(foot) = self.foot(p.lat(), p.lon()) else {
            return false;
        };
        let (_, _, azi) = self.at(foot.along_m);
        on_line(foot.cross_track_m, (azi - bearing_deg).to_radians().sin())
    }

    /// The shortest geodesic from the start to `(lat, lon)`, resolved against
    /// the path's direction: `(along, lateral)`, its length signed by whether
    /// it leaves forward or backward, and its length times the absolute sine
    /// of its azimuth off the path's.
    fn resolve(&self, lat: f64, lon: f64) -> (f64, f64) {
        let (s, azi, _, _): (f64, f64, f64, f64) = self.g.inverse(self.lat, self.lon, lat, lon);
        let (sin, cos) = (azi - self.azi).to_radians().sin_cos();
        (s.copysign(cos), s * sin.abs())
    }

    /// Whether `(lat, lon)` lies ahead of the start: the shortest geodesic to
    /// it leaves along the path (see [`is_ahead`]), so a point beyond about
    /// half way round, which the shortest geodesic reaches another way, does
    /// not.
    fn ahead(&self, lat: f64, lon: f64) -> bool {
        let (along, lateral) = self.resolve(lat, lon);
        is_ahead(along, lateral)
    }

    /// Two points on the path, 100 km either side of `along` meters along it.
    fn chord(&self, along: f64) -> [(f64, f64); 2] {
        [along - 100_000.0, along + 100_000.0].map(|s| {
            let (lat, lon, _) = self.at(s);
            (lat, lon)
        })
    }
}

/// Karney's gnomonic iteration for the crossing of two geodesic paths, seeded
/// at `(lat, lon)`: each step projects a chord of each path gnomonically about
/// the current estimate, intersects the two straight lines, and re-centres on
/// that point, read back by [`gnomonic_back`]. At the fixed point both
/// geodesics run through the centre, where the gnomonic projection maps them to exact
/// straight lines, so the fixed point is the exact crossing. `None` when a
/// chord leaves the projection's hemisphere (or its foot is not found), the
/// lines are parallel, or the iteration does not settle.
fn gnomonic_crossing(a: &Path<'_>, b: &Path<'_>, lat: f64, lon: f64) -> Option<(f64, f64)> {
    let g = a.g;
    let unit = g.equatorial_radius();
    let (mut lat0, mut lon0) = (lat, lon);
    // Where each path's foot of the centre lies, carried from step to step so
    // that a centre near half way round, whose nearest foot could be found
    // either way round, keeps to the one it started with.
    let mut feet = [0.0, 0.0];
    for _ in 0..CROSSING_ITERATIONS {
        // Each path's chord about its foot of the centre, as the line through
        // its two projected points, in homogeneous coordinates scaled by the
        // semi-major axis.
        let line = |path: &Path<'_>, along: &mut f64| -> Option<[f64; 3]> {
            *along = path.foot_from(lat0, lon0, *along).ok()?.along_m;
            let [p, q] = path.chord(*along);
            let (p, q) = (
                gnomonic_forward(g, lat0, lon0, p)?,
                gnomonic_forward(g, lat0, lon0, q)?,
            );
            Some(cross3(
                [p.0 / unit, p.1 / unit, 1.0],
                [q.0 / unit, q.1 / unit, 1.0],
            ))
        };
        let [along_a, along_b] = &mut feet;
        let (line_a, line_b) = (line(a, along_a)?, line(b, along_b)?);
        // Settled once both geodesics pass within 0.1 µm of the centre: a
        // shallow crossing slides along them by round-off over about
        // 1 nm / θ, so the distance the centre moves is no stopping test.
        let tolerance = 1e-7 / unit;
        if passes_within(line_a, tolerance) && passes_within(line_b, tolerance) {
            return Some((lat0, lon0));
        }
        let [x, y, w] = cross3(line_a, line_b);
        let (x, y) = (x / w * unit, y / w * unit);
        // Parallel lines meet at infinity.
        if !x.hypot(y).is_finite() {
            return None;
        }
        (lat0, lon0) = gnomonic_back(g, lat0, lon0, x, y);
    }
    None
}

/// The gnomonic projection `(x, y)` of `p` about `(lat0, lon0)`, in meters,
/// or `None` beyond the projection's hemisphere (geodesic scale not
/// positive).
fn gnomonic_forward(g: &Geodesic, lat0: f64, lon0: f64, p: (f64, f64)) -> Option<(f64, f64)> {
    let (_, azi, _, m, scale, _, _): (f64, f64, f64, f64, f64, f64, f64) =
        g.inverse(lat0, lon0, p.0, p.1);
    if scale <= 0.0 {
        return None;
    }
    let rho = m / scale;
    let (sin, cos) = azi.to_radians().sin_cos();
    Some((rho * sin, rho * cos))
}

/// The point at gnomonic `(x, y)` about `(lat0, lon0)`, read back as on a
/// sphere of radius `a`, the semi-major axis: `a · atan(ρ / a)` along the
/// azimuth `atan2(x, y)`, with `ρ = hypot(x, y)`. Exact on a sphere; on the
/// ellipsoid it is off by about the flattening times the distance, which the
/// crossing iteration absorbs, because its fixed point (`ρ = 0`) does not
/// depend on the read-back.
fn gnomonic_back(g: &Geodesic, lat0: f64, lon0: f64, x: f64, y: f64) -> (f64, f64) {
    let a = g.equatorial_radius();
    let (azi, rho) = (x.atan2(y).to_degrees(), x.hypot(y));
    g.direct(lat0, lon0, azi, a * (rho / a).atan())
}

/// Whether the homogeneous line `l` passes strictly within `distance` of the
/// origin, in the units of its coordinates.
fn passes_within(l: [f64; 3], distance: f64) -> bool {
    l[2].abs() < distance * l[0].hypot(l[1])
}

fn cross3(u: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// The nearest foot from `point` onto the geodesic path `start` → `end`,
/// after validating the three positions.
fn foot_on_path(point: &impl LatLon, start: &impl LatLon, end: &impl LatLon) -> Result<Foot> {
    validate_pair(point, start)?;
    let g = geodesic_for_pair(start, end)?;
    if coincident_or_antipodal(start, end) {
        return Err(Error::InvalidValue {
            field: "path",
            detail: "start and end must be distinct and not antipodal".into(),
        });
    }
    let (azi, _, _): (f64, f64, f64) = g.inverse(start.lat(), start.lon(), end.lat(), end.lon());
    Path::new(&g, start, azi).foot(point.lat(), point.lon())
}

#[cfg(test)]
mod tests;
