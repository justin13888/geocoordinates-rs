# Roadmap

`geocoordinates` is released **incrementally**. The full intended API surface was
scaffolded up front (see git history), but shipping it all at once would publish
methods that `todo!()`-panic. Instead, only implemented-and-tested code is part of
the public surface; everything else is commented out at its `pub mod` declaration
(plus re-exports, prelude entries, and Cargo feature) and brought back **one release
at a time**.

This file is the order in which that happens. It is excluded from the published crate
(`exclude = ["ROADMAP.md"]` in `Cargo.toml`) — it's a contributor-facing plan, not API docs.

## How a milestone ships

For each milestone below:

1. Uncomment the module's `pub mod`, re-exports, and prelude entries in `src/lib.rs`
   (and submodule re-exports, e.g. in `src/geodesy/mod.rs`).
2. Uncomment the matching Cargo feature(s) in `Cargo.toml` if any.
3. Implement the `todo!()` bodies (the stub source already exists on disk).
4. Add tightly-scoped tests (reference vectors, round-trip stability, edge cases:
   antimeridian, poles, out-of-range).
5. `just check` (fmt + clippy `-D warnings` + test) and `cargo doc` must stay clean.
6. Bump the version and release.

Versions are `0.x`, so each minor may make breaking changes until `1.0`.

## Release order

Ordering follows the dependency graph: a capability ships only after everything it
builds on.

### 0.1 — Foundation *(shipped — the current surface)*

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

### 0.2 — Angles & units complete

Small, dependency-free primitive math, unlocking honest angle handling:

- `Dd` ↔ `Dms` ↔ `Ddm` conversions (`From`/`to_*`)
- `angle::wrap_longitude` / `clamp_latitude` / `normalize_degrees`
- `Length::from_unit` / `to_unit`
- `Coordinate::validate` / `is_null_island`

### 0.3 — Geodesy I: frames

- `Ellipsoid` (parameters + derived quantities)
- `Ecef` (geodetic ↔ geocentric)
- local tangent frames: `Enu`, `Ned`, `Aer`

### 0.4 — Geodesy II: geodesics

Reuses `geo` where it already implements the math:

- exact `geodesic_distance` (Karney), `rhumb_distance`
- `initial_bearing` / `final_bearing` / `rhumb_bearing`
- producers: `destination`, `midpoint`, `intermediate`, `intersection`, `rhumb_destination`
- `cross_track_distance` / `along_track_distance`

### 0.5 — Geodesy III: classic datums

- `Helmert` (7-parameter Bursa-Wolf) + `DatumTransform`
- classic datum support for `Nad27`, `Tokyo`, `Pulkovo42`

### 0.6 — Convert dispatch

- `convert::convert` / `can_convert` — runtime routing over `Crs`, now able to route
  both China datums and the classic datums through a WGS-84 hub.

### 0.7 — Grids I: UTM / UPS

- `Utm`, `Ups` projections (+ `TryFrom<Coordinate>`).

### 0.8 — Grids II: MGRS

- `Mgrs` (depends on UTM/UPS).

### 0.9 — Grids III: encoded systems

- `Geohash`, `PlusCode`, `Maidenhead` (`encode` / `decode`).

### 0.10 — Parse I: text

- `parse::parse_coordinate`, `from_geo_uri`, `text::parse_with`
- enables `Coordinate: FromStr` (needs 0.2 angle conversions).

### 0.11 — Format / presentation

- `format::format` / `format_fix`, `FormatOptions`, `Representation`
- enables `Coordinate: Display` (needs 0.2 + grids for UTM/MGRS/PlusCode/Geohash output).

### 0.12 — Parse II: interchange *(feature-gated)*

- `from_geojson` / `from_wkt` / `from_gpx` / `from_kml`
- features: `geojson`, `wkt`, `gpx`, `kml`.

### 0.13 — Parse III: sensors *(feature-gated)*

- `from_nmea_sentence`, `from_exif`
- features: `nmea`, `exif`.

### Long tail *(optional, feature-gated; scheduled as the integrations mature)*

- `proj` — PROJ-backed EPSG/datum long tail (`CrsId`, `proj::transform`).
- `geoid` — EGM96/EGM2008/EGM2020 height undulation (`height` module).
- `dgg` — Uber H3 / Google S2 indexing.

### 1.0 — API freeze

Once the surface above is implemented and has proven stable, freeze the public API and
adopt strict semver. Add a coverage threshold (see `just coverage`).
