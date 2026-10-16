use super::*;
use crate::test_support::assert_close;

/// Spread of signed values × axes used for round-trip checks. Includes
/// `0.0`/`-0.0`, the poles/antimeridian, and a high-precision fraction.
const SAMPLES: &[f64] = &[
    0.0,
    -0.0,
    1.0,
    -1.0,
    40.712_775_3,
    -74.006_0,
    89.999_999,
    -89.999_999,
    90.0,
    -90.0,
    179.999_999,
    -179.999_999,
    180.0,
    -180.0,
    45.123_456_789,
    -123.456_789,
];

/// Every sample that is in range for `axis`.
fn samples(axis: Axis) -> impl Iterator<Item = f64> {
    SAMPLES
        .iter()
        .copied()
        .filter(move |v| v.abs() <= axis_max(axis))
}

const AXES: [Axis; 2] = [Axis::Latitude, Axis::Longitude];

#[test]
fn degrees_dms_round_trip_is_exact() {
    for axis in AXES {
        for v in samples(axis) {
            let back = Dms::from_degrees(v, axis).unwrap().to_degrees();
            assert_close(back, v, 1e-12);
        }
    }
}

#[test]
fn degrees_ddm_round_trip_is_exact() {
    for axis in AXES {
        for v in samples(axis) {
            let back = Ddm::from_degrees(v, axis).unwrap().to_degrees();
            assert_close(back, v, 1e-12);
        }
    }
}

#[test]
fn dms_ddm_round_trips_agree_with_degrees() {
    for axis in AXES {
        for v in samples(axis) {
            let dms = Dms::from_degrees(v, axis).unwrap();
            let ddm = Ddm::from_degrees(v, axis).unwrap();
            assert_close(dms.to_ddm().to_degrees(), v, 1e-12);
            assert_close(ddm.to_dms().to_degrees(), v, 1e-12);
            assert_eq!(dms.to_ddm().hemisphere(), ddm.hemisphere());
            assert_eq!(dms.to_ddm().degrees(), ddm.degrees());
        }
    }
}

#[test]
fn from_degrees_decomposes_components() {
    let dms = Dms::from_degrees(40.712_775_3, Axis::Latitude).unwrap();
    assert_eq!(dms.degrees(), 40);
    assert_eq!(dms.minutes(), 42);
    assert_close(dms.seconds(), 45.991_08, 1e-3);
    assert_eq!(dms.hemisphere(), Hemisphere::North);

    let ddm = Ddm::from_degrees(-74.006, Axis::Longitude).unwrap();
    assert_eq!(ddm.degrees(), 74);
    assert_close(ddm.minutes(), 0.36, 1e-9);
    assert_eq!(ddm.hemisphere(), Hemisphere::West);
}

#[test]
fn signed_degrees_follow_the_hemisphere() {
    let south = Dms::new(33, 51, 54.0, Hemisphere::South).unwrap();
    assert_close(
        south.to_degrees(),
        -(33.0 + 51.0 / 60.0 + 54.0 / 3600.0),
        1e-12,
    );
    let west = Ddm::new(74, 0.36, Hemisphere::West).unwrap();
    assert_close(west.to_degrees(), -74.006, 1e-12);
    let east = Ddm::new(151, 12.6, Hemisphere::East).unwrap();
    assert_close(east.to_degrees(), 151.21, 1e-12);
}

#[test]
fn negative_zero_is_positive_hemisphere() {
    assert_eq!(
        Dms::from_degrees(-0.0, Axis::Latitude)
            .unwrap()
            .hemisphere(),
        Hemisphere::North
    );
    assert_eq!(
        Ddm::from_degrees(-0.0, Axis::Longitude)
            .unwrap()
            .hemisphere(),
        Hemisphere::East
    );
    // A positive zero behaves identically.
    assert_eq!(
        Dms::from_degrees(0.0, Axis::Longitude)
            .unwrap()
            .hemisphere(),
        Hemisphere::East
    );
}

#[test]
fn axis_selects_hemisphere_letters() {
    let hemi = |v, axis| Dms::from_degrees(v, axis).unwrap().hemisphere();
    assert_eq!(hemi(10.0, Axis::Latitude), Hemisphere::North);
    assert_eq!(hemi(-10.0, Axis::Latitude), Hemisphere::South);
    assert_eq!(hemi(10.0, Axis::Longitude), Hemisphere::East);
    assert_eq!(hemi(-74.006, Axis::Longitude), Hemisphere::West);
}

#[test]
fn dms_ddm_inter_conversions_preserve_hemisphere() {
    let dms = Dms::new(40, 42, 46.0, Hemisphere::North).unwrap();
    let ddm = dms.to_ddm();
    assert_eq!(ddm.degrees(), 40);
    assert_close(ddm.minutes(), 42.0 + 46.0 / 60.0, 1e-12);
    assert_eq!(ddm.hemisphere(), Hemisphere::North);

    // Ddm -> Dms recovers whole minutes and seconds.
    let dms2 = ddm.to_dms();
    assert_eq!(dms2.degrees(), 40);
    assert_eq!(dms2.minutes(), 42);
    assert_close(dms2.seconds(), 46.0, 1e-9);
    assert_eq!(dms2.hemisphere(), Hemisphere::North);
}

#[test]
fn to_ddm_carries_a_minute_that_rounds_up_to_60() {
    // The largest f64 below 60″ makes 59′ + s/60 round to exactly 60′.
    let seconds = f64::from_bits(60.0f64.to_bits() - 1);
    let dms = Dms::new(89, 59, seconds, Hemisphere::South).unwrap();
    let ddm = dms.to_ddm();
    assert_eq!(ddm.degrees(), 90);
    assert_eq!(ddm.minutes().to_bits(), 0.0f64.to_bits());
    assert_eq!(ddm.hemisphere(), Hemisphere::South);
    // The carried value is still a valid DDM, and a valid latitude.
    assert_eq!(Ddm::new(90, 0.0, Hemisphere::South), Ok(ddm));
    assert_close(ddm.to_degrees(), -90.0, 0.0);

    // A value just short of the carry keeps its minutes.
    let ddm = Dms::new(10, 59, 30.0, Hemisphere::East).unwrap().to_ddm();
    assert_eq!(ddm.degrees(), 10);
    assert_close(ddm.minutes(), 59.5, 0.0);
}

#[test]
fn ddm_to_dms_stays_below_60_seconds() {
    // The largest f64 below 60′ splits into 59′ and a fraction that scales
    // exactly below 60″.
    let minutes = f64::from_bits(60.0f64.to_bits() - 1);
    let dms = Ddm::new(10, minutes, Hemisphere::North).unwrap().to_dms();
    assert_eq!(dms.minutes(), 59);
    assert!(dms.seconds() < 60.0, "{}", dms.seconds());
    assert_eq!(Dms::new(10, 59, dms.seconds(), Hemisphere::North), Ok(dms));
}

#[test]
fn hemisphere_sign() {
    assert_close(Hemisphere::North.sign(), 1.0, 0.0);
    assert_close(Hemisphere::East.sign(), 1.0, 0.0);
    assert_close(Hemisphere::South.sign(), -1.0, 0.0);
    assert_close(Hemisphere::West.sign(), -1.0, 0.0);
}

#[test]
fn wrap_longitude_is_half_open() {
    // The antimeridian normalizes to the western edge.
    assert_close(wrap_longitude(180.0), -180.0, 1e-12);
    assert_close(wrap_longitude(-180.0), -180.0, 1e-12);
    assert_close(wrap_longitude(540.0), -180.0, 1e-12);
    assert_close(wrap_longitude(190.0), -170.0, 1e-12);
    assert_close(wrap_longitude(-190.0), 170.0, 1e-12);
    assert_close(wrap_longitude(0.0), 0.0, 1e-12);
    assert_close(wrap_longitude(179.999), 179.999, 1e-12);
    // Already-in-range values are unchanged.
    assert_close(wrap_longitude(-73.5), -73.5, 1e-12);
}

#[test]
fn wrap_longitude_never_returns_negative_zero() {
    for lon in [-0.0, 0.0, -360.0, 360.0, -720.0, -1e-20] {
        let wrapped = wrap_longitude(lon);
        assert_eq!(wrapped, 0.0, "{lon}");
        assert!(wrapped.is_sign_positive(), "{lon} wrapped to -0.0");
    }
}

#[test]
fn clamp_latitude_includes_poles() {
    assert_close(clamp_latitude(90.0), 90.0, 1e-12);
    assert_close(clamp_latitude(-90.0), -90.0, 1e-12);
    assert_close(clamp_latitude(91.0), 90.0, 1e-12);
    assert_close(clamp_latitude(-90.5), -90.0, 1e-12);
    assert_close(clamp_latitude(45.0), 45.0, 1e-12);
}

#[test]
fn normalize_degrees_is_zero_to_360() {
    assert_close(normalize_degrees(0.0), 0.0, 1e-12);
    assert_close(normalize_degrees(-0.0), 0.0, 1e-12);
    assert_close(normalize_degrees(360.0), 0.0, 1e-12);
    assert_close(normalize_degrees(-1.0), 359.0, 1e-12);
    assert_close(normalize_degrees(720.5), 0.5, 1e-12);
    assert_close(normalize_degrees(45.0), 45.0, 1e-12);
    assert!(normalize_degrees(f64::NAN).is_nan());
}

#[test]
fn normalize_degrees_never_returns_360_or_negative_zero() {
    // `rem_euclid` rounds a tiny negative up to the excluded bound 360.
    assert_eq!(normalize_degrees(-1e-20).to_bits(), 0.0f64.to_bits());
    assert_eq!(normalize_degrees(-0.0).to_bits(), 0.0f64.to_bits());
    assert_eq!(normalize_degrees(-360.0).to_bits(), 0.0f64.to_bits());
    // The largest value below 360 is kept, not folded to 0.
    let below = f64::from_bits(360.0f64.to_bits() - 1);
    assert_eq!(normalize_degrees(below), below);
}

#[cfg(feature = "serde")]
#[test]
fn deserialize_rejects_out_of_range_components() {
    let dms = r#"{"degrees":40,"minutes":60,"seconds":0.0,"hemisphere":"North"}"#;
    assert!(serde_json::from_str::<Dms>(dms).is_err());
    let ddm = r#"{"degrees":40,"minutes":60.0,"hemisphere":"North"}"#;
    assert!(serde_json::from_str::<Ddm>(ddm).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_with_an_unchanged_shape() {
    let dms = Dms::new(40, 42, 46.5, Hemisphere::North).unwrap();
    let json = serde_json::to_string(&dms).unwrap();
    assert_eq!(
        json,
        r#"{"degrees":40,"minutes":42,"seconds":46.5,"hemisphere":"North"}"#
    );
    assert_eq!(serde_json::from_str::<Dms>(&json).unwrap(), dms);

    let ddm = Ddm::new(74, 0.36, Hemisphere::West).unwrap();
    let json = serde_json::to_string(&ddm).unwrap();
    assert_eq!(json, r#"{"degrees":74,"minutes":0.36,"hemisphere":"West"}"#);
    assert_eq!(serde_json::from_str::<Ddm>(&json).unwrap(), ddm);
}

/// The `InvalidValue` field name an `Err` carries.
fn field_of<T: std::fmt::Debug>(r: Result<T>) -> &'static str {
    match r {
        Err(Error::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn invalid_degrees_are_rejected() {
    assert_eq!(
        field_of(Dms::from_degrees(f64::NAN, Axis::Latitude)),
        "decimal degrees"
    );
    assert_eq!(
        field_of(Ddm::from_degrees(f64::INFINITY, Axis::Longitude)),
        "decimal degrees"
    );
    assert_eq!(
        field_of(Ddm::from_degrees(90.000_001, Axis::Latitude)),
        "decimal degrees"
    );
    assert_eq!(
        field_of(Dms::from_degrees(-180.000_001, Axis::Longitude)),
        "decimal degrees"
    );
    // The axis maximum itself is accepted on both axes.
    assert!(Dms::from_degrees(-90.0, Axis::Latitude).is_ok());
    assert!(Ddm::from_degrees(180.0, Axis::Longitude).is_ok());
    // A latitude-range value is still a valid longitude.
    assert!(Dms::from_degrees(120.0, Axis::Longitude).is_ok());
}

#[test]
fn invalid_components_are_rejected() {
    assert_eq!(
        field_of(Dms::new(40, 60, 0.0, Hemisphere::North)),
        "minutes"
    );
    assert_eq!(
        field_of(Dms::new(40, 0, 60.0, Hemisphere::North)),
        "seconds"
    );
    assert_eq!(
        field_of(Dms::new(40, 0, -0.1, Hemisphere::North)),
        "seconds"
    );
    assert_eq!(
        field_of(Dms::new(40, 0, f64::NAN, Hemisphere::North)),
        "seconds"
    );
    assert_eq!(field_of(Dms::new(91, 0, 0.0, Hemisphere::South)), "degrees");
    assert_eq!(field_of(Dms::new(90, 0, 0.5, Hemisphere::North)), "degrees");
    assert_eq!(field_of(Dms::new(90, 1, 0.0, Hemisphere::North)), "degrees");
    assert_eq!(field_of(Ddm::new(180, 0.1, Hemisphere::East)), "degrees");
    assert_eq!(field_of(Ddm::new(181, 0.0, Hemisphere::West)), "degrees");
    assert_eq!(field_of(Ddm::new(40, 60.0, Hemisphere::North)), "minutes");
    assert_eq!(field_of(Ddm::new(40, -0.5, Hemisphere::North)), "minutes");
    assert_eq!(
        field_of(Ddm::new(40, f64::INFINITY, Hemisphere::North)),
        "minutes"
    );
    // The hemisphere selects the axis: 120° is a longitude, not a latitude.
    assert_eq!(field_of(Ddm::new(120, 0.0, Hemisphere::North)), "degrees");
    assert!(Ddm::new(120, 0.0, Hemisphere::East).is_ok());
    // The axis maxima themselves are valid.
    assert!(Dms::new(90, 0, 0.0, Hemisphere::South).is_ok());
    assert!(Ddm::new(180, 0.0, Hemisphere::West).is_ok());
}
