//! Tests of the scanner `export_coverage.rs` runs: each reads a fixture, so a
//! scanner regression fails here rather than silently passing the gate.

use super::parity::*;
use super::receiver::{self, Types};
use super::scan::*;
use std::collections::BTreeSet;

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
        [
            Function,
            Function,
            Function,
            Constructor,
            Method,
            ForeignMethod
        ]
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
    let none = Types::default();
    let reads = |text: &str| calls(&strip(text), &f, &none);
    assert!(!reads("// geohash_encode(c, 5)"));
    assert!(!reads("/* geohash_encode(c, 5) */"));
    assert!(!reads(r#"let s = "geohash_encode(c, 5)";"#));
    assert!(!reads(r##"let s = r#"geohash_encode(c, 5)"#;"##));
    assert!(!reads("fn geohash_encode(c: Coordinate) {}"));
    assert!(!reads("my_geohash_encode(c, 5);"));
    assert!(reads("let _ = geohash_encode(c, 5);"));
    assert!(reads("let _ = ffi::geohash_encode(c, 5);"));

    let ctor = export("new", Some("Track"), Kind::Constructor);
    assert!(calls(&strip("Track::new(vec![])"), &ctor, &none));
    assert!(!calls(&strip("Vec::new()"), &ctor, &none));

    // A method counts only on a receiver of its owner's type.
    let helpers = strip("fn track() -> Arc<Track> { todo!() } fn list() -> Vec<u8> { vec![] }");
    let helper_types = receiver::types(&[helpers], &[]);
    let len = export("len", Some("Track"), Kind::Method);
    let reads = |text: &str| calls(&strip(text), &len, &helper_types);
    assert!(reads("fn t(track: &Track) { track.len(); }"));
    assert!(reads("fn t() { let t = Track::new(v).unwrap(); t.len(); }"));
    assert!(reads(
        "fn t() { let t: Arc<Track> = make(); let u = t.clone(); u.len(); }"
    ));
    assert!(reads("fn t() { track().len(); }"));
    assert!(reads("fn t() { Track::new(v).expect(\"ok\").len(); }"));
    assert!(reads("fn t() { (Track::new(v)?).len(); }"));
    assert!(reads("fn t(x: X) { Track::len(&x); }"));
    assert!(!reads("fn t() { let v = vec![1]; v.len(); }"));
    assert!(!reads("fn t() { list().len(); }"));
    assert!(!reads("fn t(track: &Track) { track.fixes().len(); }"));
    assert!(!reads("fn t(track: &Track) { self.track.len(); }"));
    assert!(!reads(
        "fn t(track: &Track) {} fn u(track: Vec<u8>) { track.len(); }"
    ));
    assert!(!reads("fn len(&self) {}"));
    let call = export("call", Some("Foreign"), Kind::ForeignMethod);
    let impls = strip("struct Mine; impl Foreign for Mine { fn call(&self) {} }");
    let impl_types = receiver::types(&[impls], &[]);
    assert!(calls(
        &strip("fn t() { run(Arc::new(Mine)); }"),
        &call,
        &impl_types
    ));
    assert!(!calls(
        &strip("fn t() { run(Arc::new(Yours)); }"),
        &call,
        &impl_types
    ));
    // Defining the implementor, or naming it outside a call's arguments, is
    // not handing one over.
    for defines in [
        "struct Mine; impl Foreign for Mine { fn call(&self) {} }",
        "impl Foreign for Mine { fn call(&self) -> u8 { helper(Mine) } }",
        "fn t(m: Mine) -> Mine { m }",
        "fn t() { let _ = Mine; }",
    ] {
        assert!(!calls(&strip(defines), &call, &impl_types), "{defines}");
    }

    let code = strip("fn a() { if x { b() } } fn b() -> u8 { 1 }");
    let bodies = fn_bodies(&code);
    let names: Vec<&str> = bodies.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(bodies[0].1, "fn a() { if x { b() } }");
    let none = BTreeSet::new();
    // An error check counts only where it pins the variant.
    assert!(!checks_error("{ assert!(f().is_err()); }", &none));
    assert!(!checks_error("{ let _ = f().unwrap_err(); }", &none));
    assert!(!checks_error("{ assert!(matches!(f(), Err(_))); }", &none));
    assert!(!checks_error("{ if let Err(e) = f() { drop(e) } }", &none));
    assert!(!checks_error(
        "{ let _ = GeoError::Other; assert!(f().is_err()); }",
        &none
    ));
    assert!(!checks_error(
        "{ let _ = GeoError::other(); f().unwrap_err(); }",
        &none
    ));
    assert!(checks_error(
        "{ assert!(matches!(f().unwrap_err(), GeoError::Parse { .. })); }",
        &none
    ));
    // The expected variant may be built before the error is taken.
    assert!(checks_error(
        "{ let want = GeoError::Other { detail }; assert_eq!(f().unwrap_err(), want); }",
        &none
    ));
    assert!(checks_error(
        "{ assert!(matches!(f(), Err(GeoError::Parse { .. }))); }",
        &none
    ));
    assert!(checks_error(
        "{ match f() { Err(GeoError::Parse { .. }) => {} _ => panic!() } }",
        &none
    ));
    assert!(checks_error(
        "{ if let Err(GeoError::Parse { kind, .. }) = f() { drop(kind) } }",
        &none
    ));
    assert!(checks_error(
        "{ let Err(e) = f() else { panic!(\"no error\") }; \
         assert!(matches!(e, GeoError::Parse { .. })); }",
        &none
    ));
    // A success-only test whose `Err` arm or branch only fails the test.
    assert!(!checks_error(
        "{ match f() { Ok(v) => assert_eq!(v, 1), Err(e) => panic!(\"{e}\") } let _ = GeoError::Other; }",
        &none
    ));
    assert!(!checks_error(
        "{ match f() { Ok(_) => {} Err(e) => { unreachable!(\"{e}\") } } let _ = GeoError::Other; }",
        &none
    ));
    assert!(!checks_error(
        "{ match f() { Ok(_) => {} Err(e) if e.is_x() => todo!() } let _ = GeoError::Other; }",
        &none
    ));
    assert!(!checks_error(
        "{ if let Err(e) = f() { panic!(\"{e}\") } let _ = GeoError::Other; }",
        &none
    ));
    assert!(!checks_error("{ match f() { MyErr(e) => {} } }", &none));
    assert!(!checks_error(
        "{ assert!(matches!(f(), Ok(_))); let _ = Err(1); }",
        &none
    ));
    let code = strip(
        "fn field_of(r: R) -> String { match r { Err(GeoError::InvalidValue { field, .. }) \
         => field, _ => panic!() } } fn t() { field_of(f()); } \
         fn bare(r: R) -> E { r.unwrap_err() } fn u() { bare(f()); }",
    );
    let bodies = fn_bodies(&code);
    let helpers = error_helpers(&bodies);
    assert_eq!(helpers, BTreeSet::from(["field_of".into(), "t".into()]));
    assert!(!checks_error("{ my_err_of(f()); }", &helpers));
}

#[test]
fn naming_scheme_records_are_read() {
    let fixture = r"
        macro_rules! mirror { ($($t:ident),*) => {$(
            impl TryFrom<$t> for gc::china::$t { }
        )*}; }
        mirror!(Wgs84, Gcj02);
        impl TryFrom<GeoBounds> for gc::geodesy::GeoBounds { }
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

#[test]
fn unit_suffix_rules_reject_each_violation() {
    let fixture = r"
        type GeoResult<T> = Result<T, GeoError>;
        #[uniffi::export]
        pub fn heading() -> f64 { 0.0 }
        #[uniffi::export]
        pub fn span_meters() -> u8 { 0 }
        #[uniffi::export]
        pub fn buffer(radius_meters: f64) -> u8 { 0 }
        #[uniffi::export]
        pub fn scale(factor: f64) -> u8 { 0 }

        #[uniffi::export]
        pub fn height_meters() -> f64 { 0.0 }
        #[uniffi::export]
        pub fn offset_m(dx_m: f64, bearing_deg: Option<f64>, width_px: f64, n: u32) -> GeoResult<f64> { Ok(0.0) }
        #[uniffi::export]
        pub fn confidence_new(value: f64) -> u8 { 0 }
    ";
    let aliases = result_aliases(&[fixture.to_string()]);
    let (wrong, unsuffixed) = super::unit_suffix_violations(&exports(fixture), &aliases);
    assert_eq!(
        wrong,
        [
            "heading: returns a bare f64 with no unit suffix",
            "span_meters: `_meters` where the scheme uses `_m`",
            "buffer(radius_meters): `_meters` where the scheme uses `_m`",
            "scale(factor): a bare f64 parameter with no unit suffix",
        ]
    );
    assert_eq!(unsuffixed, BTreeSet::from(["confidence_new(value)".into()]));
}

#[test]
fn parity_reads_the_public_api_listing() {
    let listing = "\
pub mod geocoordinates::cluster
pub struct geocoordinates::cluster::Cluster
impl geocoordinates::cluster::Cluster
pub const fn geocoordinates::cluster::Cluster::bounds(&self) -> geocoordinates::geodesy::GeoBounds
pub fn geocoordinates::cluster::Cluster::new(usize) -> geocoordinates::Result<Self>
impl core::clone::Clone for geocoordinates::cluster::Cluster
pub fn geocoordinates::cluster::Cluster::clone(&self) -> Self
#[non_exhaustive] pub struct geocoordinates::cluster::Options
pub geocoordinates::cluster::Options::radius_px: f64
pub const fn geocoordinates::cluster::Options::with_radius_px(self, f64) -> Self
pub trait geocoordinates::LatLon
pub fn geocoordinates::LatLon::lat(&self) -> f64
pub const geocoordinates::cluster::MAX_ZOOM: u8
pub fn geocoordinates::cluster::centroid<T>(&[T]) -> geocoordinates::Result<T>
pub fn geocoordinates::prelude::reexported()
";
    let labels: Vec<String> = api_items(listing).iter().map(ApiItem::label).collect();
    assert_eq!(
        labels,
        [
            "Cluster::bounds",
            "Cluster::new",
            "Options::with_radius_px",
            "MAX_ZOOM",
            "centroid"
        ]
    );

    let fixture = "
        pub struct Cluster { pub bounds: GeoBounds, count: u64 }
        pub struct Options { pub radius_px: f64 }
        pub struct Tuple(pub f64);
        #[uniffi::export]
        pub fn cluster_validate(c: Cluster) {}
        #[uniffi::export]
        pub fn centroid() {}
        #[uniffi::export]
        impl Index {
            pub fn len(&self) -> u64 { 0 }
        }
    ";
    let fields = record_fields(&[fixture.to_string()]);
    assert_eq!(fields["Cluster"], BTreeSet::from(["bounds".to_string()]));
    let exports = exports(fixture);
    let reached = |label: &str| {
        let item = match label.split_once("::") {
            Some((owner, name)) => ApiItem {
                owner: Some(owner.into()),
                name: name.into(),
            },
            None => ApiItem {
                owner: None,
                name: label.into(),
            },
        };
        reaches(&item, &exports, &fields)
    };
    assert!(reached("Cluster::bounds"));
    assert!(reached("Cluster::new"));
    assert!(reached("Options::with_radius_px"));
    assert!(reached("centroid"));
    assert!(reached("Index::len"));
    assert!(!reached("Cluster::count"));
    assert!(!reached("Other::len"));
    assert!(!reached("MAX_ZOOM"));
}
