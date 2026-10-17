//! Clipping a decoded MGRS square to the part its grid-zone designator owns.
//!
//! A reference names a nominal grid square, but the grid-zone designator (a
//! zone meridian, a latitude-band edge, the antimeridian, the Norway and
//! Svalbard exceptions, or the UTM / UPS seam) can truncate it. The nominal
//! center of a truncated square can then lie outside the designator, where
//! re-encoding it names a different reference. The part a reference owns is,
//! by definition, the set of points the encoder maps to it, so a candidate
//! point is accepted only once the encoder maps it back to the reference.

use core::f64::consts::FRAC_1_SQRT_2;

use super::{Center, encode_text};
use crate::angle::wrap_longitude;
use crate::coord::Coordinate;
use crate::grids::utm::{K0 as UTM_K0, UPS_K0, Ups, Utm, central_meridian};

/// Linearize-and-solve rounds before a truncated square falls back to its
/// nominal center. Each round re-linearizes the designator's edges at the
/// previous round's point, so the curvature of a parallel across a 100 km
/// square is corrected within two or three.
const ROUNDS: usize = 6;
/// The grid step, in meters, of the finite differences that linearize the
/// designator's edges.
const STEP_M: f64 = 0.5;

/// The extent of a grid-zone designator, as the encoder draws it.
#[derive(Debug, Clone, Copy)]
pub(super) enum Gzd {
    /// A UTM zone within a latitude band: latitudes `[south, north)`, and the
    /// longitudes `[west, east)` the zone covers there, `None` where the band
    /// drops the zone (Svalbard's 32X, 34X, 36X).
    Utm {
        zone: u8,
        south: f64,
        north: f64,
        span: Option<(f64, f64)>,
    },
    /// A UPS polar area, poleward of 84°N or of 80°S.
    Ups { north: bool },
}

impl Gzd {
    /// How far `coord` lies inside each edge of the designator, in degrees
    /// (negative outside); `None` for a dropped zone, which has no inside.
    fn slacks(self, coord: Coordinate) -> Option<Vec<f64>> {
        let (lat, lon) = (coord.lat(), coord.lon());
        Some(match self {
            Gzd::Utm {
                zone,
                south,
                north,
                span,
            } => {
                let (west, east) = span?;
                // Measured from the central meridian, so a longitude wrapped
                // across the antimeridian stays on the zone's side of it.
                let cm = central_meridian(zone);
                let rel = wrap_longitude(lon - cm);
                vec![
                    lat - south,
                    north - lat,
                    rel - (west - cm),
                    (east - cm) - rel,
                ]
            }
            Gzd::Ups { north: true } => vec![lat - 84.0],
            Gzd::Ups { north: false } => vec![-80.0 - lat],
        })
    }
}

impl Center {
    /// Grid easting and northing, in meters.
    pub(super) fn grid(self) -> (f64, f64) {
        match self {
            Center::Utm(utm) => (utm.easting_m(), utm.northing_m()),
            Center::Ups(ups) => (ups.easting_m(), ups.northing_m()),
        }
    }

    /// The point at grid `(easting, northing)` in this point's projection
    /// (same zone and hemisphere); `None` outside that projection's domain.
    pub(super) fn at(self, (easting, northing): (f64, f64)) -> Option<Center> {
        match self {
            Center::Utm(utm) => Utm::new(utm.zone(), utm.hemisphere(), easting, northing)
                .ok()
                .map(Center::Utm),
            Center::Ups(ups) => Ups::new(ups.hemisphere(), easting, northing)
                .ok()
                .map(Center::Ups),
        }
    }

    /// The geodetic WGS-84 coordinate.
    pub(super) fn coordinate(self) -> Coordinate {
        match self {
            Center::Utm(utm) => utm.to_coordinate(),
            Center::Ups(ups) => ups.to_coordinate(),
        }
    }

    /// The projection's minimum point scale factor: grid meters per ground
    /// meter never fall below it.
    pub(super) fn k_min(self) -> f64 {
        match self {
            Center::Utm(_) => UTM_K0,
            Center::Ups(_) => UPS_K0,
        }
    }
}

/// The easting the encoder names a square by: `utm`'s, the projection of
/// `coord`, kept west of the central meridian where `coord` lies west of it.
///
/// A longitude within about 1e-15° west of the central meridian projects onto
/// easting 500 000 m exactly, as its offset is below the easting's f64
/// resolution, which names a square east of the meridian. Zone 31 in band V
/// ends at its central meridian (3°E, the Norway edge with 32V), so that
/// square holds no point of 31V: [`settle`] finds no owned point and keeps
/// the nominal center, which re-encodes as 32V. Taking the largest easting
/// below 500 000 m instead names the square on the side the point lies on,
/// which does hold points of the designator. Elsewhere this moves a point
/// that close to a central meridian into the square west of it, where it
/// lies.
pub(super) fn encoder_easting(coord: Coordinate, utm: &Utm) -> f64 {
    if coord.lon() < central_meridian(utm.zone()) {
        utm.easting_m()
            .min(f64::from_bits(500_000f64.to_bits() - 1))
    } else {
        utm.easting_m()
    }
}

/// The point a reference decodes to, and its error bound in ground meters.
///
/// `center` is the center of the nominal square of side `side`, and `gzd` the
/// extent of the reference's grid-zone designator. Where the encoder maps the
/// center back to `text` it is kept, with the square's half diagonal as the
/// bound. Otherwise the designator truncates the square, and the point
/// returned is the deepest point of the part it owns: the center of the
/// largest circle inside both the square and the designator, with the
/// designator's edges linearized in grid space and re-linearized at each
/// answer until the encoder maps the point to `text`. The bound is then that
/// point's grid distance to the farthest corner of the nominal square, which
/// covers every point of the owned part. A reference whose square holds no
/// point of its designator (one the parser accepts but the encoder never
/// emits) keeps the nominal center and bound.
///
/// Every grid distance becomes a ground distance by dividing by the
/// projection's minimum scale factor `k_min`.
pub(super) fn settle(
    text: &str,
    digits: u32,
    center: Center,
    side: f64,
    gzd: Gzd,
) -> (Center, f64) {
    let k_min = center.k_min();
    let owns = |c: Center| encode_text(c.coordinate(), digits).is_ok_and(|t| t == text);
    if owns(center) {
        return (center, side * FRAC_1_SQRT_2 / k_min);
    }
    let half = side / 2.0;
    let Some(point) = deepest_owned(center, half, gzd, owns) else {
        return (center, side * FRAC_1_SQRT_2 / k_min);
    };
    let ((ce, cn), (pe, pn)) = (center.grid(), point.grid());
    // The farthest corner is the one diagonally across from the point's
    // quadrant of the square.
    let farthest = ((pe - ce).abs() + half).hypot((pn - cn).abs() + half);
    (point, farthest / k_min)
}

/// A half-plane `a · x + c ≥ 0` in grid meters about the square's center,
/// with `a` a unit vector, so `a · x + c` is the distance inside it.
type HalfPlane = ([f64; 2], f64);

/// The deepest point of the square of half side `half` about `center` that
/// is also inside `gzd`, once the encoder confirms it `owns` it.
pub(super) fn deepest_owned(
    center: Center,
    half: f64,
    gzd: Gzd,
    owns: impl Fn(Center) -> bool,
) -> Option<Center> {
    let at = |x: [f64; 2]| offset_point(center, x);
    let mut x = [0.0, 0.0];
    for _ in 0..ROUNDS {
        let s0 = gzd.slacks(at(x)?.coordinate())?;
        let se = gzd.slacks(at([x[0] + STEP_M, x[1]])?.coordinate())?;
        let sn = gzd.slacks(at([x[0], x[1] + STEP_M])?.coordinate())?;
        let mut planes: Vec<HalfPlane> = vec![
            ([1.0, 0.0], half),
            ([-1.0, 0.0], half),
            ([0.0, 1.0], half),
            ([0.0, -1.0], half),
        ];
        for k in 0..s0.len() {
            let g = [(se[k] - s0[k]) / STEP_M, (sn[k] - s0[k]) / STEP_M];
            let norm = g[0].hypot(g[1]);
            // s(y) ≈ s0 + g · (y − x) ≥ 0, scaled to meters.
            planes.push((
                [g[0] / norm, g[1] / norm],
                (s0[k] - g[0] * x[0] - g[1] * x[1]) / norm,
            ));
        }
        x = chebyshev_center(&planes, half)?;
        let point = at(x)?;
        if owns(point) {
            return Some(point);
        }
    }
    None
}

/// The point `[x, y]` grid meters east and north of `center`, in its
/// projection; `None` outside the projection's domain.
fn offset_point(center: Center, [x, y]: [f64; 2]) -> Option<Center> {
    let (ce, cn) = center.grid();
    center.at((ce + x, cn + y))
}

/// The center of the largest circle inside every half-plane: the `x`
/// maximizing `t` subject to `a · x + c ≥ t` for each plane. The optimum of
/// this three-variable linear program is a vertex where three constraints
/// are tight, so every triple is solved and the best feasible one kept.
/// `scale` sets the feasibility tolerance. `None` where no triple is
/// feasible.
pub(super) fn chebyshev_center(planes: &[HalfPlane], scale: f64) -> Option<[f64; 2]> {
    let tolerance = scale * 1e-9;
    // Every ordered triple, repeats included: a repeated plane makes the
    // system singular and is skipped, and a reordered triple has the same
    // solution, so this only re-solves what a combination walk would.
    let triples = planes.iter().flat_map(|&a| {
        planes
            .iter()
            .flat_map(move |&b| planes.iter().map(move |&c| [a, b, c]))
    });
    triples
        .filter_map(|triple| {
            let (point, depth) = tight(triple)?;
            let feasible = planes
                .iter()
                .all(|(p, c)| p[0] * point[0] + p[1] * point[1] + c >= depth - tolerance);
            feasible.then_some((point, depth))
        })
        .max_by(|(_, d0), (_, d1)| d0.total_cmp(d1))
        .map(|(point, _)| point)
}

/// The point `x` and depth `t` with `a · x + c = t` on all three planes;
/// `None` where the planes do not meet in one point. Subtracting the first
/// plane's equation from the other two eliminates `t`, leaving two linear
/// equations in `x`, solved by Cramer's rule.
fn tight([(a, ca), (b, cb), (c, cc)]: [HalfPlane; 3]) -> Option<([f64; 2], f64)> {
    // (b − a) · x = ca − cb and (c − a) · x = ca − cc.
    let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
    let (ru, rv) = (ca - cb, ca - cc);
    let det = u[0] * v[1] - u[1] * v[0];
    if det == 0.0 {
        return None;
    }
    let x = [(ru * v[1] - u[1] * rv) / det, (u[0] * rv - ru * v[0]) / det];
    Some((x, a[0] * x[0] + a[1] * x[1] + ca))
}
