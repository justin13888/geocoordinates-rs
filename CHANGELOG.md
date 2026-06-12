# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Versions are `0.x`; each minor may make breaking changes until `1.0`. Releases ship
incrementally — see [ROADMAP.md](ROADMAP.md) for the planned order.

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
