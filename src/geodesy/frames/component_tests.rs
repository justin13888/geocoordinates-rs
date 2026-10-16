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

/// A range at `f64::MAX` still converts to a valid ENU offset: the
/// components are pulled in by round-off rather than producing one whose
/// length overflows.
#[test]
fn aer_at_the_largest_range_converts_to_a_valid_enu() {
    for (azimuth, elevation) in [(45.0, 45.0), (0.0, 0.0), (0.0, 90.0), (123.0, -35.0)] {
        let aer = Aer::new(azimuth, elevation, Length::from_meters(f64::MAX)).unwrap();
        let enu = aer.to_enu();
        let rebuilt = Enu::new(enu.east_m(), enu.north_m(), enu.up_m()).unwrap();
        assert_eq!(rebuilt, enu);
        let back = enu.to_aer();
        assert_close(back.range().meters() / f64::MAX, 1.0, 1e-12);
        let ned = aer.to_ned();
        assert!(Ned::new(ned.north_m(), ned.east_m(), ned.down_m()).is_ok());
    }
}
