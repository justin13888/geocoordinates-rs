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
| **Core types** | `Coordinate`, `Crs`, `Height`, `LatLon`, `Approx<T>`, `Error` / `Result`, `Fix`, `Accuracy`, `RawSource`, `AxisOrder`, `DatumAmbiguity`, `Confidence` | ✅ | ⚠️ G4 |
| **Angles & units** | `Dd`, `Dms`, `Ddm`, `Hemisphere`, `Axis`, `wrap_longitude`, `clamp_latitude`, `normalize_degrees`, `Length`, `LengthUnit` | ✅ | ⚠️ G5 |
| **China datums** | `Wgs84`, `Gcj02`, `Bd09` (exact forward, `_fast` / `_refined` inverses), `BaiduMercator`, `out_of_china` | ✅ | ⚠️ G3, G5 |
| **Runtime dispatch** | `convert`, `can_convert` | ✅ | ✅ |
| **Format** | `format`, `format_fix`, `FormatOptions`, `Representation`, `SymbolStyle`, `HemisphereStyle` | ⚠️ G16 | ✅ |
| **Parse** | `parse_coordinate`, `from_geo_uri`, `text::parse`, `text::parse_with`, `TextParseOptions` | ⚠️ G16 | ✅ |
| **Interchange & sensors** *(feature-gated)* | `from_geojson`, `from_wkt`, `from_gpx`, `from_kml`, `from_nmea_sentence` | ✅ | ✅ |
| **Geodesy** | `Ellipsoid`, `Ecef`, `Enu` / `Ned` / `Aer`, `geodesic_distance`, `haversine_distance`, `initial_bearing` / `final_bearing`, `destination`, `midpoint`, `intermediate`, `intersection`, rhumb and cross/along-track utilities, `Helmert`, `DatumTransform` | ⚠️ G9 | ⚠️ G5 |
| **Projected grids** | `Utm`, `Ups`, `Mgrs`, `zone_for`, `central_meridian_deg` | ⚠️ G10 | ⚠️ G6 |
| **Encoded grids** | `PlusCode`, `Geohash`, `Maidenhead` | ⚠️ G7 | ✅ |
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

## Language bindings (FFI)

The API is exposed to **Python, Kotlin, Swift, and TypeScript** with full
capability parity via [UniFFI](https://mozilla.github.io/uniffi-rs/), generated
from the separate `geocoordinates-ffi` crate (Java consumes the Kotlin/JVM
artifact directly). The bindings track the released surface and gate each
release. Every public capability has a canonical FFI form, including both H3
and S2 discrete global grids.

Because the Rust API is idiomatic, the FFI surface is deliberately flattened:
generics (`Approx<T>`), traits, `Deref`, and operator overloads do not cross the
boundary. Their equivalents are concrete records (e.g. `ApproxWgs84 { lat, lon,
max_error_m }`) and free functions returning primitives (distance is returned in
meters). Approximate inverses keep their `_fast` / `_refined` names and carry
`max_error_m`. The published `geocoordinates` crate is unaffected — the bindings
build from their own `cdylib`/`staticlib` crate.

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

Each language's bindings are published to its registry on every `v*` release, using
that ecosystem's native tooling. See [PUBLISHING.md](PUBLISHING.md) for the release
workflows and registry setup.

| Language | Registry | Package | Install |
|---|---|---|---|
| Python | [PyPI](https://pypi.org/project/geocoordinates-rs/) | `geocoordinates-rs` | `pip install geocoordinates-rs` |
| TypeScript / WASM | [npm](https://www.npmjs.com/package/geocoordinates-rs) | `geocoordinates-rs` | `npm install geocoordinates-rs` |
| Kotlin / Java | [Maven Central](https://central.sonatype.com/artifact/io.github.justin13888/geocoordinates)&nbsp;† | `io.github.justin13888:geocoordinates` | add the coordinate to Gradle / Maven |
| Swift | [GitHub Releases](https://github.com/justin13888/geocoordinates-rs/releases) | `GeoCoordinates` | `.package(url: "https://github.com/justin13888/geocoordinates-rs", from: "0.1.1")` |

> † **Maven Central** publishing is pending registry credentials — the JVM workflow builds the
> artifact but skips the upload until the Central Portal token and GPG key are configured (see
> [PUBLISHING.md](PUBLISHING.md)).

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

`mise tasks` lists everything; the common ones are:

| Command                 | Description                           |
| ----------------------- | ------------------------------------- |
| `cargo build`           | Build the crate                       |
| `mise run test`         | Run tests                             |
| `mise run fmt`          | Format code                           |
| `mise run lint-fix`     | Lint and auto-fix                     |
| `mise run coverage`     | Report code coverage                  |
| `mise run check`        | fmt-check + lint + test (CI parity)   |
| `mise run bindings`     | Generate FFI bindings (all languages) |

Each gate is defined once, in [`.mise.toml`](.mise.toml). CI and the git hooks both
invoke these same tasks, so there is nothing to keep in sync by hand.

### Git Hooks

This project uses [hk](https://hk.jdx.dev), configured in [`hk.pkl`](hk.pkl). `mise install`
installs the hooks; `hk install` re-runs that step on its own.

Pre-commit auto-fixes formatting and linting on staged Rust files. The commit-msg hook
validates each message against [Conventional Commits](https://www.conventionalcommits.org)
(via `convco`). Pre-push lints the pushed commits and runs format checks, Clippy, tests, a
coverage report, and mutation tests on the changed lines.

Run them on demand without committing:

```bash
hk check --all           # fmt-check + lint + test
hk fix --all             # apply the pre-commit fixes
hk run pre-push          # the full pre-push gate set
```

Set `HK=0` to skip hooks for a single git invocation.

### CI/CD

GitHub Actions runs format checks, Clippy, tests, and a coverage report on pushes to `master` and pull requests, and lints PR commit messages against [Conventional Commits](https://www.conventionalcommits.org) (via `convco`). A separate FFI workflow builds the bindings `cdylib`, generates bindings for all four languages, and runs a Python smoke test. Both workflows cancel superseded in-flight runs when a PR is updated.

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

This project uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) for LLVM-based code coverage. No minimum threshold is enforced yet — tests and a threshold are added once the API stabilizes.

```bash
mise run coverage
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
