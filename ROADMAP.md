# Roadmap

`geocoordinates` is released **incrementally**. The full intended API surface was
scaffolded up front (see git history), but shipping it all at once would publish
methods that `todo!()`-panic. Instead, only implemented-and-tested code is part of
the public surface; everything else is commented out at its `pub mod` declaration
(plus re-exports, prelude entries, and Cargo feature) and brought back **one release
at a time**.

This file is the plan. Milestones are **named** (version numbers are assigned at
release time): the **core path** ships in order, the **deferred** section is
unscheduled, and a few things are explicitly **out of scope**. It is excluded from
the published crate (`exclude = ["ROADMAP.md"]` in `Cargo.toml`) — it's a
contributor-facing plan, not API docs.

## Scope

This crate provides **low-level geospatial coordinate primitives** for higher-level
libraries to consume (UNIX philosophy: abstract only the geo-related complexity).
The core path covers what is strictly necessary to (a) process photo geolocation
metadata and (b) convert coordinates into the representations used by maps
(Google, Gaode/AMap, Baidu, Apple). Everything else stays deferred until a concrete
consumer appears.

## How a milestone ships

For each milestone below:

1. Uncomment the module's `pub mod`, re-exports, and prelude entries in `src/lib.rs`
   (and submodule re-exports, e.g. in `src/geodesy/mod.rs`).
2. Uncomment the matching Cargo feature(s) in `Cargo.toml` if any.
3. Implement the `todo!()` bodies (the stub source already exists on disk).
4. Add tightly-scoped tests (reference vectors, round-trip stability, edge cases:
   antimeridian, poles, out-of-range).
5. Mirror the new surface in `geocoordinates-ffi` (full capability parity — the FFI
   crate gates the release; see the FFI section below) and keep `just ffi-check`
   clean.
6. `just check` (fmt + clippy `-D warnings` + test) and `cargo doc` must stay clean.
7. Bump the version and release.

Versions are `0.x`, so each minor may make breaking changes until `1.0`.

## Shipped

### Foundation *(0.1 — the current surface)*

The canonical data model plus the flagship China datums:

- `Coordinate`, `Crs`, `Height`, `LatLon`
- `Approx<T>`, `Error` / `Result`
- `Length` / `LengthUnit` (+ arithmetic)
- angle encodings: `Dd`, `Dms`, `Ddm`, `Hemisphere`, `Axis` (types only)
- `Fix`, `Accuracy`, `RawSource`, `AxisOrder`, `DatumAmbiguity`, `Confidence`
- **China datums:** `Wgs84` / `Gcj02` / `Bd09` (exact forward + approximate inverses),
  `BaiduMercator`
- `geodesy::haversine_distance`
- the `serde` feature

## Core path

Ordering follows the dependency graph: a capability ships only after everything it
builds on.

### 1. Angles & units complete

Small, dependency-free primitive math, unlocking honest angle handling. These are
the primitives the external EXIF library consumes (GPS rationals → decimal degrees,
hemisphere signs), so this milestone is the immediate priority:

- `Dd` ↔ `Dms` ↔ `Ddm` conversions (`From`/`to_*`)
- `angle::wrap_longitude` / `clamp_latitude` / `normalize_degrees`
- `Length::from_unit` / `to_unit`
- `Coordinate::validate` / `is_null_island`

This (breaking) minor also carries two FFI-coherence changes:

- Drop `#[non_exhaustive]` from the FFI-mirrored **data** enums (`Crs`,
  `LengthUnit`, `DatumAmbiguity`; stub-side `Representation` / `SymbolStyle` /
  `HemisphereStyle`, `GeoidModel`) so adding a variant **fails the FFI mirror's
  compile** instead of silently hitting a wildcard arm. Delete the mirror's
  `_ => Crs::Wgs84` fallback. `Error` keeps `#[non_exhaustive]` — its FFI
  catch-all (`GeoError::Other { detail }`) preserves meaning, unlike a datum
  fallback which would mislabel data.
- FFI catch-up for the 0.1 surface: `Fix` / `Accuracy` / `RawSource` /
  `Confidence` (UniFFI maps `SystemTime` natively via its builtin `Timestamp`),
  the angle types, and the new `Length` unit helpers as free functions.

### 2. Format: DD / DMS / DDM

- `format::format` / `format_fix`, `FormatOptions`, `Representation` (DD/DMS/DDM
  only — grid representations are added by their own milestones)
- enables `Coordinate: Display` (needs angles & units only)

### 3. Parse: text + `geo:` URI

- `parse::parse_coordinate`, `from_geo_uri`, `text::parse_with`
- enables `Coordinate: FromStr`
- round-trip parse ↔ format tests close the loop with the format milestone

### 4. Plus Code

- `PlusCode` (`encode` / `decode`) — Google Maps' shareable representation, the one
  grid system that is a map representation rather than an indexing scheme.
  Re-extends `Representation` and `parse_coordinate` token detection (each
  re-added variant compile-forces the FFI mirror update).

The live 0.1 surface (China datums for Gaode/Baidu/Apple-in-China, `BaiduMercator`,
`haversine_distance` for clustering, the `Fix` model) covers the rest of the
map-conversion story.

## Deferred

Unscheduled — promoted into the core path only when a concrete consumer appears.
Dependency notes are kept so the order stays derivable:

- **Geohash** (first candidate, if a consumer needs spatial-index keys) and
  **Maidenhead** (`encode` / `decode`).
- **Geodesy I: frames** — `Ellipsoid` (parameters + derived quantities), `Ecef`
  (geodetic ↔ geocentric), local tangent frames `Enu` / `Ned` / `Aer`.
- **Geodesy II: geodesics** — exact `geodesic_distance` (Karney, delegated to
  [`geographiclib-rs`](https://crates.io/crates/geographiclib-rs), the same engine
  the `geo` crate uses, with `default-features = false`), hand-rolled spherical
  `rhumb_distance` / `rhumb_bearing` / `rhumb_destination`, `initial_bearing` /
  `final_bearing`, producers (`destination`, `midpoint`, `intermediate`,
  `intersection`), `cross_track_distance` / `along_track_distance`.
- **Geodesy III: classic datums** — `Helmert` (7-parameter Bursa-Wolf) +
  `DatumTransform`; classic datum support for `Nad27`, `Tokyo`, `Pulkovo42`.
- **Convert dispatch** — `convert::convert` / `can_convert`; only matters once
  multiple datum families exist (the China bridge already routes
  WGS-84 / GCJ-02 / BD-09).
- **UTM / UPS** (needs Ellipsoid) → **MGRS** (needs UTM/UPS).
- **Interchange** *(feature-gated)* — `from_gpx` (track-log geotagging),
  `from_geojson`, `from_wkt`, `from_kml`; features `gpx`, `geojson`, `wkt`, `kml`.
  (The `gpx` crate brings `geo-types` in transitively; that's acceptable — it's
  light and never touches the public API.)
- **Sensors: NMEA** *(feature-gated)* — `from_nmea_sentence`; feature `nmea`.
- **Long tail** *(optional, feature-gated)* — `proj` (PROJ-backed EPSG/datum long
  tail: `CrsId`, `proj::transform`), `geoid` (EGM96/EGM2008/EGM2020 height
  undulation), `dgg` (Uber H3 / Google S2 indexing).

## Out of scope

- **EXIF/XMP GPS extraction** — handled by a separate library that consumes this
  crate's primitives (angle conversions for GPS rationals, `Fix` / `RawSource`,
  `DatumAmbiguity::PossiblyGcj02` for the China-EXIF ambiguity). The `from_exif`
  stub and `exif` feature were removed.
- **Polygon / line geometry ops** (area, point-in-polygon, centroid, simplification,
  …) — use the [`geo`](https://crates.io/crates/geo) crate directly.

## FFI bindings *(gating; full capability parity)*

The `geocoordinates-ffi` crate exposes the live surface to Python / Kotlin / Java /
Swift / TypeScript via UniFFI with **full capability parity**: every public
capability is reachable through one canonical FFI form (concrete records, free
functions, primitives) — though not necessarily every Rust-side overload, trait
impl, or operator. The FFI mirror **gates every release**: a milestone is not done
until its surface crosses the boundary (step 5 of "How a milestone ships").

The crate is `publish = false` (bindings ship via PyPI / npm / Maven Central /
SwiftPM, not crates.io). A catch-up for the 0.1 surface rides the angles & units
milestone. See `README.md` and the FFI-translatability note in `AGENTS.md`.

## 1.0 — API freeze

Once the core path is implemented and has proven stable, freeze the public API and
adopt strict semver. Add a coverage threshold (see `just coverage`).
