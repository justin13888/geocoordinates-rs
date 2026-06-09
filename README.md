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

### Git Hooks

This project uses [Lefthook](https://github.com/evilmartians/lefthook). Pre-commit hooks auto-fix formatting and linting on staged files. Pre-push hooks run format checks, Clippy, tests, and a coverage report.

### CI/CD

GitHub Actions runs format checks, Clippy, tests, and a coverage report on pushes to `master` and pull requests.

### Code Coverage

This project uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) for LLVM-based code coverage. No minimum threshold is enforced yet — tests and a threshold are added once the API stabilizes.

```bash
just coverage
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
