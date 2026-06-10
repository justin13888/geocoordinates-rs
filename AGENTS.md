# gcoordinates

A feature-complete geospatial coordinate library for Rust — China datums (GCJ-02/BD-09), geodetic transforms, geodesics, ingestion, and presentation.

## Architecture constraints

- **Own what `geo` lacks:** GCJ-02 / BD-09 offset math, DMS / DDM parsing, fuzzy free-text coordinate extraction, locale-aware formatting.
- **Honesty about accuracy:** mark GCJ-02 / BD-09 inversions as approximate (iterative, lossy, decimeter-to-meter) — never present them as exact like a Helmert transform.
- **Axis order is first-class:** GeoJSON / WKT are lon-lat (X,Y); humans and many EPSG CRS are lat-first. Never let it be implicit.
- **Antimeridian and poles:** the two places spatial routines break — handle them explicitly throughout.
- **Round-trip stability:** parse → model → format → parse must not drift.

## Quality

Validate changes:

```bash
just test            # correctness
just fmt-check       # formatting
just lint            # clippy with warnings denied
just coverage        # coverage report (no threshold enforced yet)
```

## Project Conventions

- Prioritize intuitive, language-idiomatic APIs that leverage compile-time checks to enforce correct usage without unnecessary restriction, supplemented by concise inline documentation to resolve any remaining ambiguity.
  - Approximate conversions must say so in both the doc comment and (where natural) the name, with a stated error bound.
- All `pub` items need doc comments. Mark pure getters / fallible-return types with `#[must_use]` where appropriate.
- Safe rust only. Return typed errors via `thiserror`.
- Keep `geo` reuse direct — wrap only to add China-datum or parsing logic `geo` does not provide.
- **Conversion naming:** geometric / projected types (ECEF, ENU/NED/AER, UTM/UPS, MGRS) use `to_coordinate` / `from_coordinate` (or `TryFrom` where the forward is fallible); discrete cell / encoding systems (Geohash, Plus Code, Maidenhead, H3, S2) keep domain-standard `encode` / `decode`.
- **serde:** under the `serde` feature, derive `Serialize`/`Deserialize` on every value / option / id type (coordinates, enums, config structs like `FormatOptions`/`TextParseOptions`, ids like `CrsId`). Skip only purely behavioral types.
- **FFI translatability (UniFFI):** the separate `geocoordinates-ffi` crate mirrors a curated subset of the public API across the FFI boundary (Python, Kotlin, Swift, and TypeScript/WASM; Java consumes the Kotlin/JVM artifact, and TypeScript is generated with `uniffi-bindgen-react-native` targeting wasm32 — see `crates/geocoordinates-ffi/web/`). The Rust API stays idiomatic; the bindings adapt to it. When extending the public API, design with the boundary in mind, because these idioms **cannot** cross FFI: generics (`Approx<T>`), traits and `&impl Trait` bounds (`LatLon`), `Deref`, operator overloads (`Length`), and `From`/`TryFrom` conversions. Their FFI forms are flattened concrete records (e.g. `ApproxWgs84 { lat, lon, max_error_m }`), free functions in place of trait methods/operators/conversions, and primitives in place of wrappers (distance as `f64` meters). Keep exported names language-neutral and preserve exactness semantics across the boundary (retain `_fast`/`_refined`). Mirror types are closed snapshots: adding a `#[non_exhaustive]` variant (`Crs`, `LengthUnit`) or a new FFI-relevant value type means updating the mirror + its `From`/`Into` impls in `geocoordinates-ffi`.
