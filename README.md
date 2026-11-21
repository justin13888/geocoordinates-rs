# geocoordinates

Low-level geospatial coordinate primitives for Rust — China datums (GCJ-02/BD-09), angle encodings, coordinate parsing/formatting, and geodesy utilities. The crate abstracts only geo-related complexity, as primitives for higher-level libraries to consume.

> **Status:** The public API is `0.x` and may change between minor versions.
> **0.15.0 is the stabilization release**; what that claims, and the evidence
> for it, is in
> [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md#what-stabilized-means).

## API surface

The whole public surface, by subsystem. A ✅ in **Rust** or **FFI** marks the
subsystem as stabilized on that side; the stabilization bar, and the evidence
that each subsystem meets it, are in
[STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md#what-stabilized-means).

Every public item has exactly one path (plus its `prelude` entry, if any): the core
types sit at the crate root, and every other item sits flat in the module the
table names, e.g. `geocoordinates::china::Wgs84` or
`geocoordinates::grids::Utm`. `mise run public-api-check` enforces it. The
`prelude` holds only nine types (`Coordinate`, `Crs`, `Height`, `LatLon`,
`Approx`, `Length`, `LengthUnit`, `Error`, and `fix::Fix`): the core types
less `ParseErrorKind` and `Result`, plus `Fix`; functions and
feature-gated items are imported from their modules.

| Subsystem | Public surface | Rust | FFI |
|---|---|:--:|:--:|
| **Core types** (crate root) | `Coordinate`, `Crs`, `Height`, `LatLon`, `Approx<T>`, `Error` / `Result`, `ParseErrorKind`, `Length`, `LengthUnit` | ✅ | ✅ |
| **Fix metadata** (`fix`) | `Fix`, `Accuracy`, `AccuracyKind`, `RawSource`, `AxisOrder`, `DatumAmbiguity`, `Confidence` | ✅ | ✅ |
| **Angles** (`angle`) | `Dms`, `Ddm`, `Hemisphere`, `Axis`, `wrap_longitude`, `clamp_latitude`, `normalize_degrees` | ✅ | ✅ |
| **China datums** (`china`) | `Wgs84`, `Gcj02`, `Bd09` (valid by construction; exact forward, `_fast` / `_refined` inverses with honest bounds), `BaiduMercator`, `Wgs84::is_outside_china` / `Gcj02::is_outside_china` | ✅ | ✅ |
| **Runtime dispatch** (crate root) | `Coordinate::try_to_crs` (FFI: `convert`) | ✅ | ✅ |
| **Format** (`format`) | `format`, `format_fix`, `FormatOptions`, `Representation`, `SymbolStyle`, `HemisphereStyle`, `DecimalSeparator` | ✅ | ✅ |
| **Parse** (`parse`) | `parse_coordinate`, `parse_coordinate_with`, `ParseOptions` (with `for_format` for the round trip), `GridKind`, `parse_geo_uri`, `parse_text`, `parse_text_with` | ✅ | ✅ |
| **Interchange & sensors** *(feature-gated; `parse::interchange`, `parse::sensors`)* | `parse_geojson`, `parse_wkt`, `parse_gpx`, `parse_kml`, `parse_nmea_sentence` | ✅ | ✅ |
| **Geodesy** (`geodesy`) | `Ellipsoid`, `Ecef`, `Enu` / `Ned` / `Aer` (on any classic datum the origin and target share), `geodesic_distance`, `haversine_distance`, `initial_bearing` / `final_bearing`, `destination`, `midpoint`, `intermediate`, `intersection` / `GeodesicIntersection`, `cross_track_distance` / `along_track_distance` (on each datum's own ellipsoid), `intersection_spherical` / `SphericalIntersection`, `cross_track_distance_spherical` / `along_track_distance_spherical` (on the mean sphere), `rhumb_distance` / `rhumb_bearing` / `rhumb_destination` (on each datum's own ellipsoid), `Helmert`, `DatumTransform`, `GeoBounds` (antimeridian-aware box) | ✅ | ✅ |
| **Projected grids** (`grids`) | `Utm` (with `Utm::zone_for` / `Utm::central_meridian_deg`; FFI: `utm_zone_for` / `utm_central_meridian_deg`), `Ups`, `UtmHemisphere`, `Mgrs`, `MgrsPrecision` | ✅ | ✅ |
| **Encoded grids** (`grids`) | `PlusCode`, `Geohash`, `Maidenhead` | ✅ | ✅ |
| **Discrete global grids** *(feature-gated; `dgg`)* | `H3Cell`, `S2CellId` | ✅ | ✅ |
| **Spatial index** *(feature-gated; `index`)* | `SpatialIndex`, `Neighbor` | ✅ | ✅ |
| **EXIF / XMP GPS tags** *(feature-gated; `exif`)* | `CaptureGeo::from_exif`, `CaptureGeo::from_exif_with`, `CaptureGeo::from_xmp`, `CaptureGeo::from_xmp_with`, `ExifGps`, `ExifRational`, `XmpGps`, `CaptureGeo`, `NorthBearing`, `NorthReference`, `ExifOptions`, `NullIslandPolicy`, `AuxiliaryTagPolicy` | ✅ | ✅ |
| **Clustering** *(feature-gated; `cluster`)* | `centroid`, `dbscan`, `Dbscan`, `PointLabel`, `Cluster`, `ClusterIndex`, `ClusterIndexOptions`, `MapCluster`, `MAX_ZOOM`, `bucket_by_geohash`, `bucket_by_h3`, `GridBucket`, `segment_visits`, `segment_trips`, `Visit`, `Trip` | ✅ | ✅ |
| **Reverse geocoding** *(feature-gated; `reverse`, `reverse-bundled`)* | `ReverseGeocoder`, `Gazetteer`, `Place`, `FeatureClass`, `ReverseOptions` | ✅ | ✅ (no bundled dataset: load a gazetteer file) |
| **Map-view math** *(feature-gated; `map`, GeoJSON layers also `geojson`)* | `WebMercator`, `WorldPixel`, `MAX_LATITUDE`, `Tile`, `MAX_TILE_ZOOM`, `tiles_covering`, `Viewport`, `Camera`, `fit_bounds`, `viewport_bounds`, `clusters_in_view`, `points_to_geojson`, `clusters_to_geojson`, `DisplayDatum`, `display_coordinate`, `tap_to_wgs84` | ✅ | ✅ (`clusters_to_geojson` as `clusters_in_view_geojson`) |
| **Geotagging** *(feature-gated; `geotag`)* | `Track`, `GeotagOptions`, `Interpolation`, `estimate_offset` | ✅ | ✅ (`estimate_offset` as `estimate_offset_ms`) |
| **Location privacy** *(feature-gated; `privacy`, H3 / S2 snapping also `h3` / `s2`)* | `coarsen`, `Precision`, `SnapGrid`, `ExclusionZone`, `ZoneAction`, `apply` | ✅ | ✅ (`apply` as `apply_exclusion_zones`) |

Optional serde `Serialize` / `Deserialize` is derived on every public value,
option and id type under the `serde` feature.

**Known defects:** the [audit ledger](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md#audit-ledger)
records every finding of the stabilization and pre-1.0 audits and whether it is
still open.

**Deferred:** PROJ-backed EPSG conversion and geoid grid models, both blocked on
a system library or multi-megabyte external data.

**Out of scope:** container/byte parsing of JPEG, TIFF and HEIF (use an EXIF reader
and pass its tag values in), polygon/line geometry operations such as area,
point-in-polygon, simplification and hull (use [`geo`](https://crates.io/crates/geo)),
and rendering static map images.

## Flagship example

The [flagship example](https://github.com/justin13888/geocoordinates-rs/tree/master/examples/flagship)
is executable documentation and a self-verifying downstream API test. It uses
embedded reference vectors and no network or external files. It is not shipped in
the published crate, so run it from a clone of the
[repository](https://github.com/justin13888/geocoordinates-rs):

```bash
cargo run --all-features --example flagship
cargo test --all-features --example flagship
```

It demonstrates the eight core capability groups, including exact versus
approximate conversion semantics, axis order, antimeridian/pole behavior,
interchange and sensor ingestion, classic and China datums, geodesics, frames,
projected/encoded grids, serde round trips, and typed failures. It does not
exercise the feature-gated photo-library subsystems (`index`, `exif`, `cluster`,
`reverse`, `map`, `geotag`, `privacy`); their per-feature test tasks below do.

## Minimum supported Rust version

The core crate and most features build on **Rust 1.85** (`rust-version` in
[`Cargo.toml`](https://github.com/justin13888/geocoordinates-rs/blob/master/Cargo.toml)). Two features need a newer toolchain, because their
upstream dependencies do:

| Feature | Needs | Why |
|---|---|---|
| every other feature | 1.85 | core `rust-version` |
| `geojson` | 1.88 | geojson 1.0 uses `if let` chains |
| `h3` | 1.94 | h3o 0.10 calls `f64::mul_add` in a `const fn` |

Enabling `geojson` or `h3` therefore raises your MSRV to the version shown. The
exceptions are recorded in `[package.metadata.feature-rust-version]` in
`Cargo.toml`, and `mise run msrv` (a CI job) tests the core on 1.85 and each
exception on its own toolchain against the committed `Cargo.lock`.

## Language bindings (FFI)

The API is exposed to **Python, Kotlin, Swift, and TypeScript** via
[UniFFI](https://mozilla.github.io/uniffi-rs/), generated from the separate
`geocoordinates-ffi` crate (Java consumes the Kotlin/JVM artifact directly).
The bindings track the released surface and gate each release. **Full
capability parity is the design rule**: every public capability has one
canonical FFI form, including both H3 and S2 discrete global grids, and the FFI
column of the surface table above records it subsystem by subsystem.

The FFI surface is deliberately flatter than the idiomatic Rust API; the
[bindings README](https://github.com/justin13888/geocoordinates-rs/blob/master/crates/geocoordinates-ffi/README.md#how-each-rust-idiom-is-re-expressed) states how each Rust idiom
is re-expressed, and its [naming section](https://github.com/justin13888/geocoordinates-rs/blob/master/crates/geocoordinates-ffi/README.md#naming)
states the one naming scheme every export follows. The published `geocoordinates` crate is unaffected — the
bindings build from their own `cdylib`/`staticlib` crate.

> Java is served by the Kotlin/JVM bindings (directly callable from Java), so it
> has no separate generated surface. TypeScript targets WebAssembly (browser,
> bundler, or plain Node) via [`ubrn`](https://github.com/jhugman/uniffi-bindgen-react-native);
> initialize it with `await uniffiInitAsync()` before use (see
> [`crates/geocoordinates-ffi/web/`](https://github.com/justin13888/geocoordinates-rs/tree/master/crates/geocoordinates-ffi/web/)).

Generate the bindings (needs the [Rust toolchain](https://rustup.rs) and
[mise](https://mise.jdx.dev); the TypeScript leg, `mise run bindings-web`, also
needs the `wasm32-unknown-unknown` target,
[`wasm-pack`](https://rustwasm.github.io/wasm-pack/), Node.js with npm, and Perl):

```bash
mise run bindings           # Python, Kotlin, Swift -> bindings/<lang>/;
                            # TypeScript -> crates/geocoordinates-ffi/web/dist/
mise run bindings-python    # a single language
```

Python example:

```python
import geocoordinates_ffi as gc

gcj = gc.wgs84_to_gcj02(gc.Wgs84(lat=39.915, lon=116.404))
wgs = gc.gcj02_to_wgs84_refined(gcj)          # approximate inverse
print(wgs.lat, wgs.lon, "±", wgs.max_error_m, "m")
```

### Published packages

Each language's bindings are uploaded to its registry on every `v*` release, using
that ecosystem's native tooling (for Maven Central, see † below). See [PUBLISHING.md](https://github.com/justin13888/geocoordinates-rs/blob/master/PUBLISHING.md) for the release
workflows and registry setup.

| Language | Registry | Package | Install |
|---|---|---|---|
| Python | [PyPI](https://pypi.org/project/geocoordinates-rs/) | `geocoordinates-rs` | `pip install geocoordinates-rs` |
| TypeScript / WASM | [npm](https://www.npmjs.com/package/geocoordinates-rs) | `geocoordinates-rs` | `npm install geocoordinates-rs` |
| Kotlin / Java | [Maven Central](https://central.sonatype.com/artifact/io.github.justin13888/geocoordinates)&nbsp;† | `io.github.justin13888:geocoordinates` | add the coordinate to Gradle / Maven |
| Swift | [GitHub Releases](https://github.com/justin13888/geocoordinates-rs/releases) | `GeoCoordinates` | `.package(url: "https://github.com/justin13888/geocoordinates-rs", .upToNextMinor(from: "0.15.0"))` |

> † **Maven Central:** the JVM workflow uploads each release to the Central Portal with the
> configured token and GPG key and requests automatic release, so the portal publishes it to
> Maven Central once it passes validation. Releases v0.14.2 and earlier were uploaded without
> automatic release and are not on Maven Central (see [PUBLISHING.md](https://github.com/justin13888/geocoordinates-rs/blob/master/PUBLISHING.md));
> for a version the registry does not list, build the JAR locally as PUBLISHING.md describes.

> The Swift requirement pins the minor version on purpose: while the API is `0.x`, a minor
> release may break it, and a plain `from:` would accept every later `0.x`.

> The native Rust crate is published separately to
> [crates.io](https://crates.io/crates/geocoordinates) (`cargo add geocoordinates`).

## Prerequisites

- [Rust (rustup)](https://rustup.rs) — toolchain (pinned via `rust-toolchain.toml`)
- [mise](https://mise.jdx.dev) — tool manager and task runner
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — code coverage
- Python 3.11 or later as `python3` — the gate scripts and `version-check` (which reads TOML with `tomllib`)

Everything else is pinned in [`.mise.toml`](https://github.com/justin13888/geocoordinates-rs/blob/master/.mise.toml) and provisioned for you:

```bash
mise install
```

That installs [hk](https://hk.jdx.dev) (git hooks) and wires the hooks into this clone.
[cargo-hack](https://github.com/taiki-e/cargo-hack),
[cargo-public-api](https://github.com/cargo-public-api/cargo-public-api),
[cargo-mutants](https://github.com/sourcefrog/cargo-mutants) and
[convco](https://convco.github.io) are declared on the tasks that use them, so they install
on first use rather than up front. `.mise.toml` is inert without mise, so a system-installed
toolchain is unaffected.

## Quick Start

```bash
cargo build
mise run check   # fmt-check + clippy + tests
```

## Development

`mise tasks` lists everything. The gates a change must pass:

| Command                     | Gate                                                                 |
| --------------------------- | -------------------------------------------------------------------- |
| `mise run test`             | Tests, doctests, and the flagship example (all features)             |
| `mise run fmt-check`        | Formatting (`mise run fmt` fixes it)                                 |
| `mise run lint`             | Clippy with warnings denied (`mise run lint-fix` fixes what it can)  |
| `mise run file-length-check`| Files over `max_file_lines` (in `.mise.toml`) fail unless in `.file-length-baseline` (shrink-only) |
| `mise run ffi-check`        | Clippy on the FFI crate (the mirror gates every release)             |
| `mise run ffi-test`         | Tests for the FFI crate (`mise run test` covers only the core crate) |
| `mise run version-check`    | Every crate and binding manifest ships the workspace version (`mise run version-sync` fixes npm) |
| `mise run scripts-test`     | Fixture tests for `scripts/check_public_paths.py` and the `file-length-check`, `coverage-ratchet` and `version-check` tasks |
| `mise run doc`              | Rustdoc build (CI denies its warnings)                               |
| `mise run public-api-check` | Every public item has one path besides any prelude entry, no fewer items than the floor in `scripts/check_public_paths.py`, and the surface matches the snapshots in `public-api/` (pinned nightly rustdoc JSON, `cargo-public-api`) |
| `mise run features`         | Library check with no features and each feature alone (cargo-hack); tests with no features |
| `mise run index-test`       | Tests and doctests for the `index` feature alone; compiles its benchmark |
| `mise run exif-test`        | Tests and doctests for the `exif` feature alone                      |
| `mise run cluster-test`     | Tests and doctests for the `cluster` feature alone; compiles its benchmark |
| `mise run reverse-test`     | Tests and doctests for `reverse` and `reverse-bundled` alone and for the gazetteer build tool; compiles the benchmark |
| `mise run map-test`         | Tests and doctests for the `map` feature alone, and with `geojson` for its layers |
| `mise run geotag-test`      | Tests and doctests for the `geotag` feature alone, and with `gpx` for its golden track |
| `mise run privacy-test`     | Tests and doctests for the `privacy` feature alone, and with `h3`, `s2` and `serde` |
| `mise run msrv`             | Build and test on the declared MSRV and each feature exception       |
| `mise run coverage`         | Coverage report; fails under the line floor in `.coverage-baseline`  |
| `mise run coverage-ratchet` | Fails if the floor in `.coverage-baseline` falls below `origin/master`'s |
| `mise run mutants`          | Mutation tests on the lines changed vs `origin/master`               |

Other common tasks:

| Command                 | Description                           |
| ----------------------- | ------------------------------------- |
| `cargo build`           | Build the crate                       |
| `mise run check`        | Core gates: fmt-check + lint + test   |
| `mise run public-api-snapshot` | Regenerate the public API snapshots in `public-api/` |
| `mise run bindings`     | Generate FFI bindings (all languages) |

A pull request that changes the public API runs `mise run public-api-snapshot`
and commits the regenerated `public-api/` files in the same diff, so every
intended change to the surface is a reviewable diff and an unintended one fails
`public-api-check`. `public-api/geocoordinates/` is the surface with every
feature on and `public-api/geocoordinates-default/` the surface with none, one
file per public module. A change that removes public items also lowers
`MIN_ITEMS` in `scripts/check_public_paths.py`.

Each gate is defined once, in [`.mise.toml`](https://github.com/justin13888/geocoordinates-rs/blob/master/.mise.toml), and CI and the git hooks
both invoke these same tasks, so no gate definition needs syncing by hand. Tool
version pins are the exception: the mise version is repeated in every workflow that
installs it, and the hk version in `.mise.toml` and twice in `hk.pkl` (its `amends`
URL and `min_hk_version`), so bump each set together.

### Git Hooks

This project uses [hk](https://hk.jdx.dev), configured in [`hk.pkl`](https://github.com/justin13888/geocoordinates-rs/blob/master/hk.pkl). `mise install`
installs the hooks; `hk install` re-runs that step on its own.

Pre-commit auto-fixes formatting and linting on staged Rust files. The commit-msg hook
validates each message against [Conventional Commits](https://www.conventionalcommits.org)
(via `convco`). Pre-push lints the pushed commits and runs format checks, the file-length
check, Clippy (core and FFI crates), tests, and the coverage ratchet. Those are the fast gates;
every other gate in the [Development](#development) table runs in CI.

Run them on demand without committing:

```bash
hk check --all           # fmt-check + file-length-check + lint + ffi-check + test
hk fix --all             # apply the pre-commit fixes
hk run pre-push          # the full pre-push gate set
```

Set `HK=0` to skip hooks for a single git invocation.

### CI/CD

GitHub Actions runs these workflows:

- **CI** (pushes to `master` and pull requests): format check, a check that every manifest ships the workspace version, the gate scripts' fixture tests, file-length check, Clippy, tests, a feature matrix that checks the library with no features and with each feature alone and then tests it with no features, a per-feature test job for each of `index`, `exif`, `cluster`, `reverse` / `reverse-bundled`, `map`, `geotag` and `privacy`, the MSRV build, a rustdoc build with warnings denied, a check that every public item has one canonical path and that the surface matches the `public-api/` snapshots, and a coverage report that fails under the line-coverage floor, whose ratchet lets it only rise; on pull requests only, mutation tests on the changed lines and a lint of the commit messages against [Conventional Commits](https://www.conventionalcommits.org) (via `convco`).
- **FFI** (pushes to `master` and pull requests): Clippy and the Rust tests on the FFI crate, the bindings `cdylib` build, Python/Kotlin/Swift binding generation, a check that the committed Swift bindings are current, a WASM/TypeScript build with Node tests against the built wasm, the Python smoke test, JVM (Kotlin and Java) tests, and a Swift package build plus `swift test` on macOS.
- **Full-tree mutation tests** (weekly, and on demand): mutates the whole tree and reports survivors without gating anything.
- **Release dry runs**: each binding release workflow also runs as a non-publishing dry run on pull requests that touch its pipeline and weekly (see [PUBLISHING.md](https://github.com/justin13888/geocoordinates-rs/blob/master/PUBLISHING.md)).

CI and FFI cancel superseded in-flight runs when a PR is updated.

### Releases

Releases are automated with [release-plz](https://release-plz.dev) and driven by
[Conventional Commits](https://www.conventionalcommits.org). As commits land on `master`,
release-plz maintains a **release PR** that bumps the version and updates
[`CHANGELOG.md`](https://github.com/justin13888/geocoordinates-rs/blob/master/CHANGELOG.md). Merging that PR cuts the release: it tags `vX.Y.Z`, creates
a GitHub release, and publishes to [crates.io](https://crates.io/crates/geocoordinates).

To honor the staged [stabilization plan](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md), the release PR is merged at milestone boundaries
rather than on every commit. Publishing uses crates.io
[Trusted Publishing](https://crates.io/docs/trusted-publishing) (OIDC) — no API token is
stored in the repository.

### Code Coverage

This project uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) for LLVM-based code coverage. Total line coverage of the core crate must stay at or above the
floor recorded in [`.coverage-baseline`](https://github.com/justin13888/geocoordinates-rs/blob/master/.coverage-baseline), in whole percent; `mise run coverage`
and CI fail below it. The floor may only rise: `mise run coverage-ratchet` fails when it is lower
than the floor on the base ref, so lost coverage is restored with tests rather than by lowering the
number. When coverage passes the next whole percent, raise the floor to the measured value rounded
down.

```bash
mise run coverage
mise run coverage-ratchet
```

## License

Licensed under either of [MIT](https://github.com/justin13888/geocoordinates-rs/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/justin13888/geocoordinates-rs/blob/master/LICENSE-APACHE) at your option.

The gazetteer data the `reverse-bundled` feature compiles in
(`src/reverse/data/geonames-cities1000.gaz`), and any gazetteer file built with
`geocoordinates-gazetteer`, is derived from [GeoNames](https://www.geonames.org/) and
licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), not the
code's licence. An application that shows places from it must credit GeoNames;
`Gazetteer::attribution` returns the credit line to ship.

To regenerate the bundled file, download `cities1000.zip` (unzipped to
`cities1000.txt`), `admin1CodesASCII.txt` and `admin2Codes.txt` from the
[GeoNames dump](https://download.geonames.org/export/dump/) and run `cargo run -p geocoordinates-gazetteer -- --cities cities1000.txt --admin1
admin1CodesASCII.txt --admin2 admin2Codes.txt --source-date YYYY-MM-DD --out
src/reverse/data/geonames-cities1000.gaz`, with the dump's date as `--source-date`.
