# Stabilization ledger

`geocoordinates` shipped thirteen milestones incrementally to v0.14.0 without the
public surface ever being audited as a whole. **0.15.0 is the internal
stabilization target**: the release whose surface gets dogfooded on real
applications before the 1.0 freeze.

This file is the single contributor-facing record of that effort — what
"stabilized" means, what the audit found, and what remains. `README.md` carries
the user-facing surface table. This file is excluded from the published crate
(`exclude` in `Cargo.toml`); it is a contributor plan, not API docs.

## What "stabilized" means

An item is stabilized when **all five** hold:

1. **Naming.** It follows the crate's conversion rule — a named inherent method
   is the access path; `From` / `TryFrom` are thin sugar over it. Geometric and
   projected types use `to_coordinate` / `from_coordinate`; discrete cell and
   encoding systems use `encode` / `decode`.
2. **Honest exactness.** Approximate results return `Approx<T>` carrying a real
   error bound, and keep their `_fast` / `_refined` suffixes.
3. **Executable docs.** It has a doc comment *with a compiling doctest*.
4. **FFI parity.** It has one canonical FFI form, with semantics preserved
   across the boundary.
5. **Edges tested.** Antimeridian, pole, and round-trip behavior are tested, not
   assumed.

Criteria 1, 2 and 5 are largely met today. Criteria 3 and 4 are where the work is.

## Baseline

Measured on `master` at the start of this effort (2026-09-01):

| Signal | Value |
|---|---|
| Tests | 199 passing (191 unit + 8 flagship example) |
| Coverage | 96.0% lines, 93.2% functions |
| `cargo fmt --check` | clean |
| `clippy -D warnings` (both crates) | clean |
| `rustdoc -D missing_docs` | clean |
| **Doctests** | **0 executable** — one ` ```ignore ` fence crate-wide |
| **FFI CI** | **red since 2026-08-05** — fixed by PR 1 (#37); green since 2026-09-01 |
| Open issues | 0 |

## Audit ledger

Every finding is anchored to source. Status is `open` until its owning PR merges.
The baseline above is a snapshot from the start of the effort; the `Status`
column below reflects what has actually landed since.

| # | Finding | Anchor | Owner | Status |
|---|---|---|---|---|
| G1 | FFI CI red — Java smoke test does not declare the now-checked `GeoException` | `jvm/…/SmokeTest.java:15,21` | PR 1 | closed (#37) |
| G2 | `Wgs84`/`Gcj02`/`Bd09` ↔ `Coordinate` existed only as six `From`/`TryFrom` impls with no named method — violates the mandatory rule and cannot cross FFI | `src/china/mod.rs` | PR 1 | closed (#37) |
| G3 | No FFI way to CRS-check a `Coordinate` down to a typed datum; `Error::CrsMismatch` protection is Rust-only | consequence of G2 | PR 7 | open |
| G4 | FFI exposes only `coordinate_wgs84`/`_gcj02`/`_bd09`; no `coordinate_new(lat, lon, crs)`, so `Nad27`/`Tokyo`/`Pulkovo42` cannot be constructed by name despite `convert` accepting them | ffi `src/core.rs:360-377` (`coordinate_wgs84`..`coordinate_bd09`) | PR 7 | closed (#107: `coordinate_new` replaces the three) |
| G5 | `validate()` unreachable across FFI except for `Coordinate` — no mirror for `Dms`, `Ddm`, `Ellipsoid`, or the China newtypes | ffi `src/core.rs:400` (`coordinate_validate`) | PR 7 | open (#108 closed `Dms` / `Ddm`: valid by construction, mirrored as `dms_validate` / `ddm_validate`; `Ellipsoid` and the China newtypes remain) |
| G6 | `zone_for` / `central_meridian_deg` have no FFI mirror | `src/grids/utm.rs:94,118` | PR 7 | closed (#114: fallible `zone_for(&Coordinate)` / `central_meridian_deg`, mirrored as `utm_zone_for` / `utm_central_meridian_deg`) |
| G7 | `Geohash`, `Maidenhead`, `PlusCode`, `Mgrs` implement `FromStr` but **no `Display`** — breaks the crate's own round-trip invariant | `src/grids/` | PR 3 | open (#111 closed `Geohash` / `Maidenhead` / `PlusCode`: `Display` + `AsRef<str>` + `TryFrom<String>`, FFI `*_canonicalize`; `Mgrs` remains) |
| G8 | `utm::Hemisphere` is not re-exported, though it types public fields of both `Utm` and `Ups` | `src/grids/mod.rs:17-19` | PR 3 | closed (#114: renamed `UtmHemisphere`, re-exported from `grids` and the prelude) |
| G9 | Geodesic producers never validate their output; `intersection` divides by `phi1.cos()`, so near-pole degeneracy escapes as `Ok` | `src/geodesy/spherical.rs:103,106` | PR 4 | open |
| G10 | Utm/Ups forward failures are `OutOfRange`, reverse are `InvalidValue`; and 7 of 9 `Error` variants collapse into `GeoError::Other { detail }` across FFI, forcing string-matching | `utm.rs:140,246` vs `:184-187`; ffi `src/error.rs:38-51` (`From<gc::Error> for GeoError`) | PR 2 / 3 / 7 | closed (#143: one-to-one FFI error mirror; #114: UTM/UPS forward → `OutsideDomain`, construction → `InvalidValue { field }`, inverse infallible) |
| G11 | Doc drift — `ROADMAP.md` and `src/lib.rs` described shipped modules as unshipped | `src/lib.rs` | PR 1 | closed (#37) |
| G12 | **Zero executable doc examples crate-wide** | `src/lib.rs:21` | PR 6 | open |
| G13 | `src/proj/mod.rs` and `src/height/mod.rs` are undeclared dead files holding four `todo!()`s | `proj:30`, `height:34,42,50` | PR 9 | closed (#103: stubs deleted) |
| G14 | FFI gate is thin: one Python smoke test, one Java smoke test, zero Rust tests in the FFI crate, none for Kotlin/Swift/TypeScript | `crates/geocoordinates-ffi/` | PR 8 | open |
| G15 | `parse/interchange.rs` was 80.4% lines, now 99.29% lines (inline test module included) — lifted by #134's interchange coverage; `china/mod.rs` was 60.0% lines, now 100.0% lines — closed incidentally by PR 1's named-method bridges (#37) | — | PR 5 | closed (#37, #134) |
| G16 | `Representation` covers only DD/DMS/DDM/Plus Code, so UTM, MGRS, Geohash and Maidenhead ship as types that cannot be **formatted**, and `parse_coordinate` cannot detect their tokens — format/parse are asymmetric with the grids subsystem | `src/format/mod.rs:19`, `src/parse/mod.rs:43` | PR 3 | open |
| G17 | `from_kml` never returns on a truncated document whose last open element is a geometry or placemark (`"<kml><Point>"`, `"<kml><Placemark>"`): the `kml` 0.13.0 reader spins, and nothing here bounds it, so a 12-byte untrusted input stalls the caller. Found while lifting G15 | `src/parse/interchange.rs` `from_kml` | #135 | open |
| C8 | `PlusCode::encode` near lat 90 / lon 180 emitted a first digit `F` / `W`, and `Maidenhead::encode` at lon 179.99999999999997 a field `S` — codes their own validators reject | `src/grids/encoded/plus_code.rs`, `maidenhead.rs` | #111 | closed (#111: integer indices floored, clamped and wrapped as Google's `locationToIntegers`; Maidenhead indices clamped) |
| C9 | Encoded-grid serde bypassed validation (Maidenhead panic, garbage geohash, dropped Plus Code bytes); `Geohash::try_from` accepted any length while `encode` capped at 22 | `src/grids/encoded/` | #107 / #111 | closed (#107: validating `try_from = "String"`; #111: geohash capped at 12 for encode and parse) |
| C13 | Cell bounds scaled longitude by cos(center latitude) and used 111,320 m/deg, understating tall and polar cells; the format heuristic used 111,195 | `src/grids/encoded.rs`, `src/format/mod.rs` | #111 | closed (#111: `max_error_m` is the largest Karney distance from center to a corner; 111,195 survives only as `format`'s named `MEAN_METERS_PER_DEGREE` precision heuristic) |

**Verified clean**, and deliberately so — do not "fix" these:

- The FFI mirror has **zero** `_ =>` wildcard arms on data enums. The
  compiler-enforced mirror sync works as designed; the single `other =>` arm is
  the permitted error-only exception.
- Every public data-carrying type derives serde under the `serde` feature.
- `Crs`, `LengthUnit` and `DatumAmbiguity` are exhaustive on purpose, each with a
  doc comment explaining why.
- The China offset constants are correct, with explicit comments guarding the two
  classic bugs (Krasovsky vs WGS-84 ellipsoid; `X_PI` vs plain π).

## Delivery sequence

Ordered by dependency and file collision. Same-wave PRs share no file.

| Wave | PR | Outcome | Closes |
|---|---|---|---|
| 1 | 1 | Ground the strategy; green the FFI gate; give the China bridges named methods | G1, G2, G11 |
| 2 | 2 | Core primitive contracts: `Error` taxonomy, `Timestamp` alias, `#[must_use]` on `LatLon` accessors | G10 (Rust) |
| 2 | 4 | Geodesy correctness: validate producer outputs; handle `intersection` near-pole degeneracy | G9 |
| 2 | 5 | Parse/format hardening; lift `interchange.rs` off 80% | G15 |
| 2 | 9 | Decide `proj` / `height`: delete the dead stubs or gate them properly | G13 |
| 3 | 3 | Grid surface: `Display`, re-export `Hemisphere`, unify Utm/Ups errors, extend `Representation` | G7, G8, G16 |
| 3 | 6 | A compiling doctest on every public item | G12 |
| 4 | 7 | FFI parity sweep; widen `GeoError` | G3, G4, G5, G6, G10 (FFI) |
| 4 | 8 | Thicken the gate: Rust + Kotlin/Swift/TypeScript tests | G14 |
| 5 | 10 | Coverage threshold and `cargo semver-checks` in CI; reconcile `CHANGELOG.md`; cut 0.15.0 | — |

PR 3 is held behind PR 2 (both edit the `src/lib.rs` prelude). PR 6 is held behind
every API-shape PR, since it touches every file. PRs 7 and 8 need the Rust shape
frozen.

## 0.15.0 gate checklist

0.15.0 is not cut until all of these hold:

- [ ] Every ledger finding above is closed or explicitly deferred here with a reason.
- [ ] `mise run check` and `mise run ffi-check` green.
- [ ] `cargo test --all-features --doc` executes a non-zero number of doctests.
- [ ] `mise run bindings` regenerates cleanly for all four languages.
- [ ] `cargo run --all-features --example flagship` passes and exercises the full surface.
- [ ] Coverage threshold enabled in CI and met.
- [ ] `cargo semver-checks` output reconciled against `CHANGELOG.md` — every break recorded.

0.x means each minor may break. **0.15.0 breaks freely and deliberately**, so the
surface that gets dogfooded is the one intended for the 1.0 freeze.

## Deferred

Unscheduled. Promoted only when a concrete consumer appears.

- **`proj`** — PROJ-backed EPSG/datum long tail (`CrsId`, `proj::transform`).
  Blocked: needs the system `libproj` C library.
- **`geoid`** — EGM96/EGM2008/EGM2020 height undulation. Blocked: needs
  multi-megabyte external grid data files.

Neither has any source in-tree: PR 9 deleted the `todo!()` stubs (git history
keeps them). Each survives only as a commented-out Cargo feature, so neither can
be reached from the released surface.

## Out of scope

- **EXIF/XMP GPS extraction** — belongs to a separate library that consumes these
  primitives (angle conversions for GPS rationals, `Fix` / `RawSource`, and
  `DatumAmbiguity::PossiblyGcj02` for the China-EXIF ambiguity).
- **Polygon / line geometry ops** (area, point-in-polygon, centroid,
  simplification) — use [`geo`](https://crates.io/crates/geo) directly.

## After 0.15.0 — the 1.0 freeze

Once the surface has survived dogfooding, freeze the public API and adopt strict
semver: no breaking changes without a major bump, `cargo semver-checks` enforced
as a required check, and the coverage threshold ratcheted rather than relaxed.
