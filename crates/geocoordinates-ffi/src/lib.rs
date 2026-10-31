//! UniFFI bindings for [`geocoordinates`](https://docs.rs/geocoordinates).
//!
//! This crate exposes the Rust API to Python, Kotlin, Swift, and TypeScript/WASM
//! (and Java, via the Kotlin/JVM artifact). **Full capability parity** with the
//! released `geocoordinates` surface is the target, and the mirror gates each release;
//! open gaps are tracked in [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md).
//! Several Rust idioms cannot cross an FFI boundary, so they are re-expressed
//! as flat, language-neutral records and free functions:
//!
//! | Rust idiom | FFI form here |
//! |---|---|
//! | `Approx<T>` (generic wrapper) | flat records [`ApproxWgs84`] / [`ApproxGcj02`] (China inverses) and [`ApproxCoordinate`] ([`convert`] and every grid / cell decoder), each carrying `max_error_m`; output-only, so `Approx::new` is Rust-only |
//! | `LatLon` trait / `impl LatLon` | functions take the concrete [`Coordinate`] record |
//! | `Wgs84::is_outside_china` / `Gcj02::is_outside_china` methods | [`wgs84_is_outside_china`] / [`gcj02_is_outside_china`] |
//! | `Length` + operator overloads | distance returned as `f64` meters ([`haversine_distance_m`]) |
//! | `From` / `TryFrom` conversions | named free functions (`wgs84_to_gcj02`, …) |
//! | Inherent methods and constructors | free functions prefixed with the type (`coordinate_new`, `coordinate_from_lon_lat`, `utm_to_coordinate`, …) |
//! | `[ExifRational; 3]` (a tag of three rationals) | the named records [`ExifDms`] and [`ExifTime`] |
//! | `SpatialIndex<T>` (generic, holds its entries) | the object [`SpatialIndex`] over coordinates, whose results carry each entry's build-list position for the caller's own payload lookup |
//!
//! Exactness stays visible across the boundary: exact forward conversions
//! return a bare datum record, while approximate inverses return an `Approx*`
//! record and keep the `_fast` / `_refined` suffix.
//!
//! Every export follows one naming scheme, so a name reads the same in every
//! language:
//!
//! - **Units in the name.** A function returning a bare length names its
//!   unit (`_m`: [`geodesic_distance_m`]; [`height_meters`]), and one
//!   returning an angle ends in `_deg` ([`initial_bearing_deg`],
//!   [`wrap_longitude_deg`], [`utm_central_meridian_deg`]) or names degrees
//!   as its target ([`dms_to_degrees`], [`normalize_degrees`]). Length and
//!   angle parameters carry the same suffix (`distance_m`, `bearing_deg`).
//!   Two exceptions, where the unit is already fixed elsewhere: a
//!   constructor's parameters take the names of the record fields they fill,
//!   so [`coordinate_new`] and [`coordinate_from_lon_lat`] take `lat` / `lon`
//!   like the [`Coordinate`] record (decimal degrees); and the `Length`
//!   converters name the unit they convert between, with meters always the
//!   other side ([`length_from_unit`] returns meters, [`length_to_unit`]
//!   takes them).
//! - **Ingestion is `parse_*`.** Every function that reads text into a
//!   [`Fix`] is named for what it does: [`parse_coordinate`], [`parse_text`],
//!   [`parse_geo_uri`], [`parse_geojson`], [`parse_wkt`], [`parse_gpx`],
//!   [`parse_kml`], [`parse_nmea_sentence`]. (The core crate's readers are
//!   `from_geojson` and so on, inside a module that says what they produce; a
//!   top-level foreign function has no such module.)
//! - **`*_validate` for every invariant record.** A record whose core type is
//!   valid by construction has a `<record>_validate` that checks a hand-built
//!   one ([`coordinate_validate`], [`utm_validate`], [`h3_validate`],
//!   [`datum_transform_validate`], …). No `<record>_new` is added that would
//!   only copy its arguments into the record and validate them; one exists
//!   where construction derives or normalizes a field ([`aer_new`] normalizes
//!   the azimuth, [`raw_source_new`] fills the optional fields), and the
//!   constructors that predate this rule ([`coordinate_new`],
//!   [`ellipsoid_new`], [`accuracy_new`], [`confidence_new`],
//!   [`datum_transform_new`]) fail at construction rather than at first use.
//!   The `Approx*` records are the exception: every export that returns one
//!   is a decoder, [`convert`] or a China inverse, and no export takes one as
//!   input, so they have no `_new` or `_validate` and `Approx::new` has no
//!   FFI form.
//! - **`*_canonicalize` for grid codes.** A string grid code validates and
//!   normalizes through `<grid>_canonicalize` ([`mgrs_canonicalize`], …).
//!
//! The angle encodings (`Dms` / `Ddm`), their conversions and the
//! normalization helpers, the `Length` / `LengthUnit` unit helpers, coordinate
//! validation, and the `Fix` observation family (`Fix` / `Accuracy` /
//! `AccuracyKind` / `RawSource` / `Confidence`, with `SystemTime` mapped natively to UniFFI's
//! builtin `Timestamp`) all cross the boundary as flat records / enums and free
//! functions.
//!
//! Mirror records are plain data, so foreign code can build one with any field
//! values (an out-of-range latitude, a negative semi-major axis, a confidence
//! of `7.0`). A [`Coordinate`] record (alone or inside a [`Fix`]) is
//! **validated as it crosses into Rust**: every exported function that takes
//! one rejects an out-of-range or non-finite record with a `GeoError` before
//! doing anything else, because the core `Coordinate` is valid by
//! construction. Build one with [`coordinate_new`] to fail at construction
//! instead. [`Accuracy`] and [`Confidence`] records are validated the same way
//! (alone or inside a [`Fix`] / [`RawSource`]): build them with
//! [`accuracy_new`] / [`confidence_new`], or check a hand-built one with
//! [`accuracy_validate`] / [`confidence_validate`]. Every other record is **unvalidated until
//! used**: each exported function applies the core crate's own rules to the
//! values it consumes, exactly as the matching Rust function does.

uniffi::setup_scaffolding!();

// Each module mirrors one subsystem of the core crate. The mirrors are
// hand-written (rather than `#[uniffi::remote]`) so the FFI surface is
// flattened into language-neutral forms — and because `Approx<T>` must be
// flattened anyway. Every module holds its mirror types, their `From`
// translations to and from the core types, and its exported functions; all of
// it is re-exported here, so the crate's public paths stay flat.

mod angle;
mod china;
mod core;
mod datum;
mod dgg;
mod error;
mod exif;
mod geodesy;
mod grids;
mod index;
mod interchange;
mod text;

pub use self::angle::*;
pub use self::china::*;
pub use self::core::*;
pub use self::datum::*;
pub use self::dgg::*;
pub use self::error::*;
pub use self::exif::*;
pub use self::geodesy::*;
pub use self::grids::*;
pub use self::index::*;
pub use self::interchange::*;
pub use self::text::*;
