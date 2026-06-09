# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Versions are `0.x`; each minor may make breaking changes until `1.0`. Releases ship
incrementally — see [ROADMAP.md](ROADMAP.md) for the planned order.

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
