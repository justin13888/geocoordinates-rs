//! Per-component validation of the local-frame offsets.

use super::*;
use crate::test_support::assert_close;

fn field_of<T: std::fmt::Debug>(result: Result<T>) -> &'static str {
    match result {
        Err(Error::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

/// Each ENU and NED component is checked on its own, and the error names the
/// component that failed.
#[test]
fn each_non_finite_component_is_rejected_by_name() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(field_of(Enu::new(bad, 0.0, 0.0)), "east");
        assert_eq!(field_of(Enu::new(0.0, bad, 0.0)), "north");
        assert_eq!(field_of(Enu::new(0.0, 0.0, bad)), "up");
        assert_eq!(field_of(Ned::new(bad, 0.0, 0.0)), "north");
        assert_eq!(field_of(Ned::new(0.0, bad, 0.0)), "east");
        assert_eq!(field_of(Ned::new(0.0, 0.0, bad)), "down");
        assert_eq!(field_of(Aer::new(bad, 0.0, Length::ZERO)), "azimuth");
        assert_eq!(field_of(Aer::new(0.0, bad, Length::ZERO)), "elevation");
        assert_eq!(
            field_of(Aer::new(0.0, 0.0, Length::from_meters(bad))),
            "range"
        );
    }
}

/// Finite components whose length overflows have no AER range, so they are
/// rejected; the largest offsets whose length is finite are kept.
#[test]
fn an_offset_whose_length_overflows_is_rejected() {
    let big = f64::MAX;
    assert_eq!(field_of(Enu::new(big, big, 0.0)), "magnitude");
    assert_eq!(field_of(Ned::new(0.0, big, big)), "magnitude");
    let edge = Enu::new(big, 0.0, 0.0).unwrap();
    assert_eq!(edge.to_aer().range().meters(), big);
}

#[test]
fn aer_rejects_out_of_range_elevation_and_negative_range() {
    for elevation in [90.000_000_001, -90.000_000_001, 180.0] {
        assert_eq!(
            field_of(Aer::new(0.0, elevation, Length::ZERO)),
            "elevation"
        );
    }
    assert_eq!(
        field_of(Aer::new(0.0, 0.0, Length::from_meters(-1e-300))),
        "range"
    );
    // The bounds themselves are valid, and a negative zero range becomes +0.
    for elevation in [90.0, -90.0] {
        assert_eq!(
            Aer::new(0.0, elevation, Length::ZERO)
                .unwrap()
                .elevation_deg(),
            elevation
        );
    }
    let zero = Aer::new(0.0, 0.0, Length::from_meters(-0.0)).unwrap();
    assert!(zero.range().meters().is_sign_positive());
}

#[test]
fn aer_normalizes_azimuth() {
    for (given, stored) in [(-90.0, 270.0), (360.0, 0.0), (725.0, 5.0), (-0.0, 0.0)] {
        let aer = Aer::new(given, 0.0, Length::from_meters(1.0)).unwrap();
        assert_eq!(aer.azimuth_deg(), stored, "{given}");
        assert!(aer.azimuth_deg().is_sign_positive());
    }
}

/// A range at `f64::MAX` still converts to a valid ENU offset in every
/// direction: where rounding carries the components' magnitude past
/// `f64::MAX`, they are pulled in by round-off instead.
#[test]
fn aer_at_the_largest_range_converts_to_a_valid_enu() {
    let mut pulled_in = 0;
    for i in 0..720 {
        for j in 0..=180 {
            let (azimuth, elevation) = (f64::from(i) * 0.5, f64::from(j) - 90.0);
            let aer = Aer::new(azimuth, elevation, Length::from_meters(f64::MAX)).unwrap();
            let enu = aer.to_enu();
            let rebuilt = Enu::new(enu.east_m(), enu.north_m(), enu.up_m()).unwrap();
            assert_eq!(rebuilt, enu, "{aer:?}");
            assert_close(enu.to_aer().range().meters() / f64::MAX, 1.0, 1e-12);
            let (sin_az, cos_az) = azimuth.to_radians().sin_cos();
            let (sin_el, cos_el) = elevation.to_radians().sin_cos();
            let raw = [
                f64::MAX * cos_el * sin_az,
                f64::MAX * cos_el * cos_az,
                f64::MAX * sin_el,
            ];
            let got = [enu.east_m(), enu.north_m(), enu.up_m()];
            if got != raw {
                pulled_in += 1;
                // Every non-zero component is pulled in, not just some.
                for (g, r) in got.iter().zip(raw) {
                    assert!(*g == 0.0 || g.abs() < r.abs(), "{aer:?}");
                }
            }
        }
    }
    // The sweep reaches the pulled-in branch, e.g. at (0.5°, -42°).
    assert!(pulled_in > 0);
    // An ordinary range is never pulled in: the components are the exact
    // products.
    let east = Aer::new(90.0, 0.0, Length::from_meters(100.0))
        .unwrap()
        .to_enu();
    assert_eq!(east.east_m(), 100.0);
}
