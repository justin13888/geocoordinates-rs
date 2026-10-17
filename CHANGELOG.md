# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Versions are `0.x`; each minor may make breaking changes until `1.0`. See
[STABILIZATION.md](STABILIZATION.md) for the stabilization ledger and the road to
the 1.0 API freeze.

## [0.15.0](https://github.com/justin13888/geocoordinates-rs/compare/v0.14.2...v0.15.0) - 2026-10-17

### Added

- *(format)* [**breaking**] non-exhaustive option builders, round-trip decimal caps, Hash derives ([#210](https://github.com/justin13888/geocoordinates-rs/pull/210))
- *(parse)* [**breaking**] structured parse errors with ParseErrorKind ([#203](https://github.com/justin13888/geocoordinates-rs/pull/203))
- *(geodesy)* [**breaking**] ellipsoidal cross-track, along-track and intersection ([#202](https://github.com/justin13888/geocoordinates-rs/pull/202))
- *(grids)* inherent parse on grid codes, Hash on ids, H3/S2 level getters, dedupe grid math ([#201](https://github.com/justin13888/geocoordinates-rs/pull/201))
- *(fix)* [**breaking**] validated, kind-tagged Accuracy and an extensible Fix model ([#195](https://github.com/justin13888/geocoordinates-rs/pull/195))
- *(parse)* [**breaking**] state axis order and altitude meaning explicitly in interchange parsing ([#155](https://github.com/justin13888/geocoordinates-rs/pull/155))
- *(parse)* [**breaking**] add ParseOptions with grid detection and reject ambiguous free text ([#154](https://github.com/justin13888/geocoordinates-rs/pull/154))
- *(format)* [**breaking**] let Representation carry its own settings and cover every grid ([#150](https://github.com/justin13888/geocoordinates-rs/pull/150))
- *(grids)* [**breaking**] give MGRS a typed precision, validated serde and Display ([#148](https://github.com/justin13888/geocoordinates-rs/pull/148))

### Fixed

- *(grids)* [**breaking**] accept only the forward image in Utm::new, Ups::new and Mgrs::parse ([#211](https://github.com/justin13888/geocoordinates-rs/pull/211))
- *(parse)* reject a KML Point with no coordinates instead of panicking ([#209](https://github.com/justin13888/geocoordinates-rs/pull/209))
- *(parse)* reject unclosed KML instead of hanging in the kml reader ([#206](https://github.com/justin13888/geocoordinates-rs/pull/206))
- *(mgrs)* keep a point west of a central meridian in the square west of it ([#207](https://github.com/justin13888/geocoordinates-rs/pull/207))
- *(jvm)* release Maven Central deployments automatically ([#204](https://github.com/justin13888/geocoordinates-rs/pull/204))
- *(mgrs)* decode a truncated square inside its grid-zone designator ([#196](https://github.com/justin13888/geocoordinates-rs/pull/196))
- *(geodesy)* [**breaking**] honest errors for along-track at the path pole and bad Crs arguments; no -0.0 ([#193](https://github.com/justin13888/geocoordinates-rs/pull/193))
- *(mgrs)* emit only references the decoder accepts, and decode without re-parsing ([#194](https://github.com/justin13888/geocoordinates-rs/pull/194))
- *(china)* measured China inverse bounds, direct BD-09 to GCJ-02 in convert, and a fast BD-09 inverse ([#191](https://github.com/justin13888/geocoordinates-rs/pull/191))
- *(geodesy)* [**breaking**] rhumb bearing from a pole inverts with rhumb_destination ([#192](https://github.com/justin13888/geocoordinates-rs/pull/192))
- *(parse)* reject short GeoJSON positions and bound WKT/KML nesting ([#189](https://github.com/justin13888/geocoordinates-rs/pull/189))
- *(parse)* read an uppercase S after a seconds number as South ([#188](https://github.com/justin13888/geocoordinates-rs/pull/188))
- *(geodesy)* [**breaking**] compute rhumb lines on the datum ellipsoid and stop them at the poles ([#162](https://github.com/justin13888/geocoordinates-rs/pull/162))
- *(parse)* [**breaking**] reject NMEA sentences that report no fix and timestamp RMC ([#157](https://github.com/justin13888/geocoordinates-rs/pull/157))
- *(geodesy)* [**breaking**] make geodesics datum-aware, fix the pole intersection and name spherical functions ([#156](https://github.com/justin13888/geocoordinates-rs/pull/156))

### Other

- *(ffi)* record that Approx records are output-only with no FFI constructor ([#219](https://github.com/justin13888/geocoordinates-rs/pull/219))
- *(rhumb)* assert the zero-distance return is exact from every start ([#217](https://github.com/justin13888/geocoordinates-rs/pull/217))
- *(grids)* record the UTM inverse latitude rounding change ([#218](https://github.com/justin13888/geocoordinates-rs/pull/218))
- reconcile STABILIZATION, README and AGENTS with the audited surface ([#213](https://github.com/justin13888/geocoordinates-rs/pull/213))
- *(ffi)* [**breaking**] normalize FFI naming and close validation, capability and docs gaps ([#212](https://github.com/justin13888/geocoordinates-rs/pull/212))
- [**breaking**] uniform argument conventions (Coordinate by value, is_outside_china, LatLon/format docs) ([#208](https://github.com/justin13888/geocoordinates-rs/pull/208))
- one workspace version, compiled npm package, and release-PR packaging runs ([#200](https://github.com/justin13888/geocoordinates-rs/pull/200))
- *(geodesy)* [**breaking**] local frames valid by construction on any classic datum ([#199](https://github.com/justin13888/geocoordinates-rs/pull/199))
- docs.rs all-features, include allow-list, and per-feature CI ([#197](https://github.com/justin13888/geocoordinates-rs/pull/197))
- [**breaking**] honest Approx API and Length for every public magnitude ([#190](https://github.com/justin13888/geocoordinates-rs/pull/190))
- [**breaking**] give every public item one canonical path ([#187](https://github.com/justin13888/geocoordinates-rs/pull/187))
- reconcile README, STABILIZATION and conventions with the 0.15.0 surface ([#167](https://github.com/justin13888/geocoordinates-rs/pull/167))
- kill the surviving mutants in the core crate ([#166](https://github.com/justin13888/geocoordinates-rs/pull/166))
- enforce a ratcheting line-coverage floor in CI ([#165](https://github.com/justin13888/geocoordinates-rs/pull/165))
- *(geodesy)* add compiling doctests to every public geodesy item ([#164](https://github.com/justin13888/geocoordinates-rs/pull/164))
- *(core)* add compiling doctests to core types, angles and units ([#163](https://github.com/justin13888/geocoordinates-rs/pull/163))
- *(grids, dgg)* add compiling doctests to every public grid and cell item ([#161](https://github.com/justin13888/geocoordinates-rs/pull/161))
- *(ffi)* behavioural tests for the Kotlin, Swift and TypeScript bindings in CI ([#159](https://github.com/justin13888/geocoordinates-rs/pull/159))
- *(dgg)* [**breaking**] make H3 and S2 cell ids valid by construction ([#160](https://github.com/justin13888/geocoordinates-rs/pull/160))
- *(format, parse)* add compiling doctests to every public formatting and parsing item ([#158](https://github.com/justin13888/geocoordinates-rs/pull/158))
- *(china)* add compiling doctests with error cases to China datums and convert ([#153](https://github.com/justin13888/geocoordinates-rs/pull/153))
- *(geodesy)* [**breaking**] derive ellipsoids from the CRS, exact ECEF inverse, published Helmert bounds ([#152](https://github.com/justin13888/geocoordinates-rs/pull/152))
- *(china)* [**breaking**] make China datum types valid by construction with truthful inverse bounds ([#151](https://github.com/justin13888/geocoordinates-rs/pull/151))
- *(grids)* [**breaking**] make encoded grid codes valid by construction ([#147](https://github.com/justin13888/geocoordinates-rs/pull/147))
- *(angle)* [**breaking**] replace Dd with validated Dms/Ddm conversions and fix normalize_degrees ([#146](https://github.com/justin13888/geocoordinates-rs/pull/146))
- *(grids)* [**breaking**] make Utm and Ups valid by construction and wrap the inverse ([#145](https://github.com/justin13888/geocoordinates-rs/pull/145))
- *(coord)* [**breaking**] make Coordinate valid by construction ([#144](https://github.com/justin13888/geocoordinates-rs/pull/144))
- *(error)* [**breaking**] make the error taxonomy precise and mirror it one-to-one across FFI ([#143](https://github.com/justin13888/geocoordinates-rs/pull/143))
- *(examples)* split the flagship example into per-subsystem modules ([#142](https://github.com/justin13888/geocoordinates-rs/pull/142))
- *(ffi)* split lib.rs into per-subsystem modules ([#141](https://github.com/justin13888/geocoordinates-rs/pull/141))
- *(geodesy)* split geodesic.rs by model and remove the dead proj/height stubs ([#140](https://github.com/justin13888/geocoordinates-rs/pull/140))
- *(ffi)* run the FFI crate's Rust tests in CI ([#139](https://github.com/justin13888/geocoordinates-rs/pull/139))
- *(parse)* split the text parser below the file-length limit ([#138](https://github.com/justin13888/geocoordinates-rs/pull/138))
- pin current core-crate behaviour with characterization and property tests ([#134](https://github.com/justin13888/geocoordinates-rs/pull/134))
- correct stale and contradicted repository documentation ([#132](https://github.com/justin13888/geocoordinates-rs/pull/132))

## [0.14.2](https://github.com/justin13888/geocoordinates-rs/compare/v0.14.1...v0.14.2) - 2026-10-09

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

- *(ffi)* fail when committed Swift bindings are stale ([#87](https://github.com/justin13888/geocoordinates-rs/pull/87))
- add a file-length gate with a shrink-only baseline ([#92](https://github.com/justin13888/geocoordinates-rs/pull/92))
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
