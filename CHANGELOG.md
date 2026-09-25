# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Versions are `0.x`; each minor may make breaking changes until `1.0`. See
[STABILIZATION.md](STABILIZATION.md) for the stabilization ledger and the road to
the 1.0 API freeze.

## [0.14.2](https://github.com/justin13888/geocoordinates-rs/compare/v0.14.1...v0.14.2) - 2026-09-25

### Fixed

- *(format)* derive DMS/DDM fix precision in arcseconds/arcminutes ([#77](https://github.com/justin13888/geocoordinates-rs/pull/77))
- *(dgg)* measure H3/S2 decode error bounds on the WGS-84 ellipsoid ([#76](https://github.com/justin13888/geocoordinates-rs/pull/76))
- *(mgrs)* state decode bound in ground meters via the minimum scale factor ([#75](https://github.com/justin13888/geocoordinates-rs/pull/75))
- *(parse)* keep RawSource.raw verbatim on every parse_coordinate path ([#74](https://github.com/justin13888/geocoordinates-rs/pull/74))
- *(mgrs)* report band/row mismatch as InvalidGridRef ([#73](https://github.com/justin13888/geocoordinates-rs/pull/73))
- *(parse)* round-trip letter-style and decimal-comma formatted output ([#72](https://github.com/justin13888/geocoordinates-rs/pull/72))
- *(grids)* don't panic on multibyte characters in MGRS parsing ([#41](https://github.com/justin13888/geocoordinates-rs/pull/41))
- *(parse)* don't panic on non-ASCII or short NMEA input ([#39](https://github.com/justin13888/geocoordinates-rs/pull/39))

### Other

- declare per-feature MSRV for geojson and h3 ([#97](https://github.com/justin13888/geocoordinates-rs/pull/97))
- *(release)* dispatch CI and FFI on the release-plz PR branch ([#96](https://github.com/justin13888/geocoordinates-rs/pull/96))
- tidy .gitignore and drop redundant rustfmt.toml ([#95](https://github.com/justin13888/geocoordinates-rs/pull/95))
- *(mise)* deny rustc warnings in local tasks as CI does ([#94](https://github.com/justin13888/geocoordinates-rs/pull/94))
- deduplicate release workflow version, publish gate, and bindings generation ([#91](https://github.com/justin13888/geocoordinates-rs/pull/91))
- pin mise, cargo-llvm-cov, and npm versions instead of floating ([#90](https://github.com/justin13888/geocoordinates-rs/pull/90))
- run the mutants job only on pull requests ([#89](https://github.com/justin13888/geocoordinates-rs/pull/89))
- *(release)* dry-run the binding release workflows on pull requests and weekly ([#88](https://github.com/justin13888/geocoordinates-rs/pull/88))
- *(ffi)* compile the Swift package on pull requests ([#86](https://github.com/justin13888/geocoordinates-rs/pull/86))
- *(hooks)* run ffi-check in the pre-push and check hooks ([#85](https://github.com/justin13888/geocoordinates-rs/pull/85))
- build and test on the declared MSRV ([#84](https://github.com/justin13888/geocoordinates-rs/pull/84))
- enforce rustdoc warnings and missing_docs ([#81](https://github.com/justin13888/geocoordinates-rs/pull/81))
- run full-tree mutation tests weekly ([#79](https://github.com/justin13888/geocoordinates-rs/pull/79))
- check MGRS decode bounds with a geodesic oracle ([#78](https://github.com/justin13888/geocoordinates-rs/pull/78))
- replace exactness claims with measured error bounds ([#71](https://github.com/justin13888/geocoordinates-rs/pull/71))
- correct the stabilization ledger, npm publishing notes and gate descriptions ([#42](https://github.com/justin13888/geocoordinates-rs/pull/42))
- correct stale and contradicted core documentation ([#40](https://github.com/justin13888/geocoordinates-rs/pull/40))

## [0.14.1](https://github.com/justin13888/geocoordinates-rs/compare/v0.14.0...v0.14.1) - 2026-09-01

### Added

- *(china)* add named Coordinate bridges to the datum newtypes

### Fixed

- *(dgg)* preserve datum semantics when indexing
- *(grids)* make spatial indexes total and bounds honest
- *(geodesy)* enforce datum and height semantics
- *(parse)* enforce coordinate format invariants
- *(core)* reject invalid numeric domain inputs
- *(angle)* add serde support for axis

### Other

- retarget tooling docs at mise and hk
- remove just and the justfile
- run every gate through mise tasks
- *(hk)* replace lefthook with hk for git hooks
- *(mise)* port justfile recipes to mise tasks
- consolidate repo state into a surface table and a ledger
- *(api)* align fallible conversion contracts
- *(example)* exercise adversarial API boundaries
- *(example)* add verified full-surface flagship
- *(changelog)* backfill release notes for 0.2.0 through 0.14.0

## [0.14.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.13.0...v0.14.0) - 2026-07-11

### Added

- *(ffi)* mirror H3 encode/decode + bump 0.14.0
- *(dgg)* H3 (h3o) + S2 (s2) indexing; S2 native-only (wasm)

### Other

- *(dgg)* tighten H3/S2 cell-radius bounds to pin the formulas
- *(dgg)* pin H3/S2 decode cell-radius bounds

## [0.13.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.12.0...v0.13.0) - 2026-07-11

### Added

- *(ffi)* mirror from_nmea_sentence + bump 0.13.0
- *(parse)* NMEA 0183 sensor parsing (GGA/RMC/GLL, checksum-verified)

### Other

- *(parse)* pin GLL valid-status (no void note)

## [0.12.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.11.0...v0.12.0) - 2026-07-11

### Added

- *(ffi)* mirror interchange parsers + bump 0.12.0 (kml without KMZ for wasm)
- *(parse)* interchange parsers — GeoJSON/WKT/GPX/KML (feature-gated)

### Other

- *(parse)* exercise every KML geometry/walk arm

## [0.11.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.10.0...v0.11.0) - 2026-07-11

### Added

- *(ffi)* mirror UTM/UPS/MGRS + bump 0.11.0
- *(grids)* UTM/UPS projections + MGRS (Karney-Krüger + polar)

### Other

- *(grids)* pin Svalbard predicate, coarse-encode scaling, band-X & band-edge decode
- *(grids)* pin MGRS decode scaling, band-X, polar A-zone, zone predicates

## [0.10.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.9.0...v0.10.0) - 2026-07-11

### Added

- *(ffi)* mirror convert/can_convert + bump 0.10.0
- *(convert)* runtime CRS dispatch via WGS-84 hub (China + classic datums)

## [0.9.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.8.0...v0.9.0) - 2026-07-11

### Added

- *(ffi)* mirror Helmert/DatumTransform + bump 0.9.0
- *(geodesy)* classic-datum Helmert transforms (NAD27/Tokyo/Pulkovo42)

## [0.8.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.7.0...v0.8.0) - 2026-07-11

### Added

- *(geodesy)* implement geodesics (Karney + spherical rhumb/track)

### Other

- *(geodesy)* use asymmetric same-meridian intersection to pin azimuth branch
- *(geodesy)* pin intersection oblique-bearing and shared-meridian branches
- *(geodesy)* pin spherical helpers + rhumb/intersection edge branches

## [0.7.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.6.0...v0.7.0) - 2026-07-11

### Added

- *(ffi)* mirror the geodesy frames
- *(geodesy)* implement Ellipsoid quantities, ECEF, and ENU/NED/AER frames

## [0.6.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.5.0...v0.6.0) - 2026-07-11

### Added

- *(ffi)* mirror Geohash and Maidenhead
- *(grids)* implement Geohash and Maidenhead

## [0.5.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.4.0...v0.5.0) - 2026-07-11

### Added

- *(ffi)* mirror Plus Code
- add Plus Code as a Representation and parse-detect it
- *(grids)* implement Plus Code (Open Location Code)

### Other

- *(grids)* decode pair loop as a for-loop

## [0.4.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.3.0...v0.4.0) - 2026-07-11

### Added

- *(ffi)* mirror the parse surface
- *(parse)* implement text + geo: URI parsing

## [0.3.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.2.0...v0.3.0) - 2026-07-11

### Added

- *(ffi)* mirror the format surface
- *(coord)* implement Display for Coordinate (DD default)
- *(format)* implement DD/DMS/DDM rendering

## [0.2.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.1.1...v0.2.0) - 2026-06-23

### Added

- *(ffi)* mirror angle/units/Fix surface; make Crs exhaustive
- *(coord)* implement Coordinate::validate/is_null_island
- *(units)* implement Length::from_unit/to_unit; make LengthUnit exhaustive
- *(angle)* implement wrap_longitude/clamp_latitude/normalize_degrees
- *(angle)* implement DD/DMS/DDM conversions

### Fixed

- *(npm)* rename package to geocoordinates-rs (bare name is blocked)
- *(ci)* cross-build Intel macOS bindings on arm64 runners

### Other

- *(angle)* kill equivalent mutants in conversions and wrap_longitude
- *(fix)* make DatumAmbiguity exhaustive
- *(angle)* drop intra-doc link to not-yet-released format module
- link published binding packages to their registries in README
- redefine crate scope as low-level geo primitives
- cancel superseded PR runs and enforce conventional commits
- *(mutants)* add cargo-mutants mutation gate (mise + just + pre-push + CI)
- *(npm)* switch to OIDC trusted publishing, drop NPM_TOKEN

## [0.1.1](https://github.com/justin13888/geocoordinates-rs/compare/v0.1.0...v0.1.1) - 2026-06-12

### Added

- *(ffi)* publish bindings to PyPI, npm, Maven Central, and SwiftPM ([#5](https://github.com/justin13888/geocoordinates-rs/pull/5))
- *(ffi)* add UniFFI bindings for the v0.1 surface (Python/Kotlin/Swift/Ruby)

### Fixed

- *(ci)* set publish=false for geocoordinates-ffi in release-plz config
- *(ci)* repair corrupted release-plz action ref
- *(ci)* resolve host lib under set -e in swift/jvm release builds
- *(ffi)* publish to PyPI as geocoordinates-rs (bare name is taken)

### Other

- auto-publish bindings by dispatching from release-plz (no tokens)
- *(ffi)* retarget bindings to Python/Kotlin/Java/Swift/TypeScript, drop Ruby

## [0.1.0]

Foundation release: the canonical coordinate data model plus the flagship China datums.

### Added

- Coordinate model: `Coordinate`, `Crs`, `Height`, `LatLon`, `Approx<T>`, and typed
  `Error` / `Result`.
- Length handling: `Length` / `LengthUnit` with arithmetic.
- Angle encodings (types): `Dd`, `Dms`, `Ddm`, `Hemisphere`, `Axis`.
- Observation metadata: `Fix`, `Accuracy`, `RawSource`, `AxisOrder`, `DatumAmbiguity`,
  `Confidence`.
- China datums: `Wgs84` / `Gcj02` / `Bd09` with exact forward transforms and approximate
  inverses (explicit error bounds), plus `BaiduMercator`.
- Geodesy: spherical `geodesy::haversine_distance`.
- Optional `serde` support behind the `serde` feature.
