use super::*;
use crate::Crs;
use crate::test_support::assert_close;

// Reference point from undrift_gps `baidu_mercator.rs` (MIT).
const REF_XY: (f64, f64) = (12_949_156.80, 4_817_450.00);
const REF_BD09_LAT: f64 = 39.856_357;
const REF_BD09_LON: f64 = 116.322_989;

fn merc(x: f64, y: f64) -> BaiduMercator {
    BaiduMercator::new(x, y).unwrap()
}

fn bd(lat: f64, lon: f64) -> Bd09 {
    Bd09::new(lat, lon).unwrap()
}

#[test]
fn mercator_to_bd09_matches_reference() {
    let b = merc(REF_XY.0, REF_XY.1).try_to_bd09().unwrap();
    assert_close(b.lat(), REF_BD09_LAT, 1e-5);
    assert_close(b.lon(), REF_BD09_LON, 1e-5);
    // The southern / western mirror is the same fit with the signs flipped.
    let mirrored = merc(-REF_XY.0, -REF_XY.1).try_to_bd09().unwrap();
    assert_eq!((mirrored.lat(), mirrored.lon()), (-b.lat(), -b.lon()));
}

#[test]
fn mercator_round_trip_sub_meter() {
    let m = merc(REF_XY.0, REF_XY.1);
    let back = BaiduMercator::try_from_bd09(m.try_to_bd09().unwrap()).unwrap();
    // Independent forward/inverse fits: stable to well under a meter.
    assert_close(back.x(), m.x(), 0.5);
    assert_close(back.y(), m.y(), 0.5);
    assert_eq!(
        BaiduMercator::try_from(m.try_to_bd09().unwrap()).unwrap(),
        back
    );
    assert_eq!(Bd09::try_from(m).unwrap(), m.try_to_bd09().unwrap());
}

#[test]
fn bd09_round_trip_drift_matches_the_documented_bands() {
    // Worst ground drift of BD-09 → Mercator → BD-09 over a latitude band,
    // sampled every 0.1° of latitude and 1° of longitude (70°E–140°E).
    let worst_m = |from: u32, to: u32| {
        (from..to)
            .flat_map(|i| (70..140).map(move |lon| (f64::from(i) * 0.1, f64::from(lon))))
            .map(|(lat, lon)| {
                let m = BaiduMercator::try_from_bd09(bd(lat, lon)).unwrap();
                let b = m.try_to_bd09().unwrap();
                let d_north = (b.lat() - lat).to_radians();
                let d_east = (b.lon() - lon).to_radians() * lat.to_radians().cos();
                d_north.hypot(d_east) * 6_378_137.0
            })
            .fold(0.0_f64, f64::max)
    };
    let low = worst_m(1, 600);
    assert!(low < 0.3, "below 60°: {low} m");
    let high = worst_m(600, 741);
    assert!((1.0..8.0).contains(&high), "60°–74°: {high} m");
}

#[test]
fn coordinate_bridge_round_trips_and_checks_crs() {
    let m = merc(REF_XY.0, REF_XY.1);
    let coord = m.try_to_coordinate().unwrap();
    assert_eq!(coord.crs(), Crs::Bd09);
    assert_eq!(
        BaiduMercator::try_from_coordinate(coord).unwrap(),
        BaiduMercator::try_from(coord).unwrap()
    );
    // A non-BD-09 coordinate must be rejected, not silently reprojected.
    let wgs = Coordinate::wgs84(REF_BD09_LAT, REF_BD09_LON).unwrap();
    assert!(matches!(
        BaiduMercator::try_from(wgs),
        Err(Error::CrsMismatch { .. })
    ));
}

/// C17: Baidu clamps its projection to ±74°, so a latitude beyond it is
/// outside the domain rather than silently stretched.
#[test]
fn latitudes_beyond_74_degrees_are_outside_the_domain() {
    for lat in [74.000_001, 74.5, 80.0, -75.0, 90.0] {
        assert!(
            matches!(
                BaiduMercator::try_from_bd09(bd(lat, 10.0)),
                Err(Error::OutsideDomain {
                    operation: "Baidu Mercator",
                    ..
                })
            ),
            "lat {lat}"
        );
    }
    // 74° itself is inside, and lands exactly on the northing limit.
    let edge = BaiduMercator::try_from_bd09(bd(-74.0, 10.0)).unwrap();
    assert_close(edge.y(), -MAX_NORTHING_M, 1e-6);
    assert!(edge.try_to_bd09().is_ok());
}

/// The inverse rejects a northing beyond the 74° limit and an easting that
/// maps past the antimeridian, naming the component.
#[test]
fn inverse_rejects_values_outside_the_projection() {
    let invalid_field = |m: BaiduMercator| match m.try_to_bd09() {
        Err(Error::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    };
    assert_eq!(invalid_field(merc(0.0, MAX_NORTHING_M + 1.0)), "y");
    assert_eq!(invalid_field(merc(0.0, -MAX_NORTHING_M - 1.0)), "y");
    assert_eq!(invalid_field(merc(20_100_000.0, 0.0)), "x");
    assert_eq!(invalid_field(merc(-20_100_000.0, 0.0)), "x");
    // The limits themselves are inside.
    assert!(merc(0.0, MAX_NORTHING_M).try_to_bd09().is_ok());
    let east = merc(20_037_000.0, 0.0).try_to_bd09().unwrap();
    assert!(east.lon() > 179.99 && east.lon() <= 180.0, "{}", east.lon());
}

#[test]
fn construction_rejects_non_finite_components() {
    let field = |r: Result<BaiduMercator>| match r {
        Err(Error::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    };
    assert_eq!(field(BaiduMercator::new(f64::NAN, 0.0)), "x");
    assert_eq!(field(BaiduMercator::new(0.0, f64::INFINITY)), "y");
}

/// Each band edge selects the row above it, and anything below the last
/// edge the last row.
#[test]
fn lookup_selects_the_band_by_its_lower_edge() {
    assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, 60.0), &LL2MC[0]));
    assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, 59.999), &LL2MC[1]));
    assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, -15.0), &LL2MC[3]));
    assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, 14.999), &LL2MC[4]));
    assert!(std::ptr::eq(
        lookup(&MCBAND, &MC2LL, 8_362_377.87),
        &MC2LL[0]
    ));
    assert!(std::ptr::eq(lookup(&MCBAND, &MC2LL, 0.0), &MC2LL[4]));
}

/// The forward fit reproduces Baidu's value to the millimeter, including
/// the row's constant term (a few millimeters on its own).
#[test]
fn forward_matches_the_pinned_value() {
    let m =
        BaiduMercator::try_from_bd09(bd(39.922_699_552_216_216, 116.416_627_243_787_33)).unwrap();
    assert_close(m.x(), 12_959_580.673_891_693, 1e-6);
    assert_close(m.y(), 4_827_036.867_073_46, 1e-6);
}

/// An easting that maps exactly onto 180° of longitude is still inside.
#[test]
fn easting_mapping_exactly_to_180_degrees_is_inside() {
    let row = &MC2LL[4];
    let mut x: f64 = (180.0 - row[0]) / row[1];
    // `x` is a positive finite easting, so adjacent floats are one bit
    // pattern apart (`next_up`/`next_down` need Rust 1.86; MSRV is 1.85).
    assert!(x.is_finite() && x > 0.0);
    let next_up = |v: f64| f64::from_bits(v.to_bits() + 1);
    let next_down = |v: f64| f64::from_bits(v.to_bits() - 1);
    // Step to the float whose image is exactly 180.0.
    for _ in 0..64 {
        let lon = row[0] + row[1] * x;
        if lon == 180.0 {
            break;
        }
        x = if lon < 180.0 {
            next_up(x)
        } else {
            next_down(x)
        };
    }
    assert_eq!(row[0] + row[1] * x, 180.0, "no float maps exactly to 180°");
    assert_eq!(merc(x, 0.0).try_to_bd09().unwrap().lon(), 180.0);
    assert!(merc(next_up(next_up(x)), 0.0).try_to_bd09().is_err());
}

/// The northing limit is the forward image of 74°, to the last digit.
#[test]
fn northing_limit_is_the_image_of_74_degrees() {
    let m = BaiduMercator::try_from_bd09(bd(MAX_LAT_DEG, 0.0)).unwrap();
    assert_close(m.y(), MAX_NORTHING_M, 1e-6);
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_through_the_record() {
    let m = merc(REF_XY.0, REF_XY.1);
    let json = serde_json::to_string(&m).unwrap();
    assert_eq!(json, r#"{"x":12949156.8,"y":4817450.0}"#);
    assert_eq!(serde_json::from_str::<BaiduMercator>(&json).unwrap(), m);
    // JSON cannot carry a non-finite number, so validation is exercised
    // directly on the record conversion.
    let bad = BaiduMercatorRecord {
        x: f64::NAN,
        y: 0.0,
    };
    assert!(BaiduMercator::try_from(bad).is_err());
}

/// Every coefficient row, in both directions, reproduces Baidu's formula to
/// well under its smallest term. The rows' constant terms move a result by
/// only millimeters (forward: a fraction of a millimeter of easting; inverse:
/// about 1e-8° of longitude or latitude), so a sign slip in one of them
/// shows up only against a pinned value at this tolerance.
#[test]
fn every_band_matches_its_pinned_value_in_both_directions() {
    let forward = [
        (65.5, 9_702_066.318_423_39, 11_159_900.380_975_151),
        (50.5, 6_500_035.177_311_653, 11_159_900.380_900_662),
        (35.5, 4_206_995.350_022_907, 11_159_900.379_802_871),
        (20.5, 2_317_259.885_098_630_6, 11_159_900.380_938_785),
        (5.5, 609_068.405_959_643_2, 11_159_900.382_211_877),
    ];
    for (lat, y, x) in forward {
        let m = BaiduMercator::try_from_bd09(bd(lat, 100.25)).unwrap();
        assert_close(m.x(), x, 1e-6);
        assert_close(m.y(), y, 1e-6);
    }
    let inverse = [
        (9_000_000.0, 62.746_456_182_055_98, 98.813_606_067_552_77),
        (6_000_000.0, 47.547_043_831_489_61, 98.813_606_067_884_73),
        (4_000_000.0, 33.964_920_847_828_58, 98.813_606_077_755_08),
        (2_000_000.0, 17.791_619_404_397_78, 98.813_606_068_025_62),
        (1_000_000.0, 9.006_447_877_187_098, 98.813_606_056_750_34),
    ];
    for (y, lat, lon) in inverse {
        let b = merc(11_000_000.0, y).try_to_bd09().unwrap();
        assert_close(b.lat(), lat, 1e-11);
        assert_close(b.lon(), lon, 1e-11);
    }
}
