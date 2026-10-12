# geocoordinates

Low-level geospatial coordinate primitives for Rust — China datums (GCJ-02/BD-09), angle encodings, coordinate parsing/formatting, and geodesy utilities. The crate abstracts only geo-related complexity, as primitives for higher-level libraries to consume.

> **Status:** The public API is `0.x` and may change between minor versions.
> **0.15.0 is the internal stabilization target** — the surface below is being
> audited subsystem by subsystem before the 1.0 freeze. See
> [STABILIZATION.md](STABILIZATION.md) for the ledger and what remains.

## API surface

The whole public surface, by subsystem. **Rust** and **FFI** track whether that
subsystem meets the five-point stabilization bar defined in
[STABILIZATION.md](STABILIZATION.md); a ⚠️ links to the specific open finding.

| Subsystem | Public surface | Rust | FFI |
|---|---|:--:|:--:|
| **Core types** | `Coordinate`, `Crs`, `Height`, `LatLon`, `Approx<T>`, `Error` / `Result`, `Fix`, `Accuracy`, `RawSource`, `AxisOrder`, `DatumAmbiguity`, `Confidence` | ✅ | ✅ |
| **Angles & units** | `Dms`, `Ddm`, `Hemisphere`, `Axis`, `wrap_longitude`, `clamp_latitude`, `normalize_degrees`, `Length`, `LengthUnit` | ✅ | ✅ |
| **China datums** | `Wgs84`, `Gcj02`, `Bd09` (valid by construction; exact forward, `_fast` / `_refined` inverses with honest bounds), `BaiduMercator`, `out_of_china` | ✅ | ✅ |
| **Runtime dispatch** | `convert` | ✅ | ✅ |
| **Format** | `format`, `format_fix`, `FormatOptions`, `Representation`, `SymbolStyle`, `HemisphereStyle`, `DecimalSeparator` | ⚠️ G16 | ✅ |
| **Parse** | `parse_coordinate`, `parse_coordinate_with`, `from_geo_uri`, `text::parse`, `text::parse_with`, `TextParseOptions` | ⚠️ G16 | ✅ |
| **Interchange & sensors** *(feature-gated)* | `from_geojson`, `from_wkt`, `from_gpx`, `from_kml`, `from_nmea_sentence` | ✅ | ✅ |
| **Geodesy** | `Ellipsoid`, `Ecef`, `Enu` / `Ned` / `Aer`, `geodesic_distance`, `haversine_distance`, `initial_bearing` / `final_bearing`, `destination`, `midpoint`, `intermediate`, `intersection`, rhumb and cross/along-track utilities, `Helmert`, `DatumTransform` | ⚠️ G9 | ✅ |
| **Projected grids** | `Utm`, `Ups`, `UtmHemisphere`, `Mgrs`, `MgrsPrecision`, `zone_for`, `central_meridian_deg` | ⚠️ G16 | ✅ |
| **Encoded grids** | `PlusCode`, `Geohash`, `Maidenhead` | ✅ | ✅ |
| **Discrete global grids** *(feature-gated)* | `H3Cell`, `S2CellId` | ✅ | ✅ |

Optional serde `Serialize` / `Deserialize` is derived on every public value,
option and id type under the `serde` feature.

**Deferred:** PROJ-backed EPSG conversion and geoid grid models, both blocked on
a system library or multi-megabyte external data. **Out of scope:** EXIF
extraction and polygon/line geometry operations.

## Full-surface example

The flagship example is executable documentation and a self-verifying downstream
API test. It uses embedded reference vectors and no network or external files:

```bash
cargo run --all-features --example flagship
cargo test --all-features --example flagship
```

It demonstrates every shipped Rust capability, including exact versus
approximate conversion semantics, axis order, antimeridian/pole behavior,
interchange and sensor ingestion, classic and China datums, geodesics, frames,
projected/encoded grids, serde round trips, and typed failures.

## Minimum supported Rust version

The core crate and most features build on **Rust 1.85** (`rust-version` in
[`Cargo.toml`](Cargo.toml)). Two features need a newer toolchain, because their
upstream dependencies do:

| Feature | Needs | Why |
|---|---|---|
| everything else (`serde`, `wkt`, `gpx`, `kml`, `nmea`, `s2`) | 1.85 | core `rust-version` |
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
capability parity is the design rule** — every public capability is meant to
have a canonical FFI form, including both H3 and S2 discrete global grids —
but the ⚠️ rows in the FFI column of the surface table above mark subsystems
that don't yet meet it (tracked in [STABILIZATION.md](STABILIZATION.md)).

The FFI surface is deliberately flatter than the idiomatic Rust API; the
[bindings README](crates/geocoordinates-ffi/README.md) states how each Rust idiom
is re-expressed. The published `geocoordinates` crate is unaffected — the
bindings build from their own `cdylib`/`staticlib` crate.

> Java is served by the Kotlin/JVM bindings (directly callable from Java), so it
> has no separate generated surface. TypeScript targets WebAssembly (browser /
> bundler) via [`ubrn`](https://github.com/jhugman/uniffi-bindgen-react-native);
> initialize it with `await uniffiInitAsync()` before use (see
> [`crates/geocoordinates-ffi/web/`](crates/geocoordinates-ffi/web/)).

Generate the bindings (needs the [Rust toolchain](https://rustup.rs) and
[mise](https://mise.jdx.dev)):

```bash
mise run bindings           # all languages -> bindings/<lang>/
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
that ecosystem's native tooling; Maven Central does not list them yet (see † below). See [PUBLISHING.md](PUBLISHING.md) for the release
workflows and registry setup.

| Language | Registry | Package | Install |
|---|---|---|---|
| Python | [PyPI](https://pypi.org/project/geocoordinates-rs/) | `geocoordinates-rs` | `pip install geocoordinates-rs` |
| TypeScript / WASM | [npm](https://www.npmjs.com/package/geocoordinates-rs) | `geocoordinates-rs` | `npm install geocoordinates-rs` |
| Kotlin / Java | [Maven Central](https://central.sonatype.com/artifact/io.github.justin13888/geocoordinates)&nbsp;† | `io.github.justin13888:geocoordinates` | add the coordinate to Gradle / Maven |
| Swift | [GitHub Releases](https://github.com/justin13888/geocoordinates-rs/releases) | `GeoCoordinates` | `.package(url: "https://github.com/justin13888/geocoordinates-rs", .upToNextMinor(from: "0.14.2"))` |

> † **Maven Central:** the JVM workflow uploads each release to the Central Portal with the
> configured token and GPG key, but no version has appeared on Maven Central yet — most
> likely because the build does not request automatic release, so each uploaded deployment
> waits in the portal for a manual release (see [PUBLISHING.md](PUBLISHING.md)).
> Until it is, build the JAR locally as PUBLISHING.md describes.

> The Swift requirement pins the minor version on purpose: while the API is `0.x`, a minor
> release may break it, and a plain `from:` would accept every later `0.x`.

> The native Rust crate is published separately to
> [crates.io](https://crates.io/crates/geocoordinates) (`cargo add geocoordinates`).

## Prerequisites

- [Rust (rustup)](https://rustup.rs) — toolchain (pinned via `rust-toolchain.toml`)
- [mise](https://mise.jdx.dev) — tool manager and task runner
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — code coverage

Everything else is pinned in [`.mise.toml`](.mise.toml) and provisioned for you:

```bash
mise install
```

That installs [hk](https://hk.jdx.dev) (git hooks) and wires the hooks into this clone.
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
| `mise run file-length-check`| Files over 654 lines fail unless in `.file-length-baseline` (shrink-only) |
| `mise run ffi-check`        | Clippy on the FFI crate (the mirror gates every release)             |
| `mise run ffi-test`         | Tests for the FFI crate (`mise run test` covers only the core crate) |
| `mise run doc`              | Rustdoc build (CI denies its warnings)                               |
| `mise run msrv`             | Build and test on the declared MSRV and each feature exception       |
| `mise run coverage`         | Coverage report                                                      |
| `mise run mutants`          | Mutation tests on the lines changed vs `origin/master`               |

Other common tasks:

| Command                 | Description                           |
| ----------------------- | ------------------------------------- |
| `cargo build`           | Build the crate                       |
| `mise run check`        | Core gates: fmt-check + lint + test   |
| `mise run bindings`     | Generate FFI bindings (all languages) |

Each gate is defined once, in [`.mise.toml`](.mise.toml), and CI and the git hooks
both invoke these same tasks, so no gate definition needs syncing by hand. Tool
version pins are the exception: the mise version is repeated in every workflow that
installs it, and the hk version in both `.mise.toml` and `hk.pkl`, so bump each set
together.

### Git Hooks

This project uses [hk](https://hk.jdx.dev), configured in [`hk.pkl`](hk.pkl). `mise install`
installs the hooks; `hk install` re-runs that step on its own.

Pre-commit auto-fixes formatting and linting on staged Rust files. The commit-msg hook
validates each message against [Conventional Commits](https://www.conventionalcommits.org)
(via `convco`). Pre-push lints the pushed commits and runs format checks, the file-length
check, Clippy (core and FFI crates), tests, a coverage report, and mutation tests on the
changed lines.

Run them on demand without committing:

```bash
hk check --all           # fmt-check + file-length-check + lint + ffi-check + test
hk fix --all             # apply the pre-commit fixes
hk run pre-push          # the full pre-push gate set
```

Set `HK=0` to skip hooks for a single git invocation.

### CI/CD

GitHub Actions runs these workflows:

- **CI** (pushes to `master` and pull requests): format check, file-length check, Clippy, tests, the MSRV build, a rustdoc build with warnings denied, and a coverage report; on pull requests only, mutation tests on the changed lines and a lint of the commit messages against [Conventional Commits](https://www.conventionalcommits.org) (via `convco`).
- **FFI** (pushes to `master` and pull requests): Clippy on the FFI crate, the bindings `cdylib` build, Python/Kotlin/Swift binding generation, a check that the committed Swift bindings are current, a WASM/TypeScript build, Python and JVM (Kotlin/Java) smoke tests, and a Swift package build on macOS.
- **Full-tree mutation tests** (weekly, and on demand): mutates the whole tree and reports survivors without gating anything.
- **Release dry runs**: each binding release workflow also runs as a non-publishing dry run on pull requests that touch its pipeline and weekly (see [PUBLISHING.md](PUBLISHING.md)).

CI and FFI cancel superseded in-flight runs when a PR is updated.

### Releases

Releases are automated with [release-plz](https://release-plz.dev) and driven by
[Conventional Commits](https://www.conventionalcommits.org). As commits land on `master`,
release-plz maintains a **release PR** that bumps the version and updates
[`CHANGELOG.md`](CHANGELOG.md). Merging that PR cuts the release: it tags `vX.Y.Z`, creates
a GitHub release, and publishes to [crates.io](https://crates.io/crates/geocoordinates).

To honor the staged [stabilization plan](STABILIZATION.md), the release PR is merged at milestone boundaries
rather than on every commit. Publishing uses crates.io
[Trusted Publishing](https://crates.io/docs/trusted-publishing) (OIDC) — no API token is
stored in the repository.

### Code Coverage

This project uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) for LLVM-based code coverage. No minimum threshold is enforced yet; one is added once the API stabilizes.

```bash
mise run coverage
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
