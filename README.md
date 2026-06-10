# geocoordinates

A geospatial coordinate library for Rust — China datums (GCJ-02/BD-09), geodetic transforms, geodesics, ingestion, and presentation.

> **Status:** Early development, released incrementally. The public API is `0.x` and may change between minor versions. See [ROADMAP.md](ROADMAP.md) for the planned release order.

## What's available now

The current release ships the core data model and the China datums:

- **Coordinate model:** `Coordinate`, `Crs`, `Height`, `LatLon`; `Approx<T>`; typed `Error`/`Result`; `Length`/`LengthUnit`; angle encodings (`Dd`/`Dms`/`Ddm`); `Fix` observation metadata.
- **China datums:** `Wgs84` ↔ `Gcj02` ↔ `Bd09` (exact forward transforms, approximate inverses with explicit error bounds) and `BaiduMercator`.
- **Distance:** spherical `haversine_distance`.
- Optional `serde` support (`serde` feature).

Geodesy (ECEF/frames/Karney geodesics/Helmert datums), grids (UTM/MGRS/Geohash/Plus Code/Maidenhead), ingestion (free-text, GeoJSON/WKT/GPX/KML, NMEA/EXIF), formatting, and runtime CRS conversion are scaffolded and land in subsequent releases — see [ROADMAP.md](ROADMAP.md).

## Language bindings (FFI)

A curated subset of the API is exposed to **Python, Kotlin, Swift, and TypeScript**
via [UniFFI](https://mozilla.github.io/uniffi-rs/), generated from the separate
`geocoordinates-ffi` crate (Java consumes the Kotlin/JVM artifact directly). The
bindings cover the China-datum core: WGS-84 ↔ GCJ-02 ↔ BD-09 conversions, Baidu
Web Mercator, `out_of_china`, and haversine distance.

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

## Prerequisites

- [Rust (rustup)](https://rustup.rs) — toolchain (pinned via `rust-toolchain.toml`)
- [just](https://github.com/casey/just) — command runner
- [Lefthook](https://github.com/evilmartians/lefthook) — git hooks manager
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — code coverage

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

This project uses [Lefthook](https://github.com/evilmartians/lefthook). Pre-commit hooks auto-fix formatting and linting on staged files. Pre-push hooks run format checks, Clippy, tests, and a coverage report.

### CI/CD

GitHub Actions runs format checks, Clippy, tests, and a coverage report on pushes to `master` and pull requests. A separate FFI workflow builds the bindings `cdylib`, generates bindings for all four languages, and runs a Python smoke test.

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
