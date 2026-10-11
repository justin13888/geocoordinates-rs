use super::*;
use crate::test_support::assert_close;

fn c(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon)
}

#[test]
fn rhumb_references() {
    // Along the equator the rhumb line is the equator (q = cos 0 = 1).
    assert_close(
        rhumb_distance(&c(0.0, 0.0), &c(0.0, 1.0)).unwrap().meters(),
        111_195.0,
        1.0,
    );
    assert_close(
        rhumb_bearing(&c(0.0, 0.0), &c(0.0, 10.0)).unwrap(),
        90.0,
        1e-9,
    );
    assert_close(
        rhumb_bearing(&c(0.0, 0.0), &c(10.0, 0.0)).unwrap(),
        0.0,
        1e-9,
    );
}

#[test]
fn rhumb_destination_round_trip() {
    let (start, end) = (c(40.0, -75.0), c(45.0, -70.0));
    let d = rhumb_destination(
        &start,
        rhumb_bearing(&start, &end).unwrap(),
        rhumb_distance(&start, &end).unwrap(),
    )
    .unwrap();
    assert_close(d.lat, 45.0, 1e-6);
    assert_close(d.lon, -70.0, 1e-6);
}

#[test]
fn shortest_d_lon_is_signed_and_antimeridian_safe() {
    // Plain eastward difference (well under a half-turn).
    assert_close(
        shortest_d_lon(&c(0.0, 0.0), &c(0.0, 10.0)),
        10.0_f64.to_radians(),
        1e-12,
    );
    // Across the antimeridian the short way is +20° east, not −340°.
    assert_close(
        shortest_d_lon(&c(0.0, 170.0), &c(0.0, -170.0)),
        20.0_f64.to_radians(),
        1e-12,
    );
    // …and signed the other way when reversed.
    assert_close(
        shortest_d_lon(&c(0.0, -170.0), &c(0.0, 170.0)),
        -20.0_f64.to_radians(),
        1e-12,
    );
    // Exactly a half-turn is left as +π (the `> PI` guard is strict).
    assert_close(shortest_d_lon(&c(0.0, 0.0), &c(0.0, 180.0)), PI, 1e-12);
}

#[test]
fn stretched_lat_diff_reference() {
    use core::f64::consts::{FRAC_PI_3, FRAC_PI_6};
    // ln(tan(3π/8)) from the equator to 45°.
    assert_close(
        stretched_lat_diff(0.0, FRAC_PI_4),
        0.881_373_587_019_542_9,
        1e-12,
    );
    // A non-zero lower latitude exercises both Mercator terms.
    assert_close(
        stretched_lat_diff(FRAC_PI_6, FRAC_PI_3),
        0.767_651_752_590_761_5,
        1e-12,
    );
    // Identical latitudes stretch to nothing.
    assert_close(stretched_lat_diff(FRAC_PI_6, FRAC_PI_6), 0.0, 1e-12);
}

#[test]
fn east_west_scale_falls_back_to_cos_at_and_below_the_threshold() {
    use core::f64::consts::FRAC_PI_3;
    // |Δψ| exactly at the 1e-12 threshold (either sign) is still east-west:
    // q = cos φ₁, not the ratio Δφ/Δψ (which would be 1 here).
    assert_eq!(east_west_scale(1e-12, 1e-12, FRAC_PI_3), FRAC_PI_3.cos());
    assert_eq!(east_west_scale(-1e-12, -1e-12, FRAC_PI_3), FRAC_PI_3.cos());
    // Above it, q is the ratio.
    assert_eq!(east_west_scale(1e-11, 2e-11, FRAC_PI_3), 0.5);
    assert_eq!(east_west_scale(1e-11, -2e-11, FRAC_PI_3), -0.5);
}

#[test]
fn rhumb_destination_due_east_holds_latitude() {
    // A due-east rhumb keeps latitude fixed (d_psi → 0, so q = cos φ₁).
    let d = rhumb_destination(&c(40.0, 0.0), 90.0, Length::from_meters(100_000.0)).unwrap();
    assert_close(d.lat, 40.0, 1e-9);
    assert_close(d.lon, 1.173_979_358_250_968, 1e-9);
}

#[test]
fn rhumb_destination_reflects_over_the_poles() {
    // Pushing north past the pole reflects the latitude back down the far side.
    let n = rhumb_destination(&c(80.0, 0.0), 0.0, Length::from_meters(1_668_000.0)).unwrap();
    assert_close(n.lat, 84.999_336_333_074_71, 1e-9);
    assert_close(n.lon, -180.0, 1e-9);
    // …and symmetrically over the south pole (the negative reflection arm).
    let s = rhumb_destination(&c(-80.0, 0.0), 180.0, Length::from_meters(1_668_000.0)).unwrap();
    assert_close(s.lat, -84.999_336_333_074_71, 1e-9);
    assert_close(s.lon, -180.0, 1e-9);
}

#[test]
fn rhumb_destination_over_a_pole_adds_a_half_turn_to_the_longitude() {
    // Due north past the pole lands on the far meridian, start.lon + 180°,
    // wrapped. Adding or subtracting the half-turn names the same meridian, so
    // this pins the exact rounding of the addition: −179.9 + 180 is
    // 0.099_999_999_999_994_32, while −179.9 − 180 wraps to 0.100_000_000_000_022_74.
    let p = rhumb_destination(&c(80.0, -179.9), 0.0, Length::from_meters(1_668_000.0)).unwrap();
    assert_close(p.lat, 84.999_336_333_074_71, 1e-9);
    assert_eq!(p.lon, 0.099_999_999_999_994_32);
}

#[test]
fn rhumb_across_the_antimeridian_takes_the_short_way() {
    let (a, b) = (c(10.0, 179.5), c(10.0, -179.5));
    assert_close(rhumb_bearing(&a, &b).unwrap(), 90.0, 1e-9);
    // One degree of longitude along the 10° parallel on the sphere.
    let expected = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians() * 10.0_f64.to_radians().cos();
    assert_close(rhumb_distance(&a, &b).unwrap().meters(), expected, 1e-6);
    let d = rhumb_destination(&a, 90.0, Length::from_meters(110_000.0)).unwrap();
    assert_close(d.lat, 10.0, 1e-12);
    assert_close(d.lon, -179.495_486_787, 1e-8);
}

#[test]
fn rhumb_cardinal_bearings_and_meridian_distance() {
    assert_close(
        rhumb_bearing(&c(10.0, 0.0), &c(0.0, 0.0)).unwrap(),
        180.0,
        1e-9,
    );
    assert_close(
        rhumb_bearing(&c(0.0, 10.0), &c(0.0, 0.0)).unwrap(),
        270.0,
        1e-9,
    );
    // Pole to pole along a meridian is half a great circle.
    assert_close(
        rhumb_distance(&c(-90.0, 0.0), &c(90.0, 0.0))
            .unwrap()
            .meters(),
        MEAN_EARTH_RADIUS_M * PI,
        1e-6,
    );
}

#[test]
fn rhumb_distance_branches_at_the_stretched_latitude_threshold() {
    // |Δψ| ≤ 1e-12: the east-west branch, q = cos φ₁ — one degree of the 40°
    // parallel even though the latitudes differ by 1e-13°.
    let parallel = MEAN_EARTH_RADIUS_M * 1.0_f64.to_radians() * 40.0_f64.to_radians().cos();
    assert_close(
        rhumb_distance(&c(40.0, 0.0), &c(40.0 + 1e-13, 1.0))
            .unwrap()
            .meters(),
        parallel,
        1e-6,
    );
    // |Δψ| > 1e-12: the ratio branch, q = Δφ/Δψ, pinned on a long oblique leg.
    assert_close(
        rhumb_distance(&c(40.0, -75.0), &c(51.5, 0.0))
            .unwrap()
            .meters(),
        5_928_370.877_680,
        1e-3,
    );
    assert_close(
        rhumb_bearing(&c(40.0, -75.0), &c(51.5, 0.0)).unwrap(),
        77.543_463_128,
        1e-8,
    );
}

#[test]
fn rhumb_destination_due_north_onto_the_pole_keeps_its_meridian() {
    // Ten degrees of arc due north from 80°N lands on the pole without
    // crossing it, so there is no reflection and no 180° longitude flip.
    let ten_deg = Length::from_meters(10.0_f64.to_radians() * MEAN_EARTH_RADIUS_M);
    let p = rhumb_destination(&c(80.0, 0.0), 0.0, ten_deg).unwrap();
    assert_close(p.lat, 90.0, 1e-9);
    assert_eq!(p.lon, 0.0);
}

#[test]
fn rhumb_destination_oblique_past_the_pole_reflects() {
    // KNOWN WRONG (C7, flipped by #115): a non-meridional rhumb line spirals
    // into the pole and never crosses it, but the spherical routine reflects
    // over it as if it were a meridian and returns an arbitrary position.
    let p = rhumb_destination(&c(80.0, 0.0), 45.0, Length::from_meters(2_000_000.0)).unwrap();
    assert_close(p.lat, 87.281_689_447, 1e-8);
    assert_close(p.lon, -105.233_022_646, 1e-8);
    // KNOWN WRONG (C7, flipped by #115): landing on the pole along 45° divides
    // by Δψ → ∞, so the longitude is round-off noise; only its range is pinned.
    let to_pole =
        Length::from_meters(10.0_f64.to_radians() * MEAN_EARTH_RADIUS_M / FRAC_PI_4.cos());
    let p = rhumb_destination(&c(80.0, 0.0), 45.0, to_pole).unwrap();
    assert_close(p.lat, 90.0, 1e-9);
    assert!(p.validate().is_ok(), "{p:?}");
}
