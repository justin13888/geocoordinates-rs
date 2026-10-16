//! #149: decoding a square its grid-zone designator truncates.

use super::*;

/// The decoded point of a truncated square sits midway between the
/// designator edge that cuts the square and the square edge facing it, as the
/// center of the largest inscribed circle does where the owned part is a
/// strip. `along_easting` names the axis the strip is cut across: easting for
/// a zone meridian, northing for a band edge. The designator edge is found by
/// bisecting, along that axis from the point, for where encoding stops giving
/// the reference back.
#[track_caller]
fn assert_centered_in_strip(s: &str, along_easting: bool) {
    let m = Mgrs::try_from(s).expect("valid");
    let Decoded {
        center: nominal,
        precision,
        ..
    } = decode(s).expect("valid");
    let half = precision.side().meters() / 2.0;
    let (ce, cn) = nominal.grid();
    let (pe, pn) = m.center.grid();
    let pick = |e: f64, n: f64| if along_easting { e } else { n };
    let at = |v: f64| {
        let grid = if along_easting { (v, pn) } else { (pe, v) };
        nominal.at(grid).expect("in the domain").coordinate()
    };
    let owns = |v: f64| Mgrs::try_from_coordinate(at(v), precision).unwrap() == m;
    let (p, c) = (pick(pe, pn), pick(ce, cn));
    // The cut lies between the point and the square's edge on the other side
    // of the nominal center.
    let mut outside = if p > c { c - half } else { c + half };
    let mut inside = p;
    assert!(owns(inside) && !owns(outside), "{s}");
    for _ in 0..60 {
        let mid = (inside + outside) / 2.0;
        if owns(mid) {
            inside = mid;
        } else {
            outside = mid;
        }
    }
    let to_cut = (p - inside).abs();
    let to_edge = if p > c { c + half - p } else { p - (c - half) };
    assert!(
        (to_cut - to_edge).abs() < 0.01 * to_edge,
        "{s}: {to_cut} vs {to_edge}"
    );
    // The strip's other axis is longer, so the circle does not touch it.
    let across = pick(pn - cn, pe - ce).abs();
    assert!(half - across >= 0.99 * to_edge, "{s}");
}

#[test]
fn truncated_squares_decode_midway_across_the_owned_strip() {
    // Zone 1's westernmost column, cut by the antimeridian.
    assert_centered_in_strip("01NAA", true);
    assert_centered_in_strip("01NAA60", true);
    // A square cut by band P's 8°N edge.
    assert_centered_in_strip("31PEJ", false);
}

#[test]
fn chebyshev_center_is_the_center_of_the_largest_inscribed_circle() {
    // The square [-3, 3]² cut to [1.5, 3]² by x ≥ 1.5 and y ≥ 1.5. (Not a
    // unit square, so every offset enters the solve as itself.)
    let planes = [
        ([1.0, 0.0], 3.0),
        ([-1.0, 0.0], 3.0),
        ([0.0, 1.0], 3.0),
        ([0.0, -1.0], 3.0),
        ([1.0, 0.0], -1.5),
        ([0.0, 1.0], -1.5),
    ];
    let [x, y] = clip::chebyshev_center(&planes, 3.0).unwrap();
    assert_close(x, 2.25, 1e-12);
    assert_close(y, 2.25, 1e-12);
    // A diagonal cut x + y ≥ 3 leaves a triangle with legs of 3 at (3, 3),
    // whose incircle has radius 3 (1 − 1/√2).
    let s = core::f64::consts::FRAC_1_SQRT_2;
    let r = 3.0 * (1.0 - s);
    let diagonal = [
        planes[0],
        planes[1],
        planes[2],
        planes[3],
        ([s, s], -3.0 * s),
    ];
    let [x, y] = clip::chebyshev_center(&diagonal, 3.0).unwrap();
    assert_close(x, 3.0 - r, 1e-12);
    assert_close(y, 3.0 - r, 1e-12);
}

#[test]
fn relinearizing_at_the_answer_keeps_it() {
    // A later round linearizes the designator's edges at the previous
    // answer instead of at the center. Where the edges are nearly straight
    // (zone 1's antimeridian edge at the equator), it lands where the first
    // round did, to a few meters of curvature over 33 km; an `owns` that
    // rejects the first answer forces it.
    let Decoded {
        center,
        precision,
        gzd,
        ..
    } = decode("01NAA").expect("valid");
    let half = precision.side().meters() / 2.0;
    let first = clip::deepest_owned(center, half, gzd, |_| true).unwrap();
    let calls = core::cell::Cell::new(0);
    let second = clip::deepest_owned(center, half, gzd, |_| {
        calls.set(calls.get() + 1);
        calls.get() > 1
    })
    .unwrap();
    assert_eq!(calls.get(), 2);
    let ((e1, n1), (e2, n2)) = (first.grid(), second.grid());
    assert!((e1 - e2).hypot(n1 - n2) < 10.0, "{first:?} vs {second:?}");
    // An `owns` that never accepts exhausts the rounds.
    assert!(clip::deepest_owned(center, half, gzd, |_| false).is_none());
}

#[test]
fn zone_spans_match_the_zone_rules() {
    use crate::grids::utm::{utm_zone_for, zone_span};
    // Every band's middle latitude, and a sweep of longitudes across the
    // exceptions and the antimeridian: the zone the rules pick holds the
    // longitude in its span, and no other zone's span does.
    for band in 0..20 {
        let lat = -76.0 + 8.0 * f64::from(band);
        let lat = if band == 19 { 78.0 } else { lat };
        for step in 0..=720 {
            let lon = -180.0 + 0.5 * f64::from(step);
            let zone = utm_zone_for(&c(lat, lon)).unwrap();
            for z in 1..=60 {
                let holds = zone_span(z, lat)
                    .is_some_and(|(w, e)| (w..e).contains(&lon) || (z == 60 && lon == 180.0));
                assert_eq!(holds, z == zone, "zone {z} at ({lat}, {lon})");
            }
        }
    }
}

/// #149: squares the grid-zone designator truncates — by the antimeridian
/// from either side (at 100 km and at 10 km), Norway's widened zone 32V,
/// Svalbard's widened zone 33X at its band edge, a latitude-band edge, the
/// UTM / UPS seams at 84°N and 80°S, and the 10 m square holding 32V's
/// south-west corner (56°N 3°E), of which the designator owns only a sliver.
/// Each nominal center lies outside the designator; the decoded point lies
/// inside it, so re-encoding gives the reference back, and the bound still
/// reaches every corner of the nominal square, which contains the truncated
/// part.
#[test]
fn truncated_squares_decode_inside_their_designator() {
    for s in [
        "01NAA",
        "60NZG",
        "01NAA60",
        "32VJM",
        "33XUV",
        "31PEJ",
        "31XDP",
        "BAZ",
        "32VJH26042233",
    ] {
        let Decoded {
            center: nominal,
            precision,
            ..
        } = decode(s).expect("valid");
        assert_ne!(
            encode_text(nominal.coordinate(), precision.digits()).unwrap(),
            s,
            "{s}: the nominal center is not truncated"
        );
        let decoded = Mgrs::try_from(s).expect("valid").to_coordinate();
        let again = Mgrs::try_from_coordinate(*decoded.value(), precision).unwrap();
        assert_eq!(again.as_str(), s);

        let half = precision.side().meters() / 2.0;
        let (ce, cn) = nominal.grid();
        let corner = |de: f64, dn: f64| {
            nominal
                .at((ce + de, cn + dn))
                .expect("in the projection's domain")
                .coordinate()
        };
        let worst = worst_corner_distance(
            decoded.value(),
            [
                corner(-half, -half),
                corner(half, -half),
                corner(-half, half),
                corner(half, half),
            ],
        );
        let bound = decoded.max_error().meters();
        assert!(worst <= bound, "{s}: worst {worst} m, bound {bound} m");
        // Never more than the full diagonal from a point of the square.
        assert!(
            bound <= 2.0 * half * core::f64::consts::SQRT_2 / nominal.k_min(),
            "{s}"
        );
    }
}
