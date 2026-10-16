//! Reference values are GeographicLib's `RhumbSolve` (Karney's `Rhumb` class,
//! exact mode) on WGS-84 unless noted, run through the online calculator at
//! <https://geographiclib.sourceforge.io/cgi-bin/RhumbSolve> with `prec=9`,
//! and quoted as `RhumbSolve I lat1 lon1 lat2 lon2` (inverse) or
//! `RhumbSolve D lat1 lon1 azi12 s12` (direct). Its DMS output is converted to
//! decimal degrees.

use super::*;
use crate::error::Error;
use crate::geodesy::Ellipsoid;
use crate::test_support::assert_close;

/// Distance tolerance against RhumbSolve: 10 µm.
const METERS: f64 = 1e-5;
/// Angle tolerance against RhumbSolve: 1e-10° (about 10 µm on the ground).
const DEGREES: f64 = 1e-10;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn m(meters: f64) -> Length {
    Length::from_meters(meters)
}

#[track_caller]
fn assert_inverse(a: Coordinate, b: Coordinate, azi12: f64, s12: f64) {
    assert_close(rhumb_bearing(&a, &b).unwrap(), azi12, DEGREES);
    assert_close(rhumb_distance(&a, &b).unwrap().meters(), s12, METERS);
}

#[track_caller]
fn assert_direct(start: Coordinate, azi12: f64, s12: f64, lat2: f64, lon2: f64) {
    let p = rhumb_destination(&start, azi12, m(s12)).unwrap();
    assert_close(p.lat(), lat2, DEGREES);
    assert_close(p.lon(), lon2, DEGREES);
    assert_eq!(p.crs(), start.crs());
    assert_eq!(p.height(), None);
}

#[track_caller]
fn assert_invalid(r: Result<Coordinate>, field: &str) {
    assert!(
        matches!(&r, Err(Error::InvalidValue { field: f, .. }) if *f == field),
        "{r:?}"
    );
}

#[test]
fn inverse_matches_rhumbsolve_on_oblique_lines() {
    // RhumbSolve I 40 -75 51.5 0.
    assert_inverse(
        c(40.0, -75.0),
        c(51.5, 0.0),
        77.582_666_538_251_8,
        5_944_187.084_396_951,
    );
    // RhumbSolve I 40 -75 45 -70.
    assert_inverse(
        c(40.0, -75.0),
        c(45.0, -70.0),
        36.477_204_009_051_58,
        690_734.818_615_040,
    );
    // RhumbSolve I 40.7128 -74.006 51.5074 -0.1278 (New York → London).
    assert_inverse(
        c(40.7128, -74.006),
        c(51.5074, -0.1278),
        78.081_345_760_192_42,
        5_809_801.212_288_193,
    );
    // Reversed, the line runs the other way at the same length.
    assert_inverse(
        c(51.5, 0.0),
        c(40.0, -75.0),
        180.0 + 77.582_666_538_251_8,
        5_944_187.084_396_951,
    );
}

#[test]
fn inverse_on_a_parallel_takes_the_east_west_limit() {
    // |Δψ| = 0: the scale is its limit, the radius of the parallel.
    // RhumbSolve I 0 0 0 1 and I 40 0 40 1.
    assert_inverse(c(0.0, 0.0), c(0.0, 1.0), 90.0, 111_319.490_793_274);
    assert_inverse(c(40.0, 0.0), c(40.0, 1.0), 90.0, 85_393.856_958_618);
    assert_inverse(c(40.0, 1.0), c(40.0, 0.0), 270.0, 85_393.856_958_618);
    // |Δψ| > 0 but tiny: the closed-form differences keep the scale accurate,
    // where a ratio of subtracted arcs would lose most of its digits.
    // RhumbSolve I 40 0 40.000000001 1.
    assert_inverse(
        c(40.0, 0.0),
        c(40.000_000_001, 1.0),
        89.999_999_925_500_3,
        85_393.856_957_996,
    );
    // Smaller still (|Δψ| ≈ 2e-15), the length is the parallel's.
    assert_close(
        rhumb_distance(&c(40.0, 0.0), &c(40.0 + 1e-13, 1.0))
            .unwrap()
            .meters(),
        85_393.856_958_618,
        METERS,
    );
}

#[test]
fn inverse_across_the_antimeridian_takes_the_short_way() {
    // RhumbSolve I 10 179.5 10 -179.5.
    assert_inverse(c(10.0, 179.5), c(10.0, -179.5), 90.0, 109_639.364_068_153);
    assert_inverse(c(10.0, -179.5), c(10.0, 179.5), 270.0, 109_639.364_068_153);
}

#[test]
fn inverse_to_or_from_a_pole_is_the_meridian() {
    // RhumbSolve I -90 0 90 0: the full meridian, pole to pole.
    assert_inverse(c(-90.0, 0.0), c(90.0, 0.0), 0.0, 20_003_931.458_625_446);
    // RhumbSolve I 80 0 90 90: arriving at a pole, the longitude is moot.
    assert_inverse(c(80.0, 0.0), c(90.0, 90.0), 0.0, 1_116_825.857_375_85);
    // Leaving a pole the distance is RhumbSolve I 90 0 0 90's, while the
    // bearing, taken against the start's own meridian, names meridian 90° E
    // (RhumbSolve leaves it at 180° whatever the longitudes).
    assert_inverse(c(90.0, 0.0), c(0.0, 90.0), 90.0, 10_001_965.729_312_723);
    // Southward to the south pole.
    assert_inverse(c(0.0, 0.0), c(-90.0, 45.0), 180.0, 10_001_965.729_312_723);
}

#[test]
fn coincident_points_have_no_bearing() {
    // As for `initial_bearing`: the same place, including a pole named at two
    // longitudes and a point named on both sides of the antimeridian, has a
    // zero distance but no bearing.
    for (a, b) in [
        (c(12.0, 34.0), c(12.0, 34.0)),
        (c(90.0, 0.0), c(90.0, 45.0)),
        (c(-90.0, 10.0), c(-90.0, -80.0)),
        (c(10.0, -180.0), c(10.0, 180.0)),
    ] {
        assert_eq!(rhumb_distance(&a, &b).unwrap().meters(), 0.0, "{a:?} {b:?}");
        assert!(
            matches!(
                rhumb_bearing(&a, &b),
                Err(Error::InvalidValue {
                    field: "points",
                    ..
                })
            ),
            "{a:?} {b:?}"
        );
    }
    // A distinct point however close has a bearing, on a parallel or a meridian.
    assert_close(
        rhumb_bearing(&c(0.0, 0.0), &c(0.0, 1e-9)).unwrap(),
        90.0,
        DEGREES,
    );
    assert_close(
        rhumb_bearing(&c(0.0, 0.0), &c(1e-9, 0.0)).unwrap(),
        0.0,
        DEGREES,
    );
    assert_close(
        rhumb_bearing(&c(89.0, 0.0), &c(90.0, 0.0)).unwrap(),
        0.0,
        DEGREES,
    );
}

#[test]
fn inverse_runs_on_the_datum_ellipsoid() {
    // Tokyo is on Bessel 1841. RhumbSolve I 40 -75 51.5 0 with
    // a = 6377397.155 m, f = 1/299.1528128.
    let t = |lat, lon| Coordinate::new(lat, lon, Crs::Tokyo).unwrap();
    assert_inverse(
        t(40.0, -75.0),
        t(51.5, 0.0),
        77.582_548_951_898_42,
        5_943_470.063_790_459,
    );
}

#[test]
fn direct_runs_on_the_datum_ellipsoid_and_keeps_its_crs() {
    // Tokyo is on Bessel 1841. RhumbSolve D 40 -75 45 2000000 with
    // a = 6377397.155 m, f = 1/299.1528128 reaches
    // 52°43'26.2184964665"N 056°29'53.1349326109"W.
    let start = Coordinate::new(40.0, -75.0, Crs::Tokyo).unwrap();
    assert_direct(
        start,
        45.0,
        2_000_000.0,
        52.723_949_582_351_81,
        -56.498_093_036_836_36,
    );
    assert_eq!(
        rhumb_destination(&start, 45.0, m(2_000_000.0))
            .unwrap()
            .crs(),
        Crs::Tokyo
    );
}

#[test]
fn inverse_rejects_obfuscated_and_mixed_datums() {
    let gcj = Coordinate::new(30.0, 120.0, Crs::Gcj02).unwrap();
    let bd = Coordinate::new(30.0, 120.0, Crs::Bd09).unwrap();
    for (a, b) in [(gcj, gcj), (bd, bd), (c(30.0, 120.0), gcj)] {
        assert!(
            matches!(rhumb_distance(&a, &b), Err(Error::CrsMismatch { .. })),
            "{a:?} {b:?}"
        );
        assert!(matches!(
            rhumb_bearing(&a, &b),
            Err(Error::CrsMismatch { .. })
        ));
    }
    assert!(matches!(
        rhumb_destination(&gcj, 45.0, m(1.0)),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn direct_matches_rhumbsolve() {
    // RhumbSolve D 40 0 45 2000000: 45° from 40°N.
    assert_direct(
        c(40.0, 0.0),
        45.0,
        2_000_000.0,
        52.722_530_833_156_45,
        18.499_393_730_640_307,
    );
    // RhumbSolve D -40 0 225 2000000: its mirror image in the south-west.
    assert_direct(
        c(-40.0, 0.0),
        225.0,
        2_000_000.0,
        -52.722_530_833_156_45,
        -18.499_393_730_640_307,
    );
    // RhumbSolve D 40.7128 -74.006 45 100000.
    assert_direct(
        c(40.7128, -74.006),
        45.0,
        100_000.0,
        41.349_520_443_859_53,
        -73.165_151_728_798_76,
    );
    // RhumbSolve D 40 0 90 100000: due east holds the latitude.
    assert_direct(c(40.0, 0.0), 90.0, 100_000.0, 40.0, 1.171_044_423_587_278);
    // A bearing outside [0, 360) is normalized first.
    assert_direct(c(40.0, 0.0), -270.0, 100_000.0, 40.0, 1.171_044_423_587_278);
    // RhumbSolve D 80 0 45 1579000: 300 m short of the pole, after spiralling
    // a third of the way round it. So close in, 1e-7° of longitude is 0.5 µm
    // on the ground, which is the tolerance the longitude gets.
    let p = rhumb_destination(&c(80.0, 0.0), 45.0, m(1_579_000.0)).unwrap();
    assert_close(p.lat(), 89.997_276_040_450_03, DEGREES);
    assert_close(p.lon(), 110.438_139_105_071_14, 1e-7);
}

#[test]
fn direct_across_the_antimeridian_wraps() {
    // RhumbSolve D 10 179.5 90 110000.
    assert_direct(
        c(10.0, 179.5),
        90.0,
        110_000.0,
        10.0,
        -179.496_710_707_555_5,
    );
}

#[test]
fn direct_round_trips_the_inverse() {
    for (a, b) in [
        (c(40.0, -75.0), c(45.0, -70.0)),
        (c(-33.9, 151.2), c(51.5, -0.1)),
        (c(10.0, 179.5), c(-10.0, -179.5)),
        (c(60.0, 10.0), c(60.0, 30.0)),
    ] {
        let p = rhumb_destination(
            &a,
            rhumb_bearing(&a, &b).unwrap(),
            rhumb_distance(&a, &b).unwrap(),
        )
        .unwrap();
        assert_close(p.lat(), b.lat(), DEGREES);
        assert_close(p.lon(), b.lon(), DEGREES);
    }
}

#[test]
fn direct_non_meridional_reaching_a_pole_is_invalid() {
    // C7: a non-meridional rhumb line spirals into the pole and never crosses
    // it, so a distance that would carry it to or past the pole has no
    // destination (RhumbSolve D 80 0 45 2000000 has a NaN longitude).
    assert_invalid(
        rhumb_destination(&c(80.0, 0.0), 45.0, m(2_000_000.0)),
        "distance",
    );
    // Exactly onto the pole: the meridian arc from 80°N to the pole is
    // 1 116 825.857 375 850 m (RhumbSolve I 80 0 90 90), so along 45° the line
    // is √2 times that.
    let to_pole = m(1_116_825.857_375_85 * core::f64::consts::SQRT_2);
    assert_invalid(rhumb_destination(&c(80.0, 0.0), 45.0, to_pole), "distance");
    // …and into the south pole.
    assert_invalid(
        rhumb_destination(&c(-80.0, 0.0), 135.0, to_pole),
        "distance",
    );
}

#[test]
fn direct_from_inside_the_pole_band_may_leave_or_stay() {
    // 89.99999999999°N is 1e-11° (about 1 µm) from the pole, inside the 1e-12
    // rad band, but not on the pole: the inverse functions accept it, and so
    // does the direct problem unless the line heads further poleward.
    let start = c(89.999_999_999_99, 0.0);
    // Zero distance stays put.
    let p = rhumb_destination(&start, 45.0, m(0.0)).unwrap();
    assert_close(p.lat(), start.lat(), DEGREES);
    assert_eq!(p.lon(), 0.0);
    // A southward line leaves the band. RhumbSolve D 89.99999999999 0 225 1000
    // reaches 89°59'37.2092960924"N 081°09'43.0962803606"W. The latitude
    // matches to 1e-10°. The longitude is only good to 0.02° (about 0.25 m
    // here, where 1° of longitude is 12 m): f64 holds the start's 1.7e-13 rad
    // gap to the pole to only ~3 digits, and the longitude swept is
    // proportional to its logarithm. This line does not stay clear of the
    // pole, so the module's sub-millimeter bound does not cover it.
    let p = rhumb_destination(&start, 225.0, m(1_000.0)).unwrap();
    assert_close(p.lat(), 89.993_669_248_914_56, DEGREES);
    assert_close(p.lon(), -81.161_971_188_989_06, 0.02);
    // The southern mirror image.
    let p = rhumb_destination(&c(-89.999_999_999_99, 0.0), 315.0, m(1_000.0)).unwrap();
    assert!(p.lat() > -89.999_999_999_99, "{p:?}");
    // Heading poleward from there still reaches the pole.
    assert_invalid(rhumb_destination(&start, 45.0, m(1.0)), "distance");
    assert_invalid(
        rhumb_destination(&c(-89.999_999_999_99, 0.0), 135.0, m(1.0)),
        "distance",
    );
}

#[test]
fn direct_any_bearing_from_a_pole_names_a_meridian() {
    // RhumbSolve D 90 30 45 1000000 has a NaN longitude; here the bearing is
    // read against the start's meridian, so it names meridian 30 + 180 − 45.
    assert_direct(c(90.0, 30.0), 45.0, 1e6, 81.046_232_815_950_6, 165.0);
    // From the south pole bearing β runs up meridian 30 + β.
    assert_direct(c(-90.0, 30.0), 90.0, 1e6, -81.046_232_815_950_6, 120.0);
    // Past the far pole the line carries on up that meridian's far side.
    assert_direct(c(90.0, 0.0), 135.0, 1e6, 81.046_232_815_950_6, 45.0);
    let pole_to_pole = 20_003_931.458_625_446;
    assert_direct(
        c(90.0, 0.0),
        135.0,
        pole_to_pole + 1e6,
        -81.046_232_815_950_6,
        -135.0,
    );
    // A zero distance stays on the pole with its own longitude, whatever the
    // bearing (the meridian it names is not taken).
    let p = rhumb_destination(&c(90.0, 30.0), 0.0, m(0.0)).unwrap();
    assert_eq!((p.lat(), p.lon()), (90.0, 30.0));
}

#[test]
fn issue_175_pole_start_round_trips() {
    // From (90, 0) to (0, 90): the destination used to run down meridian 0°
    // and land 10 000 km from the target.
    let (a, b) = (c(90.0, 0.0), c(0.0, 90.0));
    let p = rhumb_destination(
        &a,
        rhumb_bearing(&a, &b).unwrap(),
        rhumb_distance(&a, &b).unwrap(),
    )
    .unwrap();
    assert_close(p.lat(), 0.0, DEGREES);
    assert_close(p.lon(), 90.0, DEGREES);
    // The bearing names b's meridian from either pole, across the antimeridian.
    assert_close(
        rhumb_bearing(&c(90.0, 170.0), &c(10.0, -170.0)).unwrap(),
        160.0,
        DEGREES,
    );
    assert_close(
        rhumb_bearing(&c(-90.0, 170.0), &c(10.0, -170.0)).unwrap(),
        20.0,
        DEGREES,
    );
    // Pole to pole: the meridian of the target's longitude.
    assert_close(
        rhumb_bearing(&c(90.0, 0.0), &c(-90.0, 45.0)).unwrap(),
        135.0,
        DEGREES,
    );
    assert_close(
        rhumb_bearing(&c(-90.0, 0.0), &c(90.0, 0.0)).unwrap(),
        0.0,
        DEGREES,
    );
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(1024))]

    /// From any start, the poles and the antimeridian included, the direct
    /// problem inverts the inverse one to within 1 mm, or the bearing is
    /// rejected because the two points are one place.
    #[test]
    fn direct_inverts_the_inverse_everywhere(
        a_lat in latitude(), a_lon in longitude(),
        b_lat in latitude(), b_lon in longitude(),
    ) {
        let (a, b) = (c(a_lat, a_lon), c(b_lat, b_lon));
        let distance = rhumb_distance(&a, &b).unwrap();
        match rhumb_bearing(&a, &b) {
            Ok(bearing) => {
                let p = rhumb_destination(&a, bearing, distance).unwrap();
                let miss = crate::geodesy::geodesic_distance(&p, &b).unwrap().meters();
                proptest::prop_assert!(miss < 1e-3, "{a:?} → {b:?}: {p:?} misses by {miss} m");
            }
            Err(e) => {
                proptest::prop_assert!(
                    matches!(e, Error::InvalidValue { field: "points", .. }),
                    "{e:?}"
                );
                proptest::prop_assert_eq!(distance.meters(), 0.0);
            }
        }
    }
}

fn latitude() -> impl proptest::strategy::Strategy<Value = f64> {
    use proptest::prelude::*;
    prop_oneof![-90.0..=90.0_f64, Just(90.0), Just(-90.0), Just(0.0)]
}

fn longitude() -> impl proptest::strategy::Strategy<Value = f64> {
    use proptest::prelude::*;
    prop_oneof![-180.0..=180.0_f64, Just(180.0), Just(-180.0), Just(0.0)]
}

#[test]
fn direct_meridional_from_a_pole_follows_the_module_convention() {
    // RhumbSolve D 90 30 180 1000000 and D -90 30 0 1000000 give the latitude
    // (and a NaN longitude): 180° from the north pole runs down the start's own
    // meridian, and 0° down the far one.
    assert_direct(c(90.0, 30.0), 180.0, 1e6, 81.046_232_815_950_6, 30.0);
    assert_direct(c(90.0, 30.0), 0.0, 1e6, 81.046_232_815_950_6, -150.0);
    assert_direct(c(-90.0, 30.0), 0.0, 1e6, -81.046_232_815_950_6, 30.0);
    assert_direct(c(-90.0, 30.0), 180.0, 1e6, -81.046_232_815_950_6, -150.0);
}

#[test]
fn direct_meridional_passes_over_the_poles() {
    // RhumbSolve D 80 0 0 1668000 gives the reflected latitude (and a NaN
    // longitude); the line carries on down the far meridian.
    assert_direct(
        c(80.0, 0.0),
        0.0,
        1_668_000.0,
        85.065_195_975_351_97,
        -180.0,
    );
    assert_direct(
        c(-80.0, 0.0),
        180.0,
        1_668_000.0,
        -85.065_195_975_351_97,
        -180.0,
    );
    // Adding the half-turn (not subtracting it) pins the rounding: −179.9 + 180
    // is 0.099_999_999_999_994_32, while −179.9 − 180 wraps to
    // 0.100_000_000_000_022_74.
    let p = rhumb_destination(&c(80.0, -179.9), 0.0, m(1_668_000.0)).unwrap();
    assert_eq!(p.lon(), 0.099_999_999_999_994_32);
    // Exactly onto the pole there is no crossing, so the meridian is kept.
    let p = rhumb_destination(&c(80.0, 7.0), 0.0, m(1_116_825.857_375_85)).unwrap();
    assert_close(p.lat(), 90.0, DEGREES);
    assert_eq!(p.lon(), 7.0);
}

#[test]
fn direct_meridional_wraps_any_number_of_turns() {
    // The quarter meridian is 10 001 965.729 312 723 m (RhumbSolve I 90 0 0 90);
    // RhumbSolve D 0 10 180 5000982.8646563615 reaches 45.144 317 705 887 94°S.
    let quarter = 10_001_965.729_312_723;
    let eighth_lat = 45.144_317_705_887_94;
    // South past the south pole (3/8 of a turn): up the far meridian.
    assert_direct(c(0.0, 10.0), 180.0, 1.5 * quarter, -eighth_lat, -170.0);
    // Half a turn north lands on the equator of the far meridian.
    assert_direct(c(0.0, 10.0), 0.0, 2.0 * quarter, 0.0, -170.0);
    // A full turn crosses both poles and comes home…
    assert_direct(c(0.0, 10.0), 0.0, 4.0 * quarter, 0.0, 10.0);
    // …and 1⅛ turns lands where 1/8 of a turn does.
    assert_direct(c(0.0, 10.0), 0.0, 4.5 * quarter, eighth_lat, 10.0);
    assert_direct(c(0.0, 10.0), 180.0, 4.5 * quarter, -eighth_lat, 10.0);
}

#[test]
fn travel_north_counts_the_poles_it_passes() {
    use core::f64::consts::FRAC_PI_4;
    let case = |mu: f64, lat: f64, far: bool| {
        let (got, got_far) = travel_north(mu);
        assert_close(got, lat, 1e-15);
        assert_eq!(got_far, far, "{mu}");
    };
    // Short of the north pole, and from the south pole itself: none passed.
    case(FRAC_PI_4, FRAC_PI_4, false);
    case(-FRAC_PI_4, -FRAC_PI_4, false);
    case(-FRAC_PI_2, -FRAC_PI_2, false);
    // Exactly onto the north pole: arrived, not passed.
    case(FRAC_PI_2, FRAC_PI_2, false);
    // Past it, heading south down the far meridian.
    case(3.0 * FRAC_PI_4, FRAC_PI_4, true);
    // Exactly onto the south pole after passing the north one: still far.
    case(3.0 * FRAC_PI_2, -FRAC_PI_2, true);
    // Past the south pole too, heading north on the starting meridian.
    case(2.0 * PI + FRAC_PI_4, FRAC_PI_4, false);
    // Exactly back onto the north pole after a full turn: two poles passed.
    case(2.0 * PI + FRAC_PI_2, FRAC_PI_2, false);
    // A third pole: far again.
    case(3.0 * PI + FRAC_PI_4, -FRAC_PI_4, true);
}

#[test]
fn meridian_scale_is_continuous_at_the_east_west_limit() {
    let s = KrugerSeries::WGS84;
    let psi = s.isometric_latitude(f64::to_radians(40.0));
    let limit = meridian_scale(&s, psi, psi);
    // The radius of the 40° parallel, ν·cos φ = a·cos φ / √(1 − e² sin² φ).
    let phi = f64::to_radians(40.0);
    let e2 = Ellipsoid::WGS84.eccentricity_sq();
    let parallel =
        Ellipsoid::WGS84.semi_major().meters() * phi.cos() / (1.0 - e2 * phi.sin().powi(2)).sqrt();
    assert_close(limit, parallel, 1e-6);
    // Off the limit the scale is the mean over the interval, so it moves by
    // about q′/q · Δψ / 2 (≈ 0.32 Δψ here) — never by more than Δψ, however
    // small Δψ gets (a ratio of subtracted arcs would be noise below ~1e-8).
    for d_psi in [1e-15, -1e-12, 1e-10, 1e-8] {
        let relative = meridian_scale(&s, psi, psi + d_psi) / limit - 1.0;
        assert!(relative.abs() <= d_psi.abs(), "{d_psi:e}: {relative:e}");
        assert!(relative * d_psi <= 0.0, "{d_psi:e}: {relative:e}");
    }
    // Symmetric in its endpoints.
    let psi2 = s.isometric_latitude(f64::to_radians(60.0));
    assert_eq!(
        meridian_scale(&s, psi, psi2).to_bits(),
        meridian_scale(&s, psi2, psi).to_bits()
    );
}

#[test]
fn shortest_d_lon_is_signed_and_antimeridian_safe() {
    assert_close(
        shortest_d_lon(&c(0.0, 0.0), &c(0.0, 10.0)),
        10.0_f64.to_radians(),
        1e-12,
    );
    assert_close(
        shortest_d_lon(&c(0.0, 170.0), &c(0.0, -170.0)),
        20.0_f64.to_radians(),
        1e-12,
    );
    assert_close(
        shortest_d_lon(&c(0.0, -170.0), &c(0.0, 170.0)),
        -20.0_f64.to_radians(),
        1e-12,
    );
    // Exactly a half-turn is left as +π (the `> PI` guard is strict).
    assert_close(shortest_d_lon(&c(0.0, 0.0), &c(0.0, 180.0)), PI, 1e-12);
}
