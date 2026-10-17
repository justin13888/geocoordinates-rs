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
//! | `Approx<T>` (generic wrapper) | flat records [`ApproxWgs84`] / [`ApproxGcj02`] carrying `max_error_m` |
//! | `LatLon` trait / `impl LatLon` | functions take the concrete [`Coordinate`] record |
//! | `Wgs84::is_outside_china` / `Gcj02::is_outside_china` methods | [`wgs84_is_outside_china`] / [`gcj02_is_outside_china`] |
//! | `Length` + operator overloads | distance returned as `f64` meters ([`haversine_distance_m`]) |
//! | `From` / `TryFrom` conversions | named free functions (`wgs84_to_gcj02`, …) |
//!
//! Exactness stays visible across the boundary: exact forward conversions
//! return a bare datum record, while approximate inverses return an `Approx*`
//! record and keep the `_fast` / `_refined` suffix.
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
mod geodesy;
mod grids;
mod interchange;
mod text;

pub use self::angle::*;
pub use self::china::*;
pub use self::core::*;
pub use self::datum::*;
pub use self::dgg::*;
pub use self::error::*;
pub use self::geodesy::*;
pub use self::grids::*;
pub use self::interchange::*;
pub use self::text::*;
