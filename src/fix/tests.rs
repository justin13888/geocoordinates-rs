use super::{Accuracy, AccuracyKind, Confidence};
use crate::{Error, Length};

#[test]
fn confidence_rejects_nan_and_out_of_range() {
    for bad in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY, -0.01, 1.01, 2.0] {
        assert!(
            matches!(
                Confidence::new(bad),
                Err(Error::InvalidValue {
                    field: "confidence",
                    ..
                })
            ),
            "{bad}"
        );
        assert!(Confidence::try_from(bad).is_err(), "{bad}");
    }
    assert_eq!(Confidence::new(0.0).unwrap().value(), 0.0);
    assert_eq!(Confidence::new(1.0).unwrap(), Confidence::CERTAIN);
    assert_eq!(f64::from(Confidence::try_from(0.25).unwrap()), 0.25);
}

#[test]
fn accuracy_rejects_invalid_magnitudes() {
    let m = |v| Some(Length::from_meters(v));
    for bad in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            matches!(
                Accuracy::new(AccuracyKind::Bound, m(bad), None),
                Err(Error::InvalidValue {
                    field: "horizontal",
                    ..
                })
            ),
            "{bad}"
        );
        assert!(
            matches!(
                Accuracy::new(AccuracyKind::Bound, m(1.0), m(bad)),
                Err(Error::InvalidValue {
                    field: "vertical",
                    ..
                })
            ),
            "{bad}"
        );
    }
    assert!(matches!(
        Accuracy::new(AccuracyKind::Cep50, None, None),
        Err(Error::InvalidValue {
            field: "accuracy",
            ..
        })
    ));
    let zero = Accuracy::new(AccuracyKind::Unspecified, Some(Length::ZERO), None).unwrap();
    assert_eq!(zero.horizontal(), Some(Length::ZERO));
    let vertical_only = Accuracy::new(AccuracyKind::OneSigma, None, m(2.0)).unwrap();
    assert_eq!(
        (
            vertical_only.kind(),
            vertical_only.horizontal(),
            vertical_only.vertical()
        ),
        (AccuracyKind::OneSigma, None, m(2.0))
    );
}

/// Deserialization validates like `new`, and the wire shape is a bare number.
#[cfg(feature = "serde")]
#[test]
fn confidence_serde_validates_and_keeps_its_shape() {
    for bad in ["1.5", "-0.5"] {
        assert!(serde_json::from_str::<Confidence>(bad).is_err(), "{bad}");
    }
    let c: Confidence = serde_json::from_str("0.5").unwrap();
    assert_eq!(c.value(), 0.5);
    assert_eq!(serde_json::to_string(&c).unwrap(), "0.5");
}

/// Deserialization validates like `new`, and the kind travels with the
/// magnitudes.
#[cfg(feature = "serde")]
#[test]
fn accuracy_serde_validates_and_round_trips() {
    let a = Accuracy::new(AccuracyKind::OneSigma, Some(Length::from_meters(4.5)), None).unwrap();
    let json = serde_json::to_string(&a).unwrap();
    assert_eq!(
        json,
        r#"{"kind":"OneSigma","horizontal_m":4.5,"vertical_m":null}"#
    );
    assert_eq!(serde_json::from_str::<Accuracy>(&json).unwrap(), a);
    for bad in [
        r#"{"kind":"Bound","horizontal_m":-1.0,"vertical_m":null}"#,
        r#"{"kind":"Bound","horizontal_m":null,"vertical_m":-0.5}"#,
        r#"{"kind":"Bound","horizontal_m":null,"vertical_m":null}"#,
        r#"{"kind":"Sometimes","horizontal_m":1.0,"vertical_m":null}"#,
        // The pre-0.15 wrapped shape no longer reads.
        r#"{"kind":"Bound","horizontal":{"meters":1.0},"vertical":null}"#,
    ] {
        assert!(serde_json::from_str::<Accuracy>(bad).is_err(), "{bad}");
    }
}

/// A `Fix` with every optional field set, and its `RawSource` likewise,
/// read back equal to what was written.
#[cfg(feature = "serde")]
#[test]
fn fix_serde_round_trips() {
    use super::{AxisOrder, DatumAmbiguity, Fix, RawSource};
    use crate::{Coordinate, Height};
    use std::time::{Duration, SystemTime};

    let coord = Coordinate::gcj02(39.9, 116.4).unwrap();
    let mut source = RawSource::new("39.9, 116.4", Confidence::new(0.8).unwrap());
    source.axis_order = Some(AxisOrder::LonLat);
    source.datum_ambiguity = Some(DatumAmbiguity::PossiblyGcj02);
    source.geoid_separation = Some(Length::from_meters(-28.5));
    source.notes = vec!["first".into(), "second".into()];
    assert_eq!(
        serde_json::to_value(&source).unwrap(),
        serde_json::json!({
            "raw": "39.9, 116.4", "confidence": 0.8, "axis_order": "LonLat",
            "datum_ambiguity": "PossiblyGcj02", "geoid_separation_m": -28.5,
            "notes": ["first", "second"],
        })
    );
    let json = serde_json::to_string(&source).unwrap();
    assert_eq!(serde_json::from_str::<RawSource>(&json).unwrap(), source);

    let mut fix = Fix::from_coord(coord.with_height(Height::Orthometric(43.5)).unwrap());
    let m = Some(Length::from_meters(3.0));
    fix.accuracy = Some(Accuracy::new(AccuracyKind::Cep50, m, m).unwrap());
    fix.timestamp = Some(SystemTime::UNIX_EPOCH + Duration::new(1_700_000_000, 250_000_000));
    fix.source = Some(source);
    let json = serde_json::to_string(&fix).unwrap();
    assert_eq!(serde_json::from_str::<Fix>(&json).unwrap(), fix);
}
