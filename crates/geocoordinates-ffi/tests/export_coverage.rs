//! Guards the FFI surface against drift, by reading the crate's own source:
//!
//! - every export is called by a Rust test in this directory, and every
//!   fallible export by a test that also checks an error;
//! - every fallible export documents its failures under a non-empty `# Errors`;
//! - every export follows the naming scheme in `README.md#naming`;
//! - the export counts equal the pinned ones, so an export cannot disappear
//!   unnoticed.
//!
//! The scan is textual but comment- and string-aware: [`scan::strip`] blanks
//! every comment, string literal and char literal before any code is matched,
//! so text that merely mentions an export counts for nothing.

#[path = "export_coverage/scan.rs"]
mod scan;

use scan::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The number of exports: free functions, object constructors and methods, and
/// foreign-trait methods. Update it in the diff that adds or removes one.
const EXPORTS: usize = 242;
/// The number of those exports whose return type resolves to a `Result`.
const FALLIBLE_EXPORTS: usize = 195;

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
fn scanner_reads_every_export_form() {
    let fixture = r##"
        // #[uniffi::export]
        // pub fn in_a_comment() {}
        const S: &str = "#[uniffi::export] pub fn in_a_string() {}";
        type Fallible<T> = Result<T, GeoError>;
        pub type AlsoFallible = Fallible<u8>;

        /// Plain.
        #[uniffi::export]
        pub fn plain(a: f64) -> f64 { a }

        /// With arguments, a blank line and doc lines after the attribute.
        #[uniffi::export(name = "renamed")]

        /// # Errors
        /// Always.
        #[allow(clippy::needless_pass_by_value)]
        pub fn with_args(
            s: String,
        ) -> Fallible<u8> where u8: Copy {
            let _ = '{';
            Err(GeoError::Other { detail: s })
        }

        /// # Errors
        #[uniffi::export]
        pub fn empty_errors() -> AlsoFallible { Ok(0) }

        #[uniffi::export]
        impl Thing {
            /// # Errors
            /// Never in practice.
            #[uniffi::constructor]
            pub fn new() -> Result<Arc<Self>, GeoError> { todo!("}") }
            pub fn len(&self) -> u64 { 0 }
            fn private(&self) {}
        }

        #[uniffi::export(with_foreign)]
        pub trait Foreign: Send + Sync {
            fn call(&self, x: u8) -> std::result::Result<u8, GeoError>;
        }
    "##;
    let found = exports(fixture);
    let labels: Vec<String> = found.iter().map(Export::label).collect();
    assert_eq!(
        labels,
        [
            "plain",
            "with_args",
            "empty_errors",
            "Thing::new",
            "Thing::len",
            "Foreign::call"
        ]
    );
    let kinds: Vec<Kind> = found.iter().map(|e| e.kind).collect();
    use Kind::*;
    assert_eq!(
        kinds,
        [Function, Function, Function, Constructor, Method, Method]
    );
    assert_eq!(found[0].ret, "f64");
    assert_eq!(found[1].params, "s: String,");
    assert_eq!(found[1].ret, "Fallible<u8>");

    let aliases = result_aliases(&[fixture.to_string()]);
    assert_eq!(
        aliases,
        BTreeSet::from(["AlsoFallible".into(), "Fallible".into()])
    );
    let fallible: Vec<String> = found
        .iter()
        .filter(|e| is_result(&e.ret, &aliases))
        .map(Export::label)
        .collect();
    assert_eq!(
        fallible,
        ["with_args", "empty_errors", "Thing::new", "Foreign::call"]
    );
    let documented: Vec<bool> = found.iter().map(|e| documents_errors(&e.doc)).collect();
    assert_eq!(documented, [false, true, false, true, false, false]);
}

#[test]
fn call_scan_ignores_comments_strings_and_definitions() {
    let export = |name: &str, owner: Option<&str>, kind| Export {
        name: name.into(),
        owner: owner.map(Into::into),
        kind,
        doc: Vec::new(),
        params: String::new(),
        ret: String::new(),
    };
    let f = export("geohash_encode", None, Kind::Function);
    let reads = |text: &str| calls(&strip(text), &f);
    assert!(!reads("// geohash_encode(c, 5)"));
    assert!(!reads("/* geohash_encode(c, 5) */"));
    assert!(!reads(r#"let s = "geohash_encode(c, 5)";"#));
    assert!(!reads(r##"let s = r#"geohash_encode(c, 5)"#;"##));
    assert!(!reads("fn geohash_encode(c: Coordinate) {}"));
    assert!(!reads("my_geohash_encode(c, 5);"));
    assert!(reads("let _ = geohash_encode(c, 5);"));
    assert!(reads("let _ = ffi::geohash_encode(c, 5);"));

    let ctor = export("new", Some("Track"), Kind::Constructor);
    assert!(calls(&strip("Track::new(vec![])"), &ctor));
    assert!(!calls(&strip("Vec::new()"), &ctor));
    let method = export("locate", Some("Track"), Kind::Method);
    assert!(calls(&strip("track.locate(t, o)"), &method));
    assert!(!calls(&strip("fn locate(&self) {}"), &method));

    let code = strip("fn a() { if x { b() } } fn b() -> u8 { 1 }");
    let bodies = fn_bodies(&code);
    let names: Vec<&str> = bodies.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(bodies[0].1, "{ if x { b() } }");
    let none = BTreeSet::new();
    assert!(checks_error("{ assert!(f().is_err()); }", &none));
    assert!(checks_error(
        "{ assert!(matches!(f(), Err(GeoError::Parse { .. }))); }",
        &none
    ));
    assert!(checks_error(
        "{ match f() { Err(GeoError::Parse { .. }) => {} _ => panic!() } }",
        &none
    ));
    assert!(checks_error("{ if let Err(e) = f() { drop(e) } }", &none));
    assert!(checks_error(
        "{ let Err(e) = f() else { panic!(\"no error\") }; }",
        &none
    ));
    // A success-only test whose `Err` arm or branch only fails the test.
    assert!(!checks_error(
        "{ match f() { Ok(v) => assert_eq!(v, 1), Err(e) => panic!(\"{e}\") } }",
        &none
    ));
    assert!(!checks_error(
        "{ match f() { Ok(_) => {} Err(e) => { unreachable!(\"{e}\") } } }",
        &none
    ));
    assert!(!checks_error(
        "{ match f() { Ok(_) => {} Err(e) if e.is_x() => todo!() } }",
        &none
    ));
    assert!(!checks_error(
        "{ if let Err(e) = f() { panic!(\"{e}\") } }",
        &none
    ));
    assert!(!checks_error("{ match f() { MyErr(e) => {} } }", &none));
    assert!(!checks_error(
        "{ assert!(matches!(f(), Ok(_))); let _ = Err(1); }",
        &none
    ));
    let code = strip("fn err_of(r: R) -> E { r.unwrap_err() } fn t() { err_of(f()); }");
    let bodies = fn_bodies(&code);
    let helpers = error_helpers(&bodies);
    assert_eq!(helpers, BTreeSet::from(["err_of".into(), "t".into()]));
    assert!(!checks_error("{ my_err_of(f()); }", &helpers));
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
    let missing: Vec<String> = crate_exports()
        .iter()
        .filter(|e| !tests.iter().any(|t| calls(t, e)))
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
    let missing: Vec<String> = fallible(&exports)
        .into_iter()
        .filter(|e| !UNREACHABLE_ERRORS.contains(&e.label().as_str()))
        .filter(|e| {
            !bodies
                .iter()
                .any(|(_, b)| calls(b, e) && checks_error(b, &helpers))
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

/// Records whose `TryFrom` only validates the records they embed, each of which
/// has its own `_validate`: they hold no invariant of their own.
const COMPOSITE_RECORDS: &[&str] = &["Fix", "RawSource", "ZoneAction"];

#[test]
fn exports_follow_naming_scheme() {
    let sources = crate_sources();
    let exports = crate_exports();
    let names: BTreeSet<&str> = exports.iter().map(|e| e.name.as_str()).collect();
    let mut wrong = Vec::new();

    // A bare length or angle names its unit, also inside `Result` / `Option`.
    let aliases = result_aliases(&sources);
    for e in exports
        .iter()
        .filter(|e| value_type(&e.ret, &aliases) == "f64")
    {
        let suffixed = ["_m", "_meters", "_deg"]
            .iter()
            .any(|s| e.name.ends_with(s));
        if !suffixed && !BARE_F64_EXCEPTIONS.contains(&e.name.as_str()) {
            wrong.push(format!(
                "{}: returns a bare f64 with no unit suffix",
                e.label()
            ));
        }
    }
    for name in BARE_F64_EXCEPTIONS {
        assert!(
            names.contains(name),
            "`BARE_F64_EXCEPTIONS` names no export: {name}"
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

#[test]
fn naming_scheme_records_are_read() {
    let fixture = r"
        macro_rules! mirror { ($($t:ident),*) => {$(
            impl TryFrom<$t> for gc::china::$t { }
        )*}; }
        mirror!(Wgs84, Gcj02);
        impl TryFrom<GeoBounds> for gc::index::GeoBounds { }
        impl TryFrom<u8> for Local { }
    ";
    let records = validated_records(&[fixture.to_string()]);
    assert_eq!(
        records,
        BTreeSet::from(["Gcj02".into(), "GeoBounds".into(), "Wgs84".into()])
    );
    assert_eq!(snake_case("BaiduMercator"), "baidu_mercator");

    // The unit rule reads the value inside `Result` / `Option` and aliases.
    let aliases = BTreeSet::from(["GeoResult".to_string()]);
    assert_eq!(value_type("Result<f64, GeoError>", &aliases), "f64");
    assert_eq!(value_type("std::result::Result<f64, E>", &aliases), "f64");
    assert_eq!(value_type("Option<f64>", &aliases), "f64");
    assert_eq!(value_type("GeoResult<Option<f64>>", &aliases), "f64");
    assert_eq!(value_type("f64", &aliases), "f64");
    assert_eq!(value_type("Vec<f64>", &aliases), "Vec<f64>");
    assert_eq!(
        value_type("Result<Vec<f64>, GeoError>", &aliases),
        "Vec<f64>"
    );
}
