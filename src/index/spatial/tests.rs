use proptest::prelude::*;

use super::*;
use crate::LengthUnit;
use crate::geodesy::geodesic_distance;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

/// Every entry as `(index, meters)`, ordered by distance then insertion order.
fn brute_force(points: &[Coordinate], center: Coordinate) -> Vec<(usize, f64)> {
    let mut all: Vec<(usize, f64)> = points
        .iter()
        .enumerate()
        .map(|(i, p)| (i, geodesic_distance(center, *p).unwrap().meters()))
        .collect();
    all.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    all
}

fn pairs(hits: &[Neighbor<'_, usize>]) -> Vec<(usize, f64)> {
    hits.iter()
        .map(|hit| (hit.index(), hit.distance().meters()))
        .collect()
}

fn build(points: &[Coordinate]) -> SpatialIndex<usize> {
    SpatialIndex::new(points.iter().copied().zip(0..)).unwrap()
}

#[test]
fn an_empty_index_answers_nothing_but_still_checks_its_inputs() {
    let index = SpatialIndex::<()>::new([]).unwrap();
    assert!(index.is_empty());
    assert_eq!(index.len(), 0);
    assert_eq!(index.crs(), None);
    let center = wgs84(0.0, 0.0);
    assert!(
        index
            .within(center, Length::from_meters(1.0))
            .unwrap()
            .is_empty()
    );
    assert!(index.nearest(center, 3).unwrap().is_empty());
    let gcj = Coordinate::gcj02(0.0, 0.0).unwrap();
    assert!(matches!(
        index.nearest(gcj, 1),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
    assert!(matches!(
        index.within(center, Length::from_meters(-1.0)),
        Err(Error::InvalidValue {
            field: "radius",
            ..
        })
    ));
}

#[test]
fn new_rejects_a_mixed_or_obfuscated_set() {
    assert!(matches!(
        SpatialIndex::new([(Coordinate::bd09(1.0, 2.0).unwrap(), ())]),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Bd09,
        })
    ));
    let tokyo = Coordinate::new(1.0, 2.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        SpatialIndex::new([(wgs84(1.0, 2.0), ()), (tokyo, ())]),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Tokyo,
        })
    ));
}

#[test]
fn queries_reject_a_center_on_another_crs() {
    let index = SpatialIndex::new([(wgs84(1.0, 2.0), ())]).unwrap();
    assert_eq!(index.crs(), Some(Crs::Wgs84));
    let tokyo = Coordinate::new(1.0, 2.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        index.nearest(tokyo, 1),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Tokyo,
        })
    ));
    assert!(matches!(
        index.within(tokyo, Length::from_meters(1.0)),
        Err(Error::CrsMismatch { .. })
    ));
    assert!(matches!(
        index.nearest(Coordinate::gcj02(1.0, 2.0).unwrap(), 1),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02,
        })
    ));
}

#[test]
fn within_rejects_a_bad_radius() {
    let index = SpatialIndex::new([(wgs84(1.0, 2.0), ())]).unwrap();
    for bad in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            index.within(wgs84(0.0, 0.0), Length::from_meters(bad)),
            Err(Error::InvalidValue {
                field: "radius",
                ..
            })
        ));
    }
}

#[test]
fn within_includes_the_radius_itself() {
    let (a, b) = (wgs84(0.0, 0.0), wgs84(1.0, 1.0));
    let index = build(&[b]);
    let exact = geodesic_distance(a, b).unwrap();
    assert_eq!(index.within(a, exact).unwrap().len(), 1);
    let less = Length::from_meters(exact.meters() - 1e-6);
    assert!(index.within(a, less).unwrap().is_empty());
    // A zero radius finds a coincident entry and nothing else.
    let zero = Length::from_meters(0.0);
    assert_eq!(index.within(b, zero).unwrap().len(), 1);
    assert!(index.within(a, zero).unwrap().is_empty());
}

#[test]
fn nearest_of_zero_is_empty_and_more_than_len_is_every_entry() {
    let index = build(&[wgs84(0.0, 0.0), wgs84(0.0, 1.0)]);
    assert!(index.nearest(wgs84(0.0, 0.0), 0).unwrap().is_empty());
    assert_eq!(index.nearest(wgs84(0.0, 0.0), 9).unwrap().len(), 2);
    assert_eq!(index.len(), 2);
    assert!(!index.is_empty());
}

#[test]
fn ties_break_on_insertion_order() {
    let same = wgs84(10.0, 10.0);
    let points = vec![same; 40];
    let index = build(&points);
    let near = index.nearest(same, 5).unwrap();
    assert_eq!(
        near.iter().map(Neighbor::index).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    let within = index.within(same, Length::from_meters(1.0)).unwrap();
    assert_eq!(
        within.iter().map(Neighbor::index).collect::<Vec<_>>(),
        (0..40).collect::<Vec<_>>()
    );
    // Mirror images are at the same distance from a point on the equator.
    let mirrored = [wgs84(1.0, 0.0), wgs84(-1.0, 0.0), wgs84(-1.0, 0.0)];
    let index = build(&mirrored);
    let hits = index.nearest(wgs84(0.0, 0.0), 2).unwrap();
    assert_eq!((hits[0].index(), hits[1].index()), (0, 1));
    assert_eq!(hits[0].distance(), hits[1].distance());
}

#[test]
fn queries_see_across_the_antimeridian_and_the_poles() {
    let points = [
        wgs84(0.0, 179.99),
        wgs84(0.0, -179.99),
        wgs84(0.0, 180.0),
        wgs84(0.0, -180.0),
        wgs84(90.0, 10.0),
        wgs84(90.0, -170.0),
        wgs84(89.99, 100.0),
    ];
    let index = build(&points);
    let km = Length::from_unit(5.0, LengthUnit::Kilometer);
    let at_seam: Vec<usize> = index
        .within(wgs84(0.0, 180.0), km)
        .unwrap()
        .iter()
        .map(Neighbor::index)
        .collect();
    assert_eq!(at_seam, vec![2, 3, 0, 1]);
    let at_pole = index.nearest(wgs84(90.0, 77.0), 3).unwrap();
    assert_eq!(
        (at_pole[0].index(), at_pole[1].index(), at_pole[2].index()),
        (4, 5, 6)
    );
    assert_eq!(at_pole[0].distance().meters(), 0.0);
    assert!(at_pole[2].distance().meters() < 1_200.0);
}

#[test]
fn the_ellipsoid_of_the_crs_measures_the_distances() {
    let (a, b) = (
        Coordinate::new(35.0, 139.0, Crs::Tokyo).unwrap(),
        Coordinate::new(36.0, 140.0, Crs::Tokyo).unwrap(),
    );
    let index = SpatialIndex::new([(b, "b")]).unwrap();
    assert_eq!(index.crs(), Some(Crs::Tokyo));
    let hit = index.nearest(a, 1).unwrap()[0];
    assert_eq!(hit.distance(), geodesic_distance(a, b).unwrap());
}

#[test]
fn an_antipodal_radius_needs_no_pruning_and_stays_exact() {
    let points = [wgs84(0.0, 0.0), wgs84(0.0, 180.0), wgs84(0.5, 179.5)];
    let index = build(&points);
    let center = wgs84(0.0, 0.0);
    let everywhere = Length::from_unit(25_000.0, LengthUnit::Kilometer);
    assert_eq!(index.within(center, everywhere).unwrap().len(), 3);
    let far = index.nearest(center, 3).unwrap();
    assert_eq!(pairs(&far), brute_force(&points, center));
}

#[test]
fn the_chord_limit_never_excludes_an_entry_at_that_distance() {
    let model = Model::for_crs(Crs::Wgs84).unwrap();
    for (a, b) in [
        (wgs84(0.0, 0.0), wgs84(0.0, 90.0)),
        (wgs84(0.0, 0.0), wgs84(90.0, 0.0)),
        (wgs84(45.0, 0.0), wgs84(-45.0, 179.0)),
        (wgs84(10.0, 10.0), wgs84(10.0, 10.000_001)),
        (wgs84(60.0, 1.0), wgs84(-60.0, -179.0)),
    ] {
        let (va, vb) = (model.unit_vector(a), model.unit_vector(b));
        let chord_sq: f64 = (0..3).map(|i| (va[i] - vb[i]).powi(2)).sum();
        let meters = model.distance_m(a, b);
        assert!(chord_sq <= model.chord_sq_limit(meters), "{a:?} {b:?}");
    }
    // The limit is finite until a half turn of the reduced sphere.
    assert!(model.chord_sq_limit(1000.0) < 1e-6);
    assert_eq!(model.chord_sq_limit(3e7), f64::INFINITY);
}

#[test]
fn the_chord_limit_exceeds_the_exact_chord_by_only_its_slack() {
    let model = Model::for_crs(Crs::Wgs84).unwrap();
    let meters = 1_000_000.0;
    let exact = (2.0 * (meters / model.semi_minor_m / 2.0).sin()).powi(2);
    let limit = model.chord_sq_limit(meters);
    assert!(
        limit > exact * (1.0 + CHORD_SLACK_REL / 2.0),
        "{limit} {exact}"
    );
    assert!(
        limit < exact * (1.0 + 2.0 * CHORD_SLACK_REL),
        "{limit} {exact}"
    );
    assert_eq!(model.chord_sq_limit(0.0), CHORD_SLACK_ABS);
}

#[test]
fn heap_entries_compare_equal_only_when_they_match() {
    let refined = |meters, index| Refined { meters, index };
    assert!(refined(1.0, 2) == refined(1.0, 2));
    assert!(refined(1.0, 2) != refined(1.0, 3));
    assert!(refined(1.0, 2) != refined(2.0, 2));
    assert!(KdTree::pending_eq_holds());
}

#[test]
fn unit_vectors_are_unit_length_and_use_the_reduced_latitude() {
    let model = Model::for_crs(Crs::Wgs84).unwrap();
    let [x, y, z] = model.unit_vector(wgs84(45.0, 90.0));
    assert!((x.hypot(y).hypot(z) - 1.0).abs() < 1e-15);
    assert!(x.abs() < 1e-15);
    let reduced = ((1.0 - model.flattening) * 45.0_f64.to_radians().tan()).atan();
    assert!((z - reduced.sin()).abs() < 1e-15);
    assert!((y - reduced.cos()).abs() < 1e-15);
    let [x, y, _] = model.unit_vector(wgs84(45.0, 0.0));
    assert!((x - reduced.cos()).abs() < 1e-15);
    assert!(y.abs() < 1e-15);
}

#[test]
fn a_neighbor_is_copy_whatever_its_value() {
    struct NotCopy;
    let index = SpatialIndex::new([(wgs84(1.0, 2.0), NotCopy)]).unwrap();
    let hit = index.nearest(wgs84(1.0, 2.0), 1).unwrap()[0];
    let copy = hit;
    assert_eq!(hit.coordinate(), copy.coordinate());
    #[expect(clippy::clone_on_copy, reason = "the manual Clone impl is under test")]
    let cloned = hit.clone();
    assert_eq!(cloned.index(), 0);
    let _: &NotCopy = cloned.value();
}

#[test]
fn a_large_set_splits_into_a_tree_and_stays_exact() {
    // More points than any one leaf holds, on a regular grid full of ties.
    let points: Vec<Coordinate> = (-20..20)
        .flat_map(|lat| (-20..20).map(move |lon| wgs84(f64::from(lat) * 4.0, f64::from(lon) * 9.0)))
        .collect();
    let index = build(&points);
    for center in [wgs84(0.0, 0.0), wgs84(-80.0, 179.0), wgs84(33.0, -91.0)] {
        let all = brute_force(&points, center);
        assert_eq!(pairs(&index.nearest(center, 25).unwrap()), all[..25]);
        assert_eq!(pairs(&index.nearest(center, all.len()).unwrap()), all);
        let radius = 2_000_000.0;
        let expected: Vec<(usize, f64)> =
            all.iter().copied().filter(|hit| hit.1 <= radius).collect();
        let found = index.within(center, Length::from_meters(radius)).unwrap();
        assert_eq!(pairs(&found), expected);
        assert!(
            found
                .iter()
                .all(|hit| points[*hit.value()] == hit.coordinate())
        );
    }
}

#[test]
fn by_chord_yields_every_point_by_rising_chord() {
    let model = Model::for_crs(Crs::Wgs84).unwrap();
    let points: Vec<Coordinate> = (0..100)
        .map(|i| wgs84(f64::from(i) - 50.0, f64::from(i) * 3.0 - 150.0))
        .collect();
    let tree = KdTree::build(points.iter().map(|p| model.unit_vector(*p)));
    let query = model.unit_vector(wgs84(10.0, 10.0));
    // One past the expected count: a walk that overruns fails here, not by hanging.
    let walk: Vec<(f64, usize)> = tree.by_chord(query).take(points.len() + 1).collect();
    assert_eq!(walk.len(), points.len());
    assert!(walk.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    let mut ids: Vec<usize> = walk.iter().map(|&(_, id)| id).collect();
    ids.sort_unstable();
    assert_eq!(ids, (0..100).collect::<Vec<_>>());
    assert_eq!(KdTree::build([]).by_chord([1.0, 0.0, 0.0]).count(), 0);
    let mut none = 0;
    KdTree::build([]).for_each_within([1.0, 0.0, 0.0], 4.0, |_| none += 1);
    assert_eq!(none, 0);
}

#[test]
fn by_chord_reports_the_squared_chord_to_each_point() {
    let tree = KdTree::build([[1.0, 2.0, 3.0], [-1.0, 0.0, 2.0]]);
    let walk: Vec<(f64, usize)> = tree.by_chord([1.0, 0.0, 0.0]).take(3).collect();
    assert_eq!(walk, [(8.0, 1), (13.0, 0)]);
}

#[test]
fn for_each_within_keeps_only_points_inside_the_limit() {
    let tree = KdTree::build([[1.0, 2.0, 3.0], [-1.0, 0.0, 2.0]]);
    let mut found = Vec::new();
    tree.for_each_within([1.0, 0.0, 0.0], 8.0, |id| found.push(id));
    assert_eq!(found, [1]);
}

#[test]
fn the_box_distance_sums_the_gap_along_each_axis() {
    let tree = KdTree::build([[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]);
    // Past the high side on x, inside on y, past the low side on z.
    assert_eq!(tree.root_box_chord_sq([3.0, 0.5, -1.0]), 5.0);
    assert_eq!(tree.root_box_chord_sq([0.5, 0.5, 0.5]), 0.0);
}

/// Random points, mixed with the places a spatial routine breaks: the
/// antimeridian, its neighbourhood, the poles, and repeated positions.
fn point() -> impl Strategy<Value = Coordinate> {
    let lat = prop_oneof![
        4 => -90.0..=90.0_f64,
        1 => 89.0..=90.0_f64,
        1 => -90.0..=-89.0_f64,
        1 => Just(90.0),
        1 => Just(-90.0),
        1 => Just(0.0),
    ];
    let lon = prop_oneof![
        3 => -180.0..=180.0_f64,
        2 => 179.0..=180.0_f64,
        2 => -180.0..=-179.0_f64,
        1 => Just(180.0),
        1 => Just(-180.0),
    ];
    (lat, lon).prop_map(|(lat, lon)| wgs84(lat, lon))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn nearest_matches_brute_force(
        points in prop::collection::vec(point(), 1..160),
        center in point(),
        k in 0..40_usize,
    ) {
        let index = build(&points);
        let all = brute_force(&points, center);
        let expected = &all[..k.min(all.len())];
        prop_assert_eq!(pairs(&index.nearest(center, k).unwrap()), expected);
    }

    #[test]
    fn within_matches_brute_force(
        points in prop::collection::vec(point(), 1..160),
        center in point(),
        radius in prop_oneof![
            0.0..=1_000.0_f64,
            0.0..=500_000.0_f64,
            0.0..=21_000_000.0_f64,
        ],
    ) {
        let index = build(&points);
        let expected: Vec<(usize, f64)> = brute_force(&points, center)
            .into_iter()
            .filter(|hit| hit.1 <= radius)
            .collect();
        let found = index.within(center, Length::from_meters(radius)).unwrap();
        prop_assert_eq!(pairs(&found), expected);
    }

    /// Repeating a set makes every distance a tie, the hardest case for the
    /// early exit: an entry at exactly the k-th distance must still be examined.
    #[test]
    fn nearest_keeps_the_earliest_of_tied_entries(
        points in prop::collection::vec(point(), 1..40),
        center in point(),
        k in 1..60_usize,
    ) {
        let doubled: Vec<Coordinate> = points.iter().chain(&points).copied().collect();
        let index = build(&doubled);
        let all = brute_force(&doubled, center);
        let expected = &all[..k.min(all.len())];
        prop_assert_eq!(pairs(&index.nearest(center, k).unwrap()), expected);
    }
}
