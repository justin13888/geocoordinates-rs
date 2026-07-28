# geocoordinates

Low-level geospatial coordinate primitives for Rust — China datums (GCJ-02/BD-09), angle encodings, coordinate parsing/formatting, and geodesy utilities. The crate abstracts only geo-related complexity, as primitives for higher-level libraries to consume.

> **Status:** Early development, released incrementally. The public API is `0.x` and may change between minor versions. See [ROADMAP.md](ROADMAP.md) for the planned release order.

## What's available now

- **Coordinate primitives:** datum-tagged coordinates, explicit height surfaces,
  angle encodings, length units, observation metadata, typed errors, and
  approximation bounds.
- **China datums:** WGS-84 ↔ GCJ-02 ↔ BD-09, including Baidu Web Mercator and
  explicit fast/refined inverse accuracy.
- **Presentation and ingestion:** DD/DMS/DDM/Plus Code formatting, tolerant text
  parsing, `geo:` URIs, GeoJSON, WKT, GPX, KML, and NMEA 0183.
- **Geodesy and datums:** Karney geodesics, spherical/rhumb/track utilities,
  ECEF and ENU/NED/AER frames, Helmert transforms, and runtime CRS dispatch.
- **Projected and encoded grids:** UTM, UPS, MGRS, Plus Code, Geohash,
  Maidenhead, H3, and S2.
- Optional serde support for public value/configuration types.

PROJ-backed EPSG conversion and geoid grid models remain deferred because they
require system libraries or external data. EXIF extraction and polygon/line
geometry operations are intentionally out of scope.

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
release. Every public capability has a canonical FFI form except S2, whose
dependency is not wasm-compatible; H3 is the FFI-exposed discrete global grid.

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
[just](https://github.com/casey/just)):

```bash
just bindings           # all languages -> bindings/<lang>/
just bindings-python    # a single language
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
- [just](https://github.com/casey/just) — command runner
- [Lefthook](https://github.com/evilmartians/lefthook) — git hooks manager
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — code coverage
- [convco](https://convco.github.io) — Conventional Commits linter

Dev tools (`just`, `cargo-mutants`, `convco`) are pinned in [`.mise.toml`](.mise.toml); run
`mise install` to provision them, or install them yourself — `.mise.toml` is inert without mise.

## Quick Start

```bash
cargo build
just check   # fmt-check + clippy + tests
```

## Development

| Command          | Description                       |
| ---------------- | -------------------------------- |
| `cargo build`    | Build the crate                  |
| `just test`      | Run tests                        |
| `just fmt`       | Format code                      |
| `just lint-fix`  | Lint and auto-fix                |
| `just coverage`  | Report code coverage             |
| `just check`     | fmt-check + lint + test (CI parity) |
| `just bindings`  | Generate FFI bindings (all languages) |

### Git Hooks

This project uses [Lefthook](https://github.com/evilmartians/lefthook). Pre-commit hooks auto-fix formatting and linting on staged files. The commit-msg hook validates each message against [Conventional Commits](https://www.conventionalcommits.org) (via `convco`). Pre-push hooks lint the pushed commits and run format checks, Clippy, tests, and a coverage report.

### CI/CD

GitHub Actions runs format checks, Clippy, tests, and a coverage report on pushes to `master` and pull requests, and lints PR commit messages against [Conventional Commits](https://www.conventionalcommits.org) (via `convco`). A separate FFI workflow builds the bindings `cdylib`, generates bindings for all four languages, and runs a Python smoke test. Both workflows cancel superseded in-flight runs when a PR is updated.

### Releases

Releases are automated with [release-plz](https://release-plz.dev) and driven by
[Conventional Commits](https://www.conventionalcommits.org). As commits land on `master`,
release-plz maintains a **release PR** that bumps the version and updates
[`CHANGELOG.md`](CHANGELOG.md). Merging that PR cuts the release: it tags `vX.Y.Z`, creates
a GitHub release, and publishes to [crates.io](https://crates.io/crates/geocoordinates).

To honor the staged [ROADMAP](ROADMAP.md), the release PR is merged at milestone boundaries
rather than on every commit. Publishing uses crates.io
[Trusted Publishing](https://crates.io/docs/trusted-publishing) (OIDC) — no API token is
stored in the repository.

### Code Coverage

This project uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) for LLVM-based code coverage. No minimum threshold is enforced yet — tests and a threshold are added once the API stabilizes.

```bash
just coverage
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
