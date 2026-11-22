//! Guards the FFI surface against drift, by reading the crate's own source:
//!
//! - every export is called by a Rust test in this directory (a method on a
//!   receiver of its own type, `export_coverage/receiver.rs`), and every
//!   fallible export by a test that also checks an error;
//! - every fallible export documents its failures under a non-empty `# Errors`;
//! - every export follows the naming scheme in `README.md#naming`;
//! - the export counts equal the pinned ones, so an export cannot disappear
//!   unnoticed;
//! - every non-trait function and constant in the core crate's public API
//!   snapshot (`public-api/geocoordinates/`) has an FFI form, save the
//!   shrink-only lists in `export_coverage/parity_gaps.rs`.
//!
//! The scan is textual but comment- and string-aware: [`scan::strip`] blanks
//! every comment, string literal and char literal before any code is matched,
//! so text that merely mentions an export counts for nothing.

#[path = "export_coverage/scan.rs"]
mod scan;

#[path = "export_coverage/parity.rs"]
mod parity;

#[path = "export_coverage/parity_gaps.rs"]
mod parity_gaps;

#[path = "export_coverage/receiver.rs"]
mod receiver;

#[path = "export_coverage/self_tests.rs"]
mod self_tests;

use parity::*;
use scan::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The number of exports: free functions, object constructors and methods, and
/// foreign-trait methods. Update it in the diff that adds or removes one.
const EXPORTS: usize = 246;
/// The number of those exports whose return type resolves to a `Result`.
const FALLIBLE_EXPORTS: usize = 197;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("readable directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn read_dir_sources(dir: &str) -> Vec<(PathBuf, String)> {
    let mut paths = Vec::new();
    rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join(dir), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).unwrap();
            (p, text)
        })
        .collect()
}

fn crate_sources() -> Vec<String> {
    read_dir_sources("src")
        .into_iter()
        .map(|(_, s)| s)
        .collect()
}

fn crate_exports() -> Vec<Export> {
    crate_sources().iter().flat_map(|s| exports(s)).collect()
}

/// The stripped text of every test file here but this one and its scanner.
fn test_code() -> Vec<String> {
    let this_file = Path::new(file!()).file_name().unwrap();
    let scanner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/export_coverage");
    read_dir_sources("tests")
        .into_iter()
        .filter(|(p, _)| p.file_name() != Some(this_file) && !p.starts_with(&scanner))
        .map(|(_, s)| strip(&s))
        .collect()
}

fn fallible(exports: &[Export]) -> Vec<&Export> {
    let aliases = result_aliases(&crate_sources());
    exports
        .iter()
        .filter(|e| is_result(&e.ret, &aliases))
        .collect()
}

#[test]
fn export_counts_match_the_pinned_counts() {
    let exports = crate_exports();
    let labels: BTreeSet<String> = exports.iter().map(Export::label).collect();
    assert_eq!(labels.len(), exports.len(), "an export is scanned twice");
    assert_eq!(
        exports.len(),
        EXPORTS,
        "update `EXPORTS` with the export set"
    );
    assert_eq!(
        fallible(&exports).len(),
        FALLIBLE_EXPORTS,
        "update `FALLIBLE_EXPORTS` with the export set"
    );
}

#[test]
fn every_fallible_export_documents_its_errors() {
    let exports = crate_exports();
    let missing: Vec<String> = fallible(&exports)
        .into_iter()
        .filter(|e| !documents_errors(&e.doc))
        .map(Export::label)
        .collect();
    assert!(
        missing.is_empty(),
        "fallible exports with no non-empty `# Errors` section: {missing:?}"
    );
}

#[test]
fn every_export_is_called_by_a_rust_test() {
    let tests = test_code();
    let exports = crate_exports();
    let types = receiver::types(&tests, &exports);
    let missing: Vec<String> = exports
        .iter()
        .filter(|e| !tests.iter().any(|t| calls(t, e, &types)))
        .map(Export::label)
        .collect();
    assert!(missing.is_empty(), "exports with no Rust test: {missing:?}");
}

/// Fallible exports whose error path no valid FFI input reaches, so no test can
/// check it.
const UNREACHABLE_ERRORS: &[&str] = &[];

#[test]
fn every_fallible_export_has_an_err_test() {
    let tests = test_code();
    let bodies: Vec<(String, &str)> = tests.iter().flat_map(|t| fn_bodies(t)).collect();
    let helpers = error_helpers(&bodies);
    let exports = crate_exports();
    let types = receiver::types(&tests, &exports);
    let missing: Vec<String> = fallible(&exports)
        .into_iter()
        .filter(|e| !UNREACHABLE_ERRORS.contains(&e.label().as_str()))
        .filter(|e| {
            !bodies
                .iter()
                .any(|(_, b)| calls(b, e, &types) && checks_error(b, &helpers))
        })
        .map(Export::label)
        .collect();
    assert!(
        missing.is_empty(),
        "fallible exports with no error test: {missing:?}"
    );
    for name in UNREACHABLE_ERRORS {
        assert!(
            exports.iter().any(|e| e.label() == *name),
            "`UNREACHABLE_ERRORS` names no export: {name}"
        );
    }
}

/// Exports that return a bare `f64` that is neither a length nor an angle, or
/// whose unit the scheme fixes elsewhere (`README.md#naming`).
const BARE_F64_EXCEPTIONS: &[&str] = &[
    // The `Length` converters: meters is always the other side.
    "length_from_unit",
    "length_to_unit",
    // A ratio, meters per unit.
    "length_unit_meters_per_unit",
    // A sign, `-1.0` or `+1.0`.
    "hemisphere_sign",
    // Names degrees as its target, as the scheme allows.
    "normalize_degrees",
    "dms_to_degrees",
    "ddm_to_degrees",
    // Dimensionless ellipsoid shape parameters, not lengths.
    "ellipsoid_flattening",
    "ellipsoid_eccentricity_sq",
];

/// The only exports suffixed `_meters`; every other length is `_m`.
const METERS_SUFFIXED: &[&str] = &["height_meters", "mgrs_precision_meters"];

/// The unit suffixes an `f64` parameter may end in.
const PARAM_UNITS: &[&str] = &["_m", "_deg", "_px"];

/// `export(param)`: the `f64` parameters with no unit suffix, each
/// dimensionless or with its unit fixed elsewhere (`README.md#naming`). This
/// list may only shrink.
const F64_PARAM_EXCEPTIONS: &[&str] = &[
    // A constructor's parameters take the names of the record fields they fill.
    "coordinate_new(lat)",
    "coordinate_new(lon)",
    "coordinate_from_lon_lat(lat)",
    "coordinate_from_lon_lat(lon)",
    // The function names degrees as its input.
    "dms_from_degrees(value)",
    "ddm_from_degrees(value)",
    "normalize_degrees(deg)",
    // The `Length` converters: the unit argument names the other side.
    "length_from_unit(value)",
    "length_to_unit(meters)",
    // Dimensionless: a confidence in `[0, 1]`, a fraction of a path, and a
    // fractional zoom level.
    "confidence_new(value)",
    "intermediate(fraction)",
    "world_pixel_from_coordinate(zoom)",
    "world_pixel_to_coordinate(zoom)",
    "fit_bounds(max_zoom)",
    "clusters_in_view(zoom)",
    "clusters_in_view_geojson(zoom)",
];

/// Records whose `TryFrom` only validates the records they embed, each of which
/// has its own `_validate`: they hold no invariant of their own.
const COMPOSITE_RECORDS: &[&str] = &["Fix", "RawSource", "ZoneAction"];

/// The unit-suffix rules of `README.md#naming` over `exports`: the violations,
/// and the labels of the unsuffixed `f64` parameters `F64_PARAM_EXCEPTIONS`
/// excused. A function apart from the gate so `self_tests.rs` can show each
/// rejection firing on a fixture.
fn unit_suffix_violations(
    exports: &[Export],
    aliases: &BTreeSet<String>,
) -> (Vec<String>, BTreeSet<String>) {
    let mut wrong = Vec::new();

    // A bare length or angle names its unit, also inside `Result` / `Option`.
    for e in exports
        .iter()
        .filter(|e| value_type(&e.ret, aliases) == "f64")
    {
        let suffixed = ["_m", "_deg"].iter().any(|s| e.name.ends_with(s))
            || METERS_SUFFIXED.contains(&e.name.as_str());
        if !suffixed && !BARE_F64_EXCEPTIONS.contains(&e.name.as_str()) {
            wrong.push(format!(
                "{}: returns a bare f64 with no unit suffix",
                e.label()
            ));
        }
    }
    // `_meters` is the suffix of exactly the exports in `METERS_SUFFIXED`.
    for e in exports {
        if e.name.ends_with("_meters") && !METERS_SUFFIXED.contains(&e.name.as_str()) {
            wrong.push(format!(
                "{}: `_meters` where the scheme uses `_m`",
                e.label()
            ));
        }
    }
    // A bare `f64` parameter names its unit too.
    let mut unsuffixed = BTreeSet::new();
    for e in exports {
        for param in e.params.split(',').filter(|p| !p.trim().is_empty()) {
            let Some((name, ty)) = param.split_once(':') else {
                continue;
            };
            let name = name.trim();
            if value_type(ty.trim(), aliases) != "f64" {
                continue;
            }
            let label = format!("{}({name})", e.label());
            if name.ends_with("_meters") {
                wrong.push(format!("{label}: `_meters` where the scheme uses `_m`"));
            } else if !PARAM_UNITS.iter().any(|s| name.ends_with(s)) {
                if F64_PARAM_EXCEPTIONS.contains(&label.as_str()) {
                    unsuffixed.insert(label);
                } else {
                    wrong.push(format!("{label}: a bare f64 parameter with no unit suffix"));
                }
            }
        }
    }
    (wrong, unsuffixed)
}

#[test]
fn exports_follow_naming_scheme() {
    let sources = crate_sources();
    let exports = crate_exports();
    let names: BTreeSet<&str> = exports.iter().map(|e| e.name.as_str()).collect();

    let aliases = result_aliases(&sources);
    let (mut wrong, unsuffixed) = unit_suffix_violations(&exports, &aliases);
    for name in BARE_F64_EXCEPTIONS.iter().chain(METERS_SUFFIXED) {
        assert!(
            names.contains(name),
            "`BARE_F64_EXCEPTIONS` / `METERS_SUFFIXED` names no export: {name}"
        );
    }
    for label in F64_PARAM_EXCEPTIONS {
        assert!(
            unsuffixed.contains(*label),
            "`F64_PARAM_EXCEPTIONS` names no unsuffixed f64 parameter: {label}"
        );
    }

    // A reader of text into a `Fix` or `CaptureGeo` is `parse_*`.
    let reads_text = |e: &Export| e.params.contains("String") || e.params.contains("&str");
    let into_fix = |e: &Export| {
        e.ret
            .split(|c: char| !is_ident(c))
            .any(|t| t == "Fix" || t == "CaptureGeo")
    };
    let mut parsers = 0;
    for e in exports.iter().filter(|e| reads_text(e) && into_fix(e)) {
        parsers += 1;
        if !e.name.starts_with("parse_") {
            wrong.push(format!(
                "{}: reads text into a fix but is not `parse_*`",
                e.label()
            ));
        }
    }
    assert!(parsers >= 10, "found only {parsers} text readers");

    // Every invariant record an export takes as input has `<record>_validate`.
    let records = validated_records(&sources);
    assert!(
        records.len() >= 29,
        "found only {} validated records",
        records.len()
    );
    for record in &records {
        let validate = format!("{}_validate", snake_case(record));
        if !names.contains(validate.as_str()) && !COMPOSITE_RECORDS.contains(&record.as_str()) {
            wrong.push(format!("{record}: has a `TryFrom` but no `{validate}`"));
        }
    }
    for record in COMPOSITE_RECORDS {
        assert!(
            records.contains(*record),
            "`COMPOSITE_RECORDS` names no record: {record}"
        );
    }

    // Every grid code - a `String` decoded to a location - has `<grid>_canonicalize`.
    let mut grids = BTreeSet::new();
    for e in &exports {
        let one_string =
            e.params.trim_end_matches(',').split(',').count() == 1 && e.params.ends_with("String");
        let locates = e.ret.contains("ApproxCoordinate") || e.ret.contains("Tile");
        if !(one_string && locates && e.kind == Kind::Function) {
            continue;
        }
        let grid = e
            .name
            .strip_suffix("_decode")
            .or_else(|| e.name.strip_suffix("_to_coordinate"))
            .or_else(|| e.name.split_once("_from_").map(|(_, g)| g));
        match grid {
            Some(grid) => {
                grids.insert(grid.to_string());
            }
            None => wrong.push(format!(
                "{}: decodes a grid code it does not name",
                e.label()
            )),
        }
    }
    let expected = ["geohash", "maidenhead", "mgrs", "plus_code", "quadkey"];
    assert!(
        expected.iter().all(|g| grids.contains(*g)),
        "grid codes found: {grids:?}"
    );
    for grid in &grids {
        if !names.contains(format!("{grid}_canonicalize").as_str()) {
            wrong.push(format!("{grid}: a grid code with no `{grid}_canonicalize`"));
        }
    }

    assert!(
        wrong.is_empty(),
        "exports off the naming scheme:\n{}",
        wrong.join("\n")
    );
}

/// Every non-trait `fn` and `const` in the core crate's all-features public
/// API snapshot.
fn core_items() -> BTreeSet<ApiItem> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../public-api/geocoordinates");
    let mut items = BTreeSet::new();
    for entry in fs::read_dir(dir).expect("the public-api snapshots") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_some_and(|ext| ext == "txt") {
            items.extend(api_items(&fs::read_to_string(path).unwrap()));
        }
    }
    items
}

#[test]
fn every_core_item_has_an_ffi_form() {
    let items = core_items();
    assert!(items.len() >= 350, "read only {} core items", items.len());
    let exports = crate_exports();
    let fields = record_fields(&crate_sources());
    let unreached: BTreeSet<String> = items
        .iter()
        .filter(|item| !reaches(item, &exports, &fields))
        .map(ApiItem::label)
        .collect();
    let names: BTreeSet<&str> = exports.iter().map(|e| e.name.as_str()).collect();
    for (item, form) in parity_gaps::RENAMED {
        let exists = match form.split_once('.') {
            Some((record, field)) => fields.get(record).is_some_and(|f| f.contains(field)),
            None => names.contains(form),
        };
        assert!(
            exists,
            "`RENAMED` maps {item} to {form}, which is no export or record field"
        );
    }
    let allowed: BTreeSet<String> = parity_gaps::ALLOWED
        .iter()
        .chain(parity_gaps::RENAMED.iter().map(|(item, _)| item))
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        allowed.len(),
        parity_gaps::ALLOWED.len() + parity_gaps::RENAMED.len(),
        "an item is listed twice in `parity_gaps`"
    );
    let missing: Vec<&String> = unreached.difference(&allowed).collect();
    assert!(
        missing.is_empty(),
        "core items with no FFI form (add one, or allow-list a deliberate exclusion):\n{}",
        missing
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let stale: Vec<&String> = allowed.difference(&unreached).collect();
    assert!(
        stale.is_empty(),
        "`parity_gaps` entries an FFI form now reaches by name, or that name no core item; remove them: {stale:?}"
    );
}
