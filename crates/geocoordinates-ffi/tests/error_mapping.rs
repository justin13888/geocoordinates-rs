//! Pins the `From<gc::Error> for GeoError` mapping for every core error variant.
//!
//! Every core variant maps one-to-one onto its own typed FFI variant, with the
//! same fields and the same display message, so binding users never have to
//! string-match `GeoError::Other`. These tests record that mapping so any change
//! to it is a deliberate, visible edit.

use geocoordinates as gc;
use geocoordinates_ffi::{Crs, GeoError};

/// Map a core error through the FFI conversion.
fn map(e: gc::Error) -> GeoError {
    GeoError::from(e)
}

/// Assert `e` maps to `expected` and keeps the core error's display message.
fn assert_maps(e: gc::Error, expected: GeoError) {
    let message = e.to_string();
    let mapped = map(e);
    assert_eq!(mapped, expected);
    assert_eq!(mapped.to_string(), message);
}

#[test]
fn out_of_range_keeps_its_values() {
    assert_maps(
        gc::Error::OutOfRange {
            lat: 91.5,
            lon: -181.25,
        },
        GeoError::OutOfRange {
            lat: 91.5,
            lon: -181.25,
        },
    );
}

#[test]
fn outside_domain_keeps_its_operation_and_point() {
    assert_maps(
        gc::Error::OutsideDomain {
            operation: "UTM",
            lat: 85.0,
            lon: 12.5,
        },
        GeoError::OutsideDomain {
            operation: "UTM".to_string(),
            lat: 85.0,
            lon: 12.5,
        },
    );
}

#[test]
fn invalid_value_keeps_its_field_and_detail() {
    assert_maps(
        gc::Error::InvalidValue {
            field: "precision",
            detail: "must be 1..=9".to_string(),
        },
        GeoError::InvalidValue {
            field: "precision".to_string(),
            detail: "must be 1..=9".to_string(),
        },
    );
}

#[test]
fn a_crs_argument_with_no_ellipsoid_maps_to_invalid_value_naming_the_argument() {
    // The core rejects GCJ-02 / BD-09 passed as a `Crs` argument with
    // `InvalidValue` naming that argument, not with an invented `CrsMismatch`.
    let identity = gc::geodesy::Helmert::IDENTITY;
    let ecef = gc::geodesy::Ecef::new(6_378_137.0, 0.0, 0.0);
    for (core, field) in [
        (ecef.try_to_coordinate(gc::Crs::Gcj02).unwrap_err(), "crs"),
        (
            gc::geodesy::DatumTransform::new(gc::Crs::Bd09, gc::Crs::Wgs84, identity, None)
                .unwrap_err(),
            "from_crs",
        ),
        (
            gc::geodesy::DatumTransform::new(gc::Crs::Nad27, gc::Crs::Gcj02, identity, None)
                .unwrap_err(),
            "to_crs",
        ),
    ] {
        let detail = match &core {
            gc::Error::InvalidValue { field: f, detail } if *f == field => detail.clone(),
            other => panic!("expected InvalidValue for `{field}`, got {other:?}"),
        };
        assert_maps(
            core,
            GeoError::InvalidValue {
                field: field.to_string(),
                detail,
            },
        );
    }
}

#[test]
fn invalid_cell_id_keeps_its_grid_and_value() {
    assert_maps(
        gc::Error::InvalidCellId {
            grid: "H3",
            value: 42,
        },
        GeoError::InvalidCellId {
            grid: "H3".to_string(),
            value: 42,
        },
    );
}

#[test]
fn parse_keeps_its_detail() {
    assert_maps(
        gc::Error::Parse("no coordinate found".to_string()),
        GeoError::Parse {
            detail: "no coordinate found".to_string(),
        },
    );
}

#[test]
fn invalid_grid_ref_keeps_its_detail() {
    assert_maps(
        gc::Error::InvalidGridRef("ZZ".to_string()),
        GeoError::InvalidGridRef {
            detail: "ZZ".to_string(),
        },
    );
}

#[test]
fn crs_mismatch_carries_typed_reference_systems() {
    assert_maps(
        gc::Error::CrsMismatch {
            expected: gc::Crs::Bd09,
            found: gc::Crs::Wgs84,
        },
        GeoError::CrsMismatch {
            expected: Crs::Bd09,
            found: Crs::Wgs84,
        },
    );
}

#[test]
fn crs_mismatch_maps_every_crs_and_names_it_by_its_display_form() {
    let cases = [
        (gc::Crs::Wgs84, Crs::Wgs84, "WGS84"),
        (gc::Crs::Gcj02, Crs::Gcj02, "GCJ-02"),
        (gc::Crs::Bd09, Crs::Bd09, "BD-09"),
        (gc::Crs::Nad27, Crs::Nad27, "NAD27"),
        (gc::Crs::Tokyo, Crs::Tokyo, "Tokyo"),
        (gc::Crs::Pulkovo42, Crs::Pulkovo42, "Pulkovo-1942"),
    ];
    for (core, ffi, name) in cases {
        let mapped = map(gc::Error::CrsMismatch {
            expected: core,
            found: gc::Crs::Gcj02,
        });
        assert_eq!(
            mapped,
            GeoError::CrsMismatch {
                expected: ffi,
                found: Crs::Gcj02,
            }
        );
        assert_eq!(
            mapped.to_string(),
            format!("crs mismatch: expected {name}, found GCJ-02")
        );
    }
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
    let parse = map(gc::Error::Parse("x".to_string()));
    assert_eq!(parse.to_string(), "could not parse coordinate: x");
    let other = GeoError::Other {
        detail: "anything".to_string(),
    };
    assert_eq!(other.to_string(), "anything");
}
