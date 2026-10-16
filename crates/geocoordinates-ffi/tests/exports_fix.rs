//! Calls the `Fix`-family constructors and validators (`Confidence`,
//! `Accuracy`, `RawSource`) and pins that a hand-built `Fix` crossing into
//! Rust is validated whole.

use geocoordinates_ffi::*;

/// Whether `result` is `InvalidValue` naming `field`.
fn invalid<T: std::fmt::Debug>(result: Result<T, GeoError>, field: &str) -> bool {
    matches!(result, Err(GeoError::InvalidValue { field: f, .. }) if f == field)
}

#[test]
fn confidence_new_and_validate_reject_nan_and_out_of_range() {
    assert_eq!(confidence_new(0.25).unwrap().value, 0.25);
    assert_eq!(confidence_new(1.0).unwrap().value, 1.0);
    for bad in [2.0, -0.5, f64::NAN, f64::INFINITY] {
        assert!(invalid(confidence_new(bad), "confidence"), "{bad}");
        assert!(
            invalid(confidence_validate(Confidence { value: bad }), "confidence"),
            "{bad}"
        );
    }
    assert!(confidence_validate(Confidence { value: 0.0 }).is_ok());
}

#[test]
fn accuracy_new_and_validate_check_magnitudes_and_keep_the_kind() {
    let a = accuracy_new(AccuracyKind::Cep50, Some(3.0), Some(5.0)).unwrap();
    assert_eq!(
        a,
        Accuracy {
            kind: AccuracyKind::Cep50,
            horizontal_m: Some(3.0),
            vertical_m: Some(5.0),
        }
    );
    assert!(accuracy_validate(a).is_ok());
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        let horizontal = accuracy_new(AccuracyKind::Bound, Some(bad), None);
        assert!(invalid(horizontal, "horizontal"), "{bad}");
        let vertical = Accuracy {
            kind: AccuracyKind::OneSigma,
            horizontal_m: None,
            vertical_m: Some(bad),
        };
        assert!(invalid(accuracy_validate(vertical), "vertical"), "{bad}");
    }
    let empty = accuracy_new(AccuracyKind::Unspecified, None, None);
    assert!(invalid(empty, "accuracy"));
}

#[test]
fn raw_source_new_validates_its_confidence() {
    let source = raw_source_new("51.5, -0.12".to_string(), confidence_new(0.9).unwrap()).unwrap();
    assert_eq!(
        source,
        RawSource {
            raw: "51.5, -0.12".to_string(),
            confidence: Confidence { value: 0.9 },
            axis_order: None,
            datum_ambiguity: None,
            geoid_separation_m: None,
            notes: Vec::new(),
        }
    );
    let bad = raw_source_new("x".to_string(), Confidence { value: 1.5 });
    assert!(invalid(bad, "confidence"));
}

/// A hand-built `Fix` crossing in is validated whole: its accuracy and its
/// source's confidence, not only its coordinate.
#[test]
fn a_fix_crossing_in_rejects_invalid_accuracy_and_confidence() {
    let coord = coordinate_new(40.7128, -74.006, Crs::Wgs84).unwrap();
    let mut negative = fix_from_coord(coord).unwrap();
    negative.accuracy = Some(Accuracy {
        kind: AccuracyKind::OneSigma,
        horizontal_m: Some(-1.0),
        vertical_m: None,
    });
    assert!(invalid(
        format_fix(negative, format_options_default()),
        "horizontal"
    ));
    let mut unsure = fix_from_coord(coord).unwrap();
    unsure.source = Some(RawSource {
        raw: "40.7128, -74.006".to_string(),
        confidence: Confidence { value: f64::NAN },
        axis_order: None,
        datum_ambiguity: None,
        geoid_separation_m: Some(-34.2),
        notes: Vec::new(),
    });
    assert!(invalid(
        format_fix(unsure, format_options_default()),
        "confidence"
    ));
}
