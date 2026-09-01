# gcoordinates

Low-level geospatial coordinate primitives for Rust — China datums (GCJ-02/BD-09),
angle encodings, coordinate parsing/formatting, and geodesy utilities. UNIX
philosophy: this crate abstracts **only** geo-related complexity, as primitives for
higher-level libraries to consume (e.g. EXIF GPS extraction lives in a separate
library built on this one).

## Architecture constraints

- **Own the coordinate domain; delegate reference math:** GCJ-02 / BD-09 offset math, DMS / DDM parsing, fuzzy free-text coordinate extraction, locale-aware formatting, frames, and datum transforms are owned here. Heavyweight, well-validated reference math is delegated behind narrow deps (Karney geodesics via `geographiclib-rs`; interchange formats behind cargo features). Polygon / line geometry ops are out of scope — point users at the `geo` crate.
- **Honesty about accuracy:** mark GCJ-02 / BD-09 inversions as approximate (iterative, lossy, decimeter-to-meter) — never present them as exact like a Helmert transform.
- **Axis order is first-class:** GeoJSON / WKT are lon-lat (X,Y); humans and many EPSG CRS are lat-first. Never let it be implicit.
- **Antimeridian and poles:** the two places spatial routines break — handle them explicitly throughout.
- **Round-trip stability:** parse → model → format → parse must not drift.

## Quality

Validate changes:

```bash
mise run test         # correctness
mise run fmt-check    # formatting
mise run lint         # clippy with warnings denied
mise run ffi-check    # FFI crate clippy (the mirror gates every release)
mise run coverage     # coverage report (no threshold enforced yet)
```

`STABILIZATION.md` is the ledger for the 0.15.0 API stabilization: the five-point
bar every public item must meet, the open findings with their source anchors, and
the delivery sequence. Check it before changing a public signature, and record any
new finding there.

## Project Conventions

- Prioritize intuitive, language-idiomatic APIs that leverage compile-time checks to enforce correct usage without unnecessary restriction, supplemented by concise inline documentation to resolve any remaining ambiguity.
  - **Ergonomic types over footguns:** make misuse unrepresentable — datum-tagged newtypes (`Wgs84`, `Gcj02`) over bare `f64` pairs, `Length` over raw meters, `Approx<T>` over silently lossy results — so the compiler catches mixed datums, swapped axes, and exact-vs-approximate confusion at build time. Correctness-by-construction must live in **distinct concrete types** (which survive FFI), not in generics or trait bounds (which don't).
  - Approximate conversions must say so in both the doc comment and (where natural) the name, with a stated error bound.
- All `pub` items need doc comments. Mark pure getters / fallible-return types with `#[must_use]` where appropriate.
- Safe rust only. Return typed errors via `thiserror`.
- **Conversion naming:** geometric / projected types (ECEF, ENU/NED/AER, UTM/UPS, MGRS) use `to_coordinate` / `from_coordinate`; discrete cell / encoding systems (Geohash, Plus Code, Maidenhead, H3, S2) keep domain-standard `encode` / `decode`. A **named inherent method is mandatory** for every conversion; `From`/`TryFrom` impls are thin sugar over it — they cannot be the only access path, because they cannot cross FFI.
- **serde:** under the `serde` feature, derive `Serialize`/`Deserialize` on every value / option / id type (coordinates, enums, config structs like `FormatOptions`/`TextParseOptions`, ids like `CrsId`). Skip only purely behavioral types.
- **FFI translatability (UniFFI):** the separate `geocoordinates-ffi` crate exposes the public API across the FFI boundary with **full capability parity** (Python, Kotlin, Swift, and TypeScript/WASM; Java consumes the Kotlin/JVM artifact, and TypeScript is generated with `uniffi-bindgen-react-native` targeting wasm32 — see `crates/geocoordinates-ffi/web/`). Parity is per **capability**, not per signature: every public capability must be reachable through one canonical FFI form, but not every Rust-side overload, trait impl, or operator. The FFI mirror **gates every release** — a milestone is not done until its surface crosses the boundary.
  - Design rule: every new public capability gets an **FFI-expressible canonical form** (named inherent methods or free functions over concrete types); Rust idioms are thin sugar layered on top. These idioms **cannot** cross FFI: generics (`Approx<T>`), traits and `&impl Trait` bounds (`LatLon`), `Deref`, operator overloads (`Length`), and `From`/`TryFrom` conversions. Their FFI forms are flattened concrete records (e.g. `ApproxWgs84 { lat, lon, max_error_m }`), free functions in place of trait methods/operators/conversions, and primitives in place of wrappers (distance as `f64` meters). `SystemTime` crosses natively (UniFFI's builtin `Timestamp`), so `Fix` metadata is FFI-clean. Keep exported names language-neutral and preserve exactness semantics across the boundary (retain `_fast`/`_refined`).
  - Mirror sync is compiler-enforced: FFI-mirrored **data** enums (`Crs`, `LengthUnit`, …) are exhaustive (no `#[non_exhaustive]`), so adding a variant fails the FFI crate's build until the mirror + its `From`/`Into` impls are updated. Mirror wildcard arms are allowed **only** where the fallback preserves meaning (errors → `GeoError::Other { detail }`), never where they would mislabel data (datums, units). `Error` alone stays `#[non_exhaustive]`.
