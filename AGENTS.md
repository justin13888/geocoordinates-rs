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
