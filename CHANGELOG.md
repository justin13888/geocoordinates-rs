# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Versions are `0.x`; each minor may make breaking changes until `1.0`. See
[STABILIZATION.md](STABILIZATION.md) for the stabilization ledger and the road to
the 1.0 API freeze.

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
