//! Pins the `From<gc::Error> for GeoError` mapping for every core error variant.
//!
//! Today only `CrsMismatch` and `OutOfRange` keep their own FFI variant; the
//! other seven collapse into `GeoError::Other`, carrying the core error's
//! display message. These tests record that collapse so any change to it is a
//! deliberate, visible edit.

use geocoordinates as gc;
use geocoordinates_ffi::GeoError;

/// Map a core error through the FFI conversion.
fn map(e: gc::Error) -> GeoError {
    GeoError::from(e)
}

/// Assert the mapping produced `Other` with exactly `expected` as its detail.
fn assert_other(e: gc::Error, expected: &str) {
    match map(e) {
        GeoError::Other { detail } => assert_eq!(detail, expected),
        other => panic!("expected GeoError::Other, got {other:?}"),
    }
}

#[test]
fn crs_mismatch_keeps_its_variant_with_canonical_names() {
    let mapped = map(gc::Error::CrsMismatch {
        expected: gc::Crs::Bd09,
        found: gc::Crs::Wgs84,
    });
    match mapped {
        GeoError::CrsMismatch { expected, found } => {
            assert_eq!(expected, "BD-09");
            assert_eq!(found, "WGS84");
        }
        other => panic!("expected GeoError::CrsMismatch, got {other:?}"),
    }
}

#[test]
fn crs_mismatch_names_every_crs_by_its_display_form() {
    let cases = [
        (gc::Crs::Wgs84, "WGS84"),
        (gc::Crs::Gcj02, "GCJ-02"),
        (gc::Crs::Bd09, "BD-09"),
        (gc::Crs::Nad27, "NAD27"),
        (gc::Crs::Tokyo, "Tokyo"),
        (gc::Crs::Pulkovo42, "Pulkovo-1942"),
    ];
    for (crs, name) in cases {
        match map(gc::Error::CrsMismatch {
            expected: crs,
            found: gc::Crs::Gcj02,
        }) {
            GeoError::CrsMismatch { expected, found } => {
                assert_eq!(expected, name);
                assert_eq!(found, "GCJ-02");
            }
            other => panic!("expected GeoError::CrsMismatch, got {other:?}"),
        }
    }
}

#[test]
fn out_of_range_keeps_its_variant_and_values() {
    match map(gc::Error::OutOfRange {
        lat: 91.5,
        lon: -181.25,
    }) {
        GeoError::OutOfRange { lat, lon } => {
            assert_eq!(lat, 91.5);
            assert_eq!(lon, -181.25);
        }
        other => panic!("expected GeoError::OutOfRange, got {other:?}"),
    }
}

#[test]
fn invalid_value_collapses_into_other() {
    assert_other(
        gc::Error::InvalidValue {
            field: "precision",
            detail: "must be 1..=9".to_string(),
        },
        "invalid value for `precision`: must be 1..=9",
    );
}

#[test]
fn invalid_cell_id_collapses_into_other() {
    assert_other(
        gc::Error::InvalidCellId {
            grid: "H3",
            value: 42,
        },
        "invalid H3 cell id: 42",
    );
}

#[test]
fn parse_collapses_into_other() {
    assert_other(
        gc::Error::Parse("no coordinate found".to_string()),
        "could not parse coordinate: no coordinate found",
    );
}

#[test]
fn unsupported_conversion_collapses_into_other() {
    assert_other(
        gc::Error::UnsupportedConversion {
            from: gc::Crs::Tokyo,
            to: gc::Crs::Pulkovo42,
        },
        "unsupported conversion: Tokyo -> Pulkovo-1942",
    );
}

#[test]
fn invalid_grid_ref_collapses_into_other() {
    assert_other(
        gc::Error::InvalidGridRef("ZZ".to_string()),
        "invalid grid reference: ZZ",
    );
}

#[test]
fn geoid_data_unavailable_collapses_into_other() {
    assert_other(
        gc::Error::GeoidDataUnavailable,
        "geoid data unavailable for the requested model or location",
    );
}

#[test]
fn feature_disabled_collapses_into_other() {
    assert_other(
        gc::Error::FeatureDisabled("h3"),
        "feature `h3` is not enabled",
    );
}

#[test]
fn geo_error_display_matches_the_core_message() {
    let crs = map(gc::Error::CrsMismatch {
        expected: gc::Crs::Bd09,
        found: gc::Crs::Wgs84,
    });
    assert_eq!(crs.to_string(), "crs mismatch: expected BD-09, found WGS84");
    let range = map(gc::Error::OutOfRange {
        lat: 91.0,
        lon: 0.0,
    });
    assert_eq!(
        range.to_string(),
        "coordinate out of valid range: lat=91, lon=0"
    );
    let other = map(gc::Error::Parse("x".to_string()));
    assert_eq!(other.to_string(), "could not parse coordinate: x");
}
