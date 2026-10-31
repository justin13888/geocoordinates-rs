//! `segment_visits` and `segment_trips`, and a golden run of the whole `cluster`
//! surface over a synthetic multi-city photo set.
#![cfg(feature = "cluster")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use geocoordinates::cluster::{
    ClusterIndex, PointLabel, Visit, bucket_by_geohash, centroid, dbscan, segment_trips,
    segment_visits,
};
use geocoordinates::fix::Fix;
use geocoordinates::index::GeoBounds;
use geocoordinates::{Coordinate, Crs, Error, Length};
use proptest::prelude::*;

const MINUTE: u64 = 60;
const HOUR: u64 = 3_600;
const DAY: u64 = 86_400;

fn at(second: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(second)
}

fn fix(lat: f64, lon: f64, second: u64) -> Fix {
    let mut fix = Fix::from_coord(Coordinate::wgs84(lat, lon).unwrap());
    fix.timestamp = Some(at(second));
    fix
}

#[test]
fn segment_visits_reports_a_visit_with_no_mean() {
    // A distance around half the planet keeps antipodes in one run.
    let fixes = [fix(10.0, 20.0, 0), fix(-10.0, -160.0, MINUTE)];
    assert!(matches!(
        segment_visits(&fixes, Length::from_meters(21_000_000.0), Duration::ZERO),
        Err(Error::InvalidValue {
            field: "points",
            ..
        })
    ));
}

fn near() -> Length {
    Length::from_meters(100.0)
}

#[test]
fn a_visit_needs_both_proximity_and_dwell() {
    let fixes = [
        fix(0.0, 0.0, 0),
        fix(0.0, 0.0001, 5 * MINUTE),
        fix(0.0, 0.0002, 10 * MINUTE),
        fix(1.0, 1.0, 11 * MINUTE),
        fix(2.0, 2.0, 12 * MINUTE),
    ];
    let visits = segment_visits(&fixes, near(), Duration::from_secs(10 * MINUTE)).unwrap();
    assert_eq!(visits.len(), 1);
    assert_eq!(visits[0].cluster().members(), [0, 1, 2]);
    assert_eq!(visits[0].cluster().id(), 0);
    assert_eq!(visits[0].arrival(), at(0));
    assert_eq!(visits[0].departure(), at(10 * MINUTE));

    // One second more dwell than that run lasted, and it is no visit.
    let longer = Duration::from_secs(10 * MINUTE + 1);
    assert!(segment_visits(&fixes, near(), longer).unwrap().is_empty());

    // Zero dwell makes every run a visit, even a lone fix.
    let all = segment_visits(&fixes, near(), Duration::ZERO).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].cluster().id(), 2);
    assert_eq!(all[2].arrival(), all[2].departure());
}

#[test]
fn distance_is_measured_from_the_run_s_first_fix_and_includes_the_limit() {
    let (a, b) = (fix(0.0, 0.0, 0), fix(0.0, 0.001, HOUR));
    let apart = geocoordinates::geodesy::geodesic_distance(a.coord, b.coord).unwrap();
    let dwell = Duration::from_secs(MINUTE);
    assert_eq!(
        segment_visits(&[a.clone(), b.clone()], apart, dwell)
            .unwrap()
            .len(),
        1
    );
    let under = Length::from_meters(apart.meters() - 1e-3);
    assert!(segment_visits(&[a, b], under, dwell).unwrap().is_empty());

    // A slow drift leaves the anchor's radius even though each step is small.
    let drift: Vec<Fix> = (0..10)
        .map(|i| fix(0.0, f64::from(i) * 0.0005, u64::from(i as u32) * HOUR))
        .collect();
    let visits = segment_visits(&drift, near(), Duration::from_secs(MINUTE)).unwrap();
    assert_eq!(visits[0].cluster().members(), [0, 1]);
    assert_eq!(visits[1].cluster().members(), [2, 3]);
}

#[test]
fn a_short_run_gives_up_only_its_first_fix() {
    // The first fix has too little time with its neighbors, but the second anchors a
    // visit that lasts.
    let fixes = [
        fix(0.0, 0.0, 0),
        fix(0.0, 0.0008, MINUTE),
        fix(0.0, 0.0012, 2 * MINUTE),
        fix(0.0, 0.0012, 2 * HOUR),
    ];
    let visits = segment_visits(&fixes, near(), Duration::from_secs(HOUR)).unwrap();
    assert_eq!(visits.len(), 1);
    assert_eq!(visits[0].cluster().members(), [1, 2, 3]);
    assert_eq!(visits[0].arrival(), at(MINUTE));
}

#[test]
fn visits_stay_whole_across_the_antimeridian_and_the_pole() {
    let dwell = Duration::from_secs(MINUTE);
    let crossing = [fix(-17.0, 179.9999, 0), fix(-17.0, -179.9999, HOUR)];
    let visits = segment_visits(&crossing, near(), dwell).unwrap();
    assert_eq!(visits[0].cluster().count(), 2);
    assert!(visits[0].cluster().bounds().crosses_antimeridian());

    let polar = [fix(89.9999, 0.0, 0), fix(89.9999, 180.0, HOUR)];
    let visits = segment_visits(&polar, Length::from_meters(50.0), dwell).unwrap();
    assert_eq!(visits[0].cluster().count(), 2);
    assert!((visits[0].cluster().centroid().value().lat() - 90.0).abs() < 1e-6);
}

#[test]
fn segment_visits_checks_its_input() {
    let dwell = Duration::ZERO;
    // A zero distance is valid: it groups fixes at one exact position.
    let same = [fix(1.0, 2.0, 0), fix(1.0, 2.0, 5)];
    let zero = Length::from_meters(0.0);
    assert_eq!(segment_visits(&same, zero, dwell).unwrap().len(), 1);
    assert!(segment_visits(&[], near(), dwell).unwrap().is_empty());
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            segment_visits(&[], Length::from_meters(bad), dwell),
            Err(Error::InvalidValue {
                field: "distance",
                ..
            })
        ));
    }
    let timeless = Fix::from_coord(Coordinate::wgs84(0.0, 0.0).unwrap());
    let Err(Error::InvalidValue { field, detail }) =
        segment_visits(&[fix(0.0, 0.0, 0), timeless], near(), dwell)
    else {
        panic!("a fix with no time is an error");
    };
    assert_eq!(field, "timestamp");
    assert!(detail.contains("fix 1"), "{detail}");

    let mut gcj = Fix::from_coord(Coordinate::gcj02(1.0, 1.0).unwrap());
    gcj.timestamp = Some(at(0));
    assert!(matches!(
        segment_visits(&[gcj], near(), dwell),
        Err(Error::CrsMismatch { .. })
    ));
    let mut tokyo = Fix::from_coord(Coordinate::new(0.0, 0.0, Crs::Tokyo).unwrap());
    tokyo.timestamp = Some(at(1));
    assert!(matches!(
        segment_visits(&[fix(0.0, 0.0, 0), tokyo], near(), dwell),
        Err(Error::CrsMismatch { .. })
    ));
}

proptest! {
    #[test]
    fn visits_do_not_depend_on_input_order(
        steps in prop::collection::vec((0_u64..4, 0_i32..3, 0_i32..3), 0..30),
        seed in any::<u64>(),
    ) {
        let mut second = 0;
        let fixes: Vec<Fix> = steps
            .iter()
            .map(|(wait, dy, dx)| {
                second += wait * 20 * MINUTE;
                fix(f64::from(*dy) * 1e-2, f64::from(*dx) * 1e-2, second)
            })
            .collect();
        let mut shuffled: Vec<usize> = (0..fixes.len()).collect();
        let mut state = seed | 1;
        for i in (1..shuffled.len()).rev() {
            state = state.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
            shuffled.swap(i, (state % (i as u64 + 1)) as usize);
        }
        let reordered: Vec<Fix> = shuffled.iter().map(|i| fixes[*i].clone()).collect();
        let dwell = Duration::from_secs(10 * MINUTE);
        let one = segment_visits(&fixes, near(), dwell).unwrap();
        let two = segment_visits(&reordered, near(), dwell).unwrap();
        prop_assert_eq!(one.len(), two.len());
        for (a, b) in one.iter().zip(&two) {
            prop_assert_eq!((a.arrival(), a.departure()), (b.arrival(), b.departure()));
            prop_assert_eq!(a.cluster().count(), b.cluster().count());
            prop_assert_eq!(a.cluster().bounds(), b.cluster().bounds());
        }
    }
}

fn visit_at(lat: f64, lon: f64, from: u64, to: u64) -> Visit {
    let point = Coordinate::wgs84(lat, lon).unwrap();
    let cluster = dbscan(&[point], Length::from_meters(1.0), 1)
        .unwrap()
        .clusters()[0]
        .clone();
    Visit::new(cluster, at(from), at(to)).unwrap()
}

#[test]
fn trips_split_on_the_gap_and_include_it() {
    let visits = [
        visit_at(0.0, 0.0, 0, HOUR),
        visit_at(1.0, 1.0, 2 * HOUR, 3 * HOUR),
        visit_at(2.0, 2.0, 5 * HOUR, 6 * HOUR),
    ];
    // Exactly the gap keeps them together.
    let gap = Duration::from_secs(2 * HOUR);
    assert_eq!(segment_trips(&visits, gap).unwrap().len(), 1);
    let narrow = Duration::from_secs(2 * HOUR - 1);
    let trips = segment_trips(&visits, narrow).unwrap();
    assert_eq!(trips.len(), 2);
    assert_eq!(trips[0].visits(), [0, 1]);
    assert_eq!((trips[0].start(), trips[0].end()), (at(0), at(3 * HOUR)));
    assert_eq!(trips[0].bounds().north(), 1.0);
    assert_eq!(trips[1].visits(), [2]);
    assert_eq!(
        (trips[1].start(), trips[1].end()),
        (at(5 * HOUR), at(6 * HOUR))
    );
    assert!(segment_trips(&[], gap).unwrap().is_empty());
}

#[test]
fn trips_take_visits_in_time_order_and_merge_overlaps() {
    let visits = [
        visit_at(2.0, 2.0, 10 * DAY, 10 * DAY + HOUR),
        visit_at(0.0, 0.0, 0, DAY),
        visit_at(1.0, 1.0, HOUR, 2 * HOUR), // inside the one before
        visit_at(3.0, 3.0, DAY + HOUR, DAY + 2 * HOUR),
    ];
    let trips = segment_trips(&visits, Duration::from_secs(2 * HOUR)).unwrap();
    assert_eq!(trips.len(), 2);
    assert_eq!(trips[0].visits(), [1, 2, 3]);
    // The end is the latest departure, though the last-listed left a while before.
    assert_eq!(trips[0].end(), at(DAY + 2 * HOUR));
    assert_eq!(trips[1].visits(), [0]);

    let held = [
        visit_at(0.0, 0.0, 0, DAY),
        visit_at(1.0, 1.0, HOUR, 2 * HOUR),
    ];
    assert_eq!(
        segment_trips(&held, Duration::ZERO).unwrap()[0].end(),
        at(DAY)
    );
}

#[test]
fn trips_refuse_mixed_datums_whatever_the_gap() {
    let tokyo = Coordinate::new(1.0, 2.0, Crs::Tokyo).unwrap();
    let cluster = dbscan(&[tokyo], Length::from_meters(1.0), 1)
        .unwrap()
        .clusters()[0]
        .clone();
    let far_off = Visit::new(cluster, at(100 * DAY), at(100 * DAY)).unwrap();
    assert!(matches!(
        segment_trips(&[visit_at(0.0, 0.0, 0, 1), far_off], Duration::ZERO),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn a_trip_crossing_the_antimeridian_keeps_a_small_box() {
    let visits = [visit_at(0.0, 179.0, 0, 1), visit_at(0.0, -179.0, 2, 3)];
    let trip = &segment_trips(&visits, Duration::from_secs(9)).unwrap()[0];
    assert!(trip.bounds().crosses_antimeridian());
    assert_eq!(
        (trip.bounds().west(), trip.bounds().east()),
        (179.0, -179.0)
    );
}

#[cfg(feature = "serde")]
#[test]
fn visits_and_trips_serde_rejects_impossible_times() {
    let visits = [visit_at(0.0, 0.0, 10, 20)];
    let json = serde_json::to_string(&visits[0]).unwrap();
    assert_eq!(serde_json::from_str::<Visit>(&json).unwrap(), visits[0]);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut flipped = value.clone();
    flipped["arrival"] = value["departure"].clone();
    flipped["departure"] = value["arrival"].clone();
    assert!(serde_json::from_value::<Visit>(flipped).is_err());

    let trip = segment_trips(&visits, Duration::ZERO).unwrap()[0].clone();
    let trip_json = serde_json::to_value(&trip).unwrap();
    assert_eq!(
        serde_json::from_value::<geocoordinates::cluster::Trip>(trip_json.clone()).unwrap(),
        trip
    );
    // A trip of one instant is valid.
    let instant = [visit_at(0.0, 0.0, 10, 10)];
    let instant = segment_trips(&instant, Duration::ZERO).unwrap()[0].clone();
    let instant_json = serde_json::to_string(&instant).unwrap();
    assert_eq!(
        serde_json::from_str::<geocoordinates::cluster::Trip>(&instant_json).unwrap(),
        instant
    );
    let mut empty = trip_json.clone();
    empty["visits"] = serde_json::json!([]);
    assert!(serde_json::from_value::<geocoordinates::cluster::Trip>(empty).is_err());
    let mut flipped = trip_json.clone();
    flipped["start"] = trip_json["end"].clone();
    flipped["end"] = trip_json["start"].clone();
    assert!(serde_json::from_value::<geocoordinates::cluster::Trip>(flipped).is_err());
}

/// Six days of photos in Paris, Tokyo, Suva across ±180°, and Longyearbyen far
/// north, with a stray shot at sea, each place a tight grid of 25 shots.
fn photo_set() -> Vec<Fix> {
    let mut fixes = Vec::new();
    let mut shoot = |lat: f64, lon: f64, start: u64| {
        for i in 0..25_u32 {
            let (row, column) = (f64::from(i / 5), f64::from(i % 5));
            let mut shot_lon = lon + column * 1e-4;
            if shot_lon > 180.0 {
                shot_lon -= 360.0;
            }
            fixes.push(fix(
                lat + row * 1e-4,
                shot_lon,
                start + u64::from(i) * MINUTE,
            ));
        }
    };
    shoot(48.8566, 2.3522, 0);
    shoot(35.6762, 139.6503, DAY);
    shoot(-18.1416, 179.9998, 2 * DAY);
    shoot(78.2232, 15.6267, 5 * DAY);
    fixes.push(fix(-40.0, -100.0, 6 * DAY));
    fixes
}

#[test]
fn golden_multi_city_photo_set() {
    let fixes = photo_set();
    let points: Vec<Coordinate> = fixes.iter().map(|fix| fix.coord).collect();

    // Density clusters: one per city, with the shot at sea as noise.
    let places = dbscan(&points, Length::from_meters(50.0), 3).unwrap();
    assert_eq!(places.clusters().len(), 4);
    assert_eq!(places.labels()[100], PointLabel::Noise);
    let sizes: Vec<usize> = places.clusters().iter().map(|c| c.count()).collect();
    assert_eq!(sizes, [25, 25, 25, 25]);

    let golden = [
        (48.8568, 2.3524),
        (35.6764, 139.6505),
        (-18.1414, 180.0),
        (78.2234, 15.6269),
    ];
    for (cluster, (lat, lon)) in places.clusters().iter().zip(golden) {
        let middle = cluster.centroid();
        assert!((middle.value().lat() - lat).abs() < 1e-6, "{middle:?}");
        assert!((middle.value().lon() - lon).abs() < 1e-6, "{middle:?}");
        assert!(middle.max_error().meters() < 50.0, "{middle:?}");
    }
    let suva = places.clusters()[2].bounds();
    assert!(suva.crosses_antimeridian());
    assert!((suva.west() - 179.9998).abs() < 1e-9 && (suva.east() + 179.9998).abs() < 1e-9);

    // Whole-set centroid: the photos are spread enough to have a mean.
    let whole = centroid(&points).unwrap();
    assert!(whole.max_error().meters() > 10_000_000.0);

    // Visits and trips: one visit per city, and two trips around the three-day gap.
    let dwell = Duration::from_secs(10 * MINUTE);
    let visits = segment_visits(&fixes, Length::from_meters(100.0), dwell).unwrap();
    assert_eq!(visits.len(), 4);
    let counts: Vec<usize> = visits.iter().map(|visit| visit.cluster().count()).collect();
    assert_eq!(counts, [25, 25, 25, 25]);
    assert_eq!(visits[1].arrival(), at(DAY));
    let trips = segment_trips(&visits, Duration::from_secs(2 * DAY)).unwrap();
    assert_eq!(
        trips.iter().map(|t| t.visits().len()).collect::<Vec<_>>(),
        [3, 1]
    );

    // Map pins: one pin per city from far out, one per photo up close.
    let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap();
    let index = ClusterIndex::new(points.iter().copied(), Default::default()).unwrap();
    assert_eq!(index.clusters(world, 5).unwrap().len(), 5);
    assert_eq!(index.clusters(world, 22).unwrap().len(), 101);
    let pacific = GeoBounds::new(-20.0, 170.0, -10.0, -170.0, Crs::Wgs84).unwrap();
    let pins = index.clusters(pacific, 5).unwrap();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].count(), 25);

    // The cheap mode gets the cities too, at a coarse enough cell, except that Suva
    // straddles the antimeridian, which is always a geohash edge.
    let buckets = bucket_by_geohash(&points, 4).unwrap();
    assert_eq!(buckets.len(), 6);
    assert_eq!(buckets[0].cluster().count(), 25);
}
