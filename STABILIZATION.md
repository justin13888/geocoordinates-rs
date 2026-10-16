# Stabilization ledger

`geocoordinates` shipped thirteen milestones incrementally to v0.14.0 without the
public surface ever being audited as a whole. **0.15.0 is the stabilization
release**: the release whose surface gets dogfooded on real applications before
the 1.0 freeze.

This file is the single contributor-facing record of that effort — what
"stabilized" means, what the audit found, which issue and pull request closed
each finding, and what remains open. `README.md` carries
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

At 0.15.0 every subsystem in the README's surface table meets all five, on this
evidence:

1. The conversion rule is stated in `AGENTS.md`, and the China bridges — the
   one place a conversion existed only as `From` / `TryFrom` — gained named
   methods with FFI mirrors (G2, G3).
2. Every approximate result is an `Approx<T>` whose bound was measured, not
   asserted (C3, C13, C15, C9b), and every routine that runs on the mean sphere
   says so in its name (C1 / C14).
3. Every public item carries a compiling doctest (G12):
   `cargo test --all-features --doc` runs 250 of them.
4. Every `#[uniffi::export]` is called by a Rust test, which
   `crates/geocoordinates-ffi/tests/export_coverage.rs` enforces, and the
   Python, Kotlin / Java, Swift and TypeScript bindings each run behavioural
   tests in CI (G14).
5. Antimeridian, pole and round-trip behavior is pinned by the characterization
   and property tests of #100 and by the regression test each C finding below
   added. The round-trip property excludes exactly one known case, which is
   tested as wrong and tracked as G18.

## Baseline

Measured on `master` at the start of this effort (2026-09-01), and again at the
0.15.0 surface (`master@4dcd177`, 2026-10-12):

| Signal | 2026-09-01 | 0.15.0 |
|---|---|---|
| Tests | 199 passing (191 unit + 8 flagship example) | 680 core-crate (unit, integration, flagship and doctests, all features) + 91 FFI-crate Rust tests |
| Coverage | 96.0% lines, 93.2% functions | 98.85% lines, 97.66% functions; CI fails under a 98% line floor |
| `cargo fmt --check` | clean | clean |
| `clippy -D warnings` (both crates) | clean | clean |
| `rustdoc -D missing_docs` | clean | clean, and denied in CI |
| **Doctests** | **0 executable** — one ` ```ignore ` fence crate-wide | 250 executable |
| **FFI CI** | **red since 2026-08-05** — fixed by #37 | green; Rust tests for every export plus behavioural tests in all four languages |
| Open issues | 0 | #130, #131, #135 and #149 besides #129 (each recorded below) |

## Audit ledger

Every finding is anchored to source by file and symbol, not by line number, so
anchors survive edits. **Issue / PR** names the issue that owned the finding and
the pull request that closed it; status is `open` until that pull request
merges.

| # | Finding | Anchor | Issue / PR | Status |
|---|---|---|---|---|
| G1 | FFI CI red — Java smoke test does not declare the now-checked `GeoException` | `crates/geocoordinates-ffi/jvm/…/SmokeTest.java` (the `throws GeoException` clauses) | — / #37 | closed (#37) |
| G2 | `Wgs84`/`Gcj02`/`Bd09` ↔ `Coordinate` existed only as six `From`/`TryFrom` impls with no named method — violates the mandatory rule and cannot cross FFI | `src/china/mod.rs` | — / #37 | closed (#37) |
| G3 | No FFI way to CRS-check a `Coordinate` down to a typed datum; `Error::CrsMismatch` protection is Rust-only | consequence of G2 | #109 / #151 | closed (#109: `wgs84_from_coordinate` / `gcj02_from_coordinate` / `bd09_from_coordinate` raise `CrsMismatch`, with `*_to_coordinate` the other way) |
| G4 | FFI exposes only `coordinate_wgs84`/`_gcj02`/`_bd09`; no `coordinate_new(lat, lon, crs)`, so `Nad27`/`Tokyo`/`Pulkovo42` cannot be constructed by name despite `convert` accepting them | ffi `src/core.rs` (`coordinate_wgs84` .. `coordinate_bd09`) | #107 / #144 | closed (#107: `coordinate_new` replaces the three) |
| G5 | `validate()` unreachable across FFI except for `Coordinate` — no mirror for `Dms`, `Ddm`, `Ellipsoid`, or the China newtypes | ffi `src/core.rs` (`coordinate_validate`) | #108 / #146, #109 / #151, #110 / #152 | closed (#108 closed `Dms` / `Ddm`: valid by construction, mirrored as `dms_validate` / `ddm_validate`; #109 closed the China newtypes: valid by construction, mirrored as `wgs84_validate` / `gcj02_validate` / `bd09_validate` / `baidu_mercator_validate`; #110 closed `Ellipsoid`: private fields behind a validating `new` and serde, mirrored as `ellipsoid_new`, with the FFI getters validating the record) |
| G6 | `zone_for` / `central_meridian_deg` have no FFI mirror | `src/grids/utm.rs` (`zone_for`, `central_meridian_deg`) | #114 / #145 | closed (#114: fallible `zone_for(&Coordinate)` / `central_meridian_deg`, mirrored as `utm_zone_for` / `utm_central_meridian_deg`) |
| G7 | `Geohash`, `Maidenhead`, `PlusCode`, `Mgrs` implement `FromStr` but **no `Display`** — breaks the crate's own round-trip invariant | `src/grids/` | #111 / #147, #116 / #148 | closed (#111: `Geohash` / `Maidenhead` / `PlusCode` gain `Display` + `AsRef<str>` + `TryFrom<String>`, FFI `*_canonicalize`; #116: `Mgrs` gains `Display` + `TryFrom<String>` and a closed `MgrsPrecision`) |
| G8 | `utm::Hemisphere` is not re-exported, though it types public fields of both `Utm` and `Ups` | `src/grids/mod.rs` (the `utm` re-exports) | #114 / #145 | closed (#114: renamed `UtmHemisphere`, re-exported from `grids` and the prelude) |
| G9 | Geodesic producers never validate their output; `intersection` divides by `phi1.cos()`, so near-pole degeneracy escapes as `Ok` | `src/geodesy/spherical.rs` (`intersection_spherical`, formerly `intersection`) | #113 / #156 | closed (#113: every geodesic and great-circle producer exits through one `finish` that wraps the longitude and validates; `intersection` became `intersection_spherical`, built on n-vectors that take a pole's bearing against its given meridian, returning `Intersection { Crossing, SameGreatCircle, NoForwardCrossing }`) |
| C1 / C14 | `intersection((90, 10), 180°, (0, 0), 90°)` returned the pole instead of (0, 10); geodesics on Tokyo / NAD27 / Pulkovo-1942 points were measured on WGS-84, and GCJ-02 / BD-09 points were accepted by the measurements; coincident `initial_bearing` / `final_bearing` returned arbitrary values and a cross/along-track path with `start == end` was silently a northward meridian | `src/geodesy/geodesic.rs`, `src/geodesy/spherical.rs` | #113 / #156 | closed (#113: Karney geodesics run on `Ellipsoid::for_crs(crs)` and GCJ-02 / BD-09 are `CrsMismatch`; producers keep a classic datum; coincident bearings and a coincident or antipodal cross/along-track path are `InvalidValue`; the spherical cross/along-track pair is renamed `*_spherical`, and `haversine_distance` states its measured 0.56% bound) |
| C7 | Rhumb lines were spherical though named like the ellipsoidal routines; a non-meridional line pushed past a pole "reflected" over it (a rhumb line spirals into the pole and never crosses it), landing exactly on the pole divided by Δψ → ∞, and a pole endpoint added a spurious east-west leg to the distance | `src/geodesy/rhumb.rs` | #115 / #162 | closed (#115: `rhumb_distance` / `rhumb_bearing` / `rhumb_destination` run on `Ellipsoid::for_crs(crs)` with the 4th-order Krüger series now shared with UTM in `src/geodesy/series.rs`, matching GeographicLib's `RhumbSolve` to 10 µm; GCJ-02 / BD-09 are `CrsMismatch`; a non-meridional line reaching a pole is `InvalidValue { field: "distance" }`, and one leaving a pole `InvalidValue { field: "bearing" }`; a line to or from a pole is the meridian; meridional lines still pass over the poles, any number of times) |
| C7b | `rhumb_bearing` from a pole returned 180° (north) / 0° (south) whatever the longitudes, while `rhumb_destination` ran down the start's own meridian, so `rhumb_destination(a, rhumb_bearing(a, b), rhumb_distance(a, b))` from `(90, 0)` to `(0, 90)` landed 10 000 km off; a non-meridional bearing from a pole was rejected though every direction from a pole is a meridian | `src/geodesy/rhumb.rs` (`rhumb_bearing`, `rhumb_destination`) | #175 | open (a bearing at a pole is read against the start's own meridian, as the geodesic functions do: from the north pole `β` runs down `λ + 180° − β`, from the south pole up `λ + β`; `rhumb_bearing` names the target's meridian and `rhumb_destination` accepts every bearing from a pole, so the round trip holds from every start, checked by a property test to 1 mm) |
| G10 | Utm/Ups forward failures are `OutOfRange`, reverse are `InvalidValue`; and 7 of 9 `Error` variants collapse into `GeoError::Other { detail }` across FFI, forcing string-matching | `src/grids/utm.rs` (`Utm` / `Ups` constructors and conversions); ffi `src/error.rs` (`From<gc::Error> for GeoError`) | #106 / #143, #114 / #145 | closed (#143: one-to-one FFI error mirror; #114: UTM/UPS forward → `OutsideDomain`, construction → `InvalidValue { field }`, inverse infallible) |
| G11 | Doc drift — `ROADMAP.md` and `src/lib.rs` described shipped modules as unshipped | `src/lib.rs` | — / #37 | closed (#37) |
| G12 | **Zero executable doc examples crate-wide** | every public item (`src/lib.rs` held the one ` ```ignore ` fence) | #121–#125 / #163, #153, #158, #164, #161 | closed (#121 core types, angles and units; #122 China datums and `convert`; #123 formatting and parsing; #124 geodesy; #125 projected, encoded and discrete grids: every public item carries a compiling doctest, and a fallible one shows an error case; 250 run) |
| G13 | `src/proj/mod.rs` and `src/height/mod.rs` are undeclared dead files holding four `todo!()`s | the two deleted files' `todo!()` bodies | #103 / #140 | closed (#103: stubs deleted) |
| G14 | FFI gate is thin: one Python smoke test, one Java smoke test, zero Rust tests in the FFI crate, none for Kotlin/Swift/TypeScript | `crates/geocoordinates-ffi/` | #101 / #136, #137 / #139, #126 / #159 | closed (#101: Rust tests for every export, enforced by `tests/export_coverage.rs`, and pinned `smoke.py` reference values; #137: CI runs them; #126: Kotlin, Swift and TypeScript behavioural tests in CI) |
| G15 | `parse/interchange.rs` was 80.4% lines, now 99.29% lines (inline test module included) — lifted by #134's interchange coverage; `china/mod.rs` was 60.0% lines, now 100.0% lines — closed incidentally by the named-method bridges of #37 | `src/parse/interchange.rs`, `src/china/mod.rs` | #100 / #134 | closed (#37, #134) |
| G16 | `Representation` covers only DD/DMS/DDM/Plus Code, so UTM, MGRS, Geohash and Maidenhead ship as types that cannot be **formatted**, and `parse_coordinate` cannot detect their tokens — format/parse are asymmetric with the grids subsystem | `src/format/mod.rs` (`Representation`), `src/parse/mod.rs` (`parse_coordinate`) | #117 / #150, #118 / #154 | closed (#150: `Representation` renders Plus Code, Geohash, Maidenhead and MGRS; UTM has no string form. #118: `parse_coordinate` always detects Plus Code and MGRS, decodes Geohash / Maidenhead when `ParseOptions::expect_grid` names them, and `FormatOptions::parse_options` re-parses every rendering; the same change rejects free text with no single reading — bare DDMM, a fraction before the last field, exponent notation — and splits decimal-comma pairs on the remaining `,` / `;`) |
| G17 | `from_kml` never returns on a truncated document whose last open element is a geometry or placemark (`"<kml><Point>"`, `"<kml><Placemark>"`): the `kml` 0.13.0 reader spins, and nothing here bounds it, so a 12-byte untrusted input stalls the caller. Found while lifting G15 | `src/parse/interchange.rs` `from_kml` | #135 / — | open; deferred past 0.15.0 because the loop is inside the `kml` crate's reader and bounding it changes no public signature, so the fix can ship in a patch release |
| G18 | `Mgrs::to_coordinate` returns the center of the nominal square even where the grid-zone designator truncates it (zone edge, band edge, antimeridian), so decoding `01NAA` and re-encoding at the same precision gives `60NZF`. Found by #117's `format → parse → format` property | `src/grids/mgrs.rs` (`Mgrs::to_coordinate`); pinned as KNOWN WRONG in `src/format/tests/round_trip.rs` | #149 / — | open; deferred past 0.15.0 because the fix moves a returned coordinate without changing any signature, so it can ship in a patch release |
| G19 | `Approx<T>` dereferenced to `T`, so `approx.lat()` read as exact; it had no public constructor, so a consuming crate could not wrap its own approximation, and serde accepted a negative or NaN bound. Magnitudes in meters were split between `Length` and bare `f64` (`Approx::max_error_m`, `DatumTransform::accuracy_m`, `Accuracy` fields, `Ellipsoid` axes, `MgrsPrecision::meters() -> u32`), coordinate components lacked a unit suffix (`Ecef::x`, `Enu::east`, `Utm::easting()`, `BaiduMercator::x()`), and the exhaustive `LengthUnit` lacked the statute mile and yard | `src/approx.rs` (`Approx`), `src/units.rs` (`LengthUnit`), `src/geodesy/datum.rs` (`DatumTransform::accuracy`), `src/fix.rs` (`Accuracy`), `src/geodesy/ellipsoid.rs`, `src/grids/mgrs.rs` (`MgrsPrecision::side`) | #174 / — | open until #174's pull request merges (no `Deref`; validated `Approx::new(value, max_error: Length)` and serde; every public magnitude is a `Length`; components are `f64` with an `_m` suffix; `LengthUnit::{Mile, Yard}`; the FFI keeps `f64` meters) |
| C2 | The worldwide BD-09 nudge returned an invalid `Ok` past the antimeridian (GCJ-02 (0, 180) → lon ≈ 180.0065) and past the North Pole (GCJ-02 (90, 0) → lat ≈ 90.006), and the refined inverse failed on the valid BD-09 (0, −180); the `china` docs claimed every conversion is the identity outside China | `src/china/bd09.rs`, `src/china/mod.rs` | #109 / #151 | closed (#109: longitude wrapped into `[-180, 180)` with wrap-aware refinement; a latitude pushed past a pole is `OutsideDomain`; docs state BD-09 applies worldwide) |
| C3 | Refined GCJ-02 / BD-09 inverses claimed 0.5 m even when the fixed point hit its 100-step cap (GCJ-02 (40, 72.006) at the box edge: true residual ≈ 362 m); the stated fast bounds disagreed with their docs and with a measured sweep (GCJ-02 fast 5 m vs "~1–2 m" and a measured 5.15 m; BD-09 fast 1 m vs "decimeter-level", and ≈ 120 m at the antimeridian) | `src/china/gcj02.rs`, `src/china/bd09.rs` | #109 / #151 | closed (#109: one `hypot` convergence test; an unconverged run reports 0.5 m plus its residual; fast bounds re-measured: GCJ-02 6 m, BD-09 0.5 m, 125 m within 0.02° of the antimeridian; a GCJ-02 input just outside the box that an inside point also maps to, e.g. WGS (55.82477, 130.0) → GCJ (55.8271067, 130.0100627), is still returned unchanged but with a bound covering the inside preimage, not 0) |
| C17 | Baidu Mercator stretched latitudes beyond Baidu's own ±74° clamp, and its band `lookup` returned an `Option` that could never be `None` | `src/china/baidu_mercator.rs` | #109 / #151 | closed (#109: `try_from_bd09` rejects \|lat\| > 74° with `OutsideDomain`; `try_to_bd09` rejects a northing beyond the image of 74°; the unreachable 75° band is dropped and `lookup` is total) |
| D11 | `can_convert` / `hub_reachable` always returned `true` (a surviving mutant), `convert`'s `# Errors` listed only a never-produced variant, and the Helmert legs used `.expect("classic datum is catalogued")` | `src/convert.rs` | #109 / #151 | closed (#109: `can_convert` removed from the crate, prelude, and FFI; `# Errors` lists `OutsideDomain` and `InvalidValue`; routing is one exhaustive `HubRoute` lookup) |
| C8 | `PlusCode::encode` near lat 90 / lon 180 emitted a first digit `F` / `W`, and `Maidenhead::encode` at lon 179.99999999999997 a field `S` — codes their own validators reject | `src/grids/encoded/plus_code.rs`, `maidenhead.rs` | #111 / #147 | closed (#111: integer indices floored, clamped and wrapped as Google's `locationToIntegers`; Maidenhead indices clamped) |
| C8b | `Mgrs::try_from_coordinate(Coordinate::wgs84(-8e-15, 3.0)?, MgrsPrecision::M1)` emitted `31MEA0000000000`: band M with a southern northing rounded to exactly 10 000 000 m, a square no southern northing reaches, so the crate's own parser rejected the reference and `to_coordinate()` on the encoded value panicked on its `expect` | `src/grids/mgrs.rs` (`Mgrs::try_from_coordinate`, `encode_utm`, `band_letter`, `Mgrs::to_coordinate`) | #169 / #194 | closed (#169: `encode_utm` takes one hemisphere decision for both the northing and the band letter, writing a southern northing of exactly 10 000 000 m as the equator in band N; `try_from_coordinate` builds its value by parsing the string it emitted, so it cannot return a reference the parser rejects; `Mgrs` holds its decoded square center, so `to_coordinate` neither re-parses nor panics; property tests run `encode → Display → parse → to_coordinate` at every precision, worldwide and within 1e-6° of the equator) |
| C9 | Serde bypassed validation: `Coordinate` deserialized `{"lat": 91, ...}`, `Dms` minutes = 60, and the encoded grids garbage (Maidenhead panic, garbage geohash, dropped Plus Code bytes); `Geohash::try_from` accepted any length while `encode` capped at 22 | `src/coord.rs`, `src/angle.rs`, `src/grids/encoded/`, `src/grids/mgrs.rs` | #107 / #144, #108 / #146, #111 / #147, #116 / #148 | closed (#107: `Coordinate` deserializes through its validating constructor, and the encoded codes through a validating `try_from = "String"`; #108: `Dms` / `Ddm` deserialize through their validating constructors, so minutes = 60 is rejected; #111: geohash capped at 12 for encode and parse; #116: `Mgrs` serializes as its validated canonical string, so an invalid reference no longer deserializes and panics in `to_coordinate`) |
| C13 | Cell bounds scaled longitude by cos(center latitude) and used 111,320 m/deg, understating tall and polar cells; the format heuristic used 111,195 | `src/grids/encoded.rs`, `src/format/mod.rs` | #111 / #147 | closed (#111: `max_error_m` is the largest Karney distance from center to a corner; 111,195 survives only as `format`'s named `MEAN_METERS_PER_DEGREE` precision heuristic) |
| C15 | `Ecef::try_from_coordinate(coord, ellipsoid)` / `try_to_coordinate(ellipsoid, crs)` and `DatumTransform`'s free `from` / `to` fields let an ellipsoid contradict the CRS (a NAD27 point placed on WGS 84 was off by about 70 m and returned `Ok`); the Bowring single-step inverse lost up to 0.3 m of latitude at GNSS altitudes; `Helmert::inverse` was first-order but named as exact; `convert` reported the Helmert legs' bound as `0.0` | `src/geodesy/ecef.rs`, `src/geodesy/datum.rs`, `src/convert.rs` | #110 / #152 | closed (#110: the ellipsoid is `Ellipsoid::for_crs(coord.crs())`, and GCJ-02 / BD-09 are `CrsMismatch`; the inverse is Vermeille's closed form as refined in GeographicLib; `DatumTransform` holds only its CRSs, shift and accuracy behind a validating `new`, with `apply` / `catalog_to_wgs84` / `inverse_first_order`; each catalogued shift carries NIMA TR8350.2's published accuracy, which `convert` reports, and Pulkovo-1942, which has none, reports infinity) |
| N4 | `Enu::to_aer` computed elevation as `asin(up / range)`, which is NaN once rounding pushes the ratio past one, and squared the components, so an offset under about 1e-162 m or over about 1e154 m reported a 0° elevation | `src/geodesy/frames.rs` | #110 / #152 | closed (#110: elevation is `atan2(up, hypot(east, north))`, and range is a `hypot`) |
| B8–B11 | `from_wkt` assumed lon-lat though WKT carries no CRS; `RawSource.axis_order` meant "assumed" for text but `None` for `geo:`, `LatLon` for GPX's labelled attributes; every interchange `Fix.raw` held a copy of the whole document; KML and GPX altitude were labelled ellipsoidal; GPX `<time>` / `<hdop>` were dropped and position errors named no index | `src/parse/interchange.rs`, `src/fix.rs`, `src/parse/mod.rs`, `src/parse/sensors.rs` | #119 / #155 | closed (#119: `from_wkt(input, axis_order)`; `axis_order` is the listing order, `None` only for labelled axes or grid references; `raw` is the position's own fragment, re-parseable for GeoJSON / WKT / KML; KML and GPX heights are orthometric; GPX time and `HDOP × 5 m` accuracy are kept; errors name the position's index) |
| C16 | `from_nmea_sentence` returned a position from RMC / GLL status `V` (with only a note) and from GGA fix quality 0; `807.038` parsed as 80° 7.038′; a GGA altitude in any unit was read as meters; RMC's date and time were discarded | `src/parse/sensors.rs` | #120 / #157 | closed (#120: status `V`, mode `N` and GGA quality 0 are `Error::Parse`; minutes need exactly two integer digits; a GGA altitude needs the unit `M`; RMC date + time fill `Fix.timestamp`, two-digit years read as 1980–2079) |
| C9b | `H3Cell(pub u64)` / `S2CellId(pub u64)` accepted any `u64`, so serde deserialized undefined ids and `decode` had to re-validate and was fallible | `src/dgg/mod.rs` | #112 / #160 | closed (#112: the id is private behind a validating `new` / `TryFrom<u64>` and serde `try_from = "u64"`, read back with `as_u64` / `u64::from`; `decode` is infallible and returns `Approx<Coordinate>`; the FFI records stay plain `u64`, so `h3_decode` / `s2_decode` validate and remain fallible) |
| C4 | `Coordinate::gcj02(..).to_string().parse::<Coordinate>()` came back as WGS-84 with its height dropped, and `Coordinate::wgs84(91.0, 0.0).to_string()` panicked because `Display` returned `fmt::Error` | `src/coord.rs` (`Coordinate`) | #107 / #144 | closed (#107: `Coordinate` is valid by construction behind private fields and a fallible constructor, and implements neither `Display` nor `FromStr`, so no implicit text round trip can relabel the datum; `format` does not write the datum and `parse_coordinate` returns WGS-84 (`ParseOptions` has no datum field), so a caller carrying a non-WGS-84 coordinate through text must keep its datum outside the string and re-tag the parsed value; a `compile_fail` doctest pins both absences) |
| C5 | `normalize_degrees(-1e-20)` returned `360.0` (`rem_euclid` rounding) and `-0.0` stayed `-0.0`, leaking into bearings and AER azimuth | `src/angle.rs` (`normalize_degrees`) | #108 / #146 | closed (#108: the result is in `[0, 360)` and never `-0.0`) |
| C6 | The UTM inverse never wrapped longitude (zone 1 eastings under about 166 km and zone 60 over about 834 km were `OutOfRange`, and MGRS squares at the antimeridian fell through to `InvalidGridRef`), and it accepted a northing up to 10 000 000 m in either hemisphere, folding over the pole past about 9 997 964 m | `src/grids/utm.rs` (`Utm::to_coordinate`) | #114 / #145 | closed (#114: the inverse wraps longitude into `[-180, 180)` and is infallible; construction rejects a northing past the pole with `InvalidValue { field: "northing" }`) |
| C10 | The default `FormatOptions` fixed precision at 6 decimals, so `format_fix` ignored the fix's accuracy | `src/format/mod.rs` (`FormatOptions`, `format_fix`) | #117 / #150 | closed (#117: precision is unset by default and `format_fix` derives it from the accuracy) |
| C11 | Decimal degrees printed `-0.000000`, and the cardinal letter was chosen before rounding, so `"0.000000 S"` re-parsed and re-rendered as `"0.000000 N"` | `src/format/render.rs` | #117 / #150 | closed (#117: the sign is chosen after rounding in every angle representation, a value that rounds to zero renders unsigned with N / E, and the `format → parse → format` property runs across every representation and separator) |
| C12 | The free-text parser read `"1234.5, 10"` as DDMM (12.575°), `"40.5 30"` as 40.5° plus 30′, the `e` of `"4e1 3e1"` as East, failed on the decimal-comma pair `"48,8566, 2,3522"`, and carried a dead `_order` parameter | `src/parse/text/` | #118 / #154 | closed (#118: each of these is rejected or read one way only — DDMM only beside a hemisphere, no fractional degrees before minutes, no exponent notation — and decimal-comma pairs split on the remaining `,` / `;`) |

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

What landed, in merge order. Each pull request closed the issue beside it; the
ledger rows it closed are listed.

| Issue | PR | Outcome | Closes |
|---|---|---|---|
| — | #37 | Ground the strategy; green the FFI gate; give the China bridges named methods | G1, G2, G11 |
| #99 | #132 | Correct stale and contradicted repository documentation | — |
| #100 | #134 | Pin current behaviour with characterization and property tests | G15 |
| #101 | #136 | Rust tests for every FFI export; pinned Python smoke values | G14 (part) |
| #102 | #138 | Split the text parser below the file-length limit | — |
| #137 | #139 | Run the FFI crate's Rust tests in CI | G14 (part) |
| #103 | #140 | Split `geodesic.rs` by model; delete the dead `proj` / `height` stubs | G13 |
| #104 | #141 | Split the FFI crate's `lib.rs` by subsystem | — |
| #105 | #142 | Split the flagship example by subsystem | — |
| #106 | #143 | Precise `Error` taxonomy, mirrored one-to-one across FFI | G10 (FFI) |
| #107 | #144 | `Coordinate` valid by construction; `coordinate_new` | G4, C4, C9 (part) |
| #114 | #145 | `Utm` / `Ups` valid by construction; wrapped inverse; `UtmHemisphere` | G6, G8, G10 (Rust), C6 |
| #108 | #146 | Validated `Dms` / `Ddm` conversions; `normalize_degrees` fixed | G5 (part), C5, C9 (part) |
| #111 | #147 | Encoded grid codes valid by construction, with `Display` and honest bounds | G7 (part), C8, C9 (part), C13 |
| #116 | #148 | MGRS typed precision, validated serde and `Display` | G7, C9 |
| #117 | #150 | `Representation` carries its own settings and covers every grid | G16 (part), C10, C11 |
| #109 | #151 | China datum types valid by construction with truthful inverse bounds | G3, G5 (part), C2, C3, C17, D11 |
| #110 | #152 | Ellipsoids derived from the CRS; exact ECEF inverse; published Helmert bounds | G5, C15, N4 |
| #122 | #153 | Compiling doctests: China datums and `convert` | G12 (part) |
| #118 | #154 | `ParseOptions` with grid detection; ambiguous free text rejected | G16, C12 |
| #119 | #155 | Explicit axis order and altitude meaning in interchange parsing | B8–B11 |
| #113 | #156 | Datum-aware geodesics; pole intersection fixed; spherical functions named | G9, C1 / C14 |
| #120 | #157 | NMEA sentences with no fix rejected; RMC timestamped | C16 |
| #123 | #158 | Compiling doctests: formatting and parsing | G12 (part) |
| #112 | #160 | H3 and S2 cell ids valid by construction | C9b |
| #126 | #159 | Kotlin, Swift and TypeScript behavioural tests in CI | G14 |
| #125 | #161 | Compiling doctests: projected, encoded and discrete grids | G12 (part) |
| #115 | #162 | Rhumb lines on the datum ellipsoid, stopped at the poles | C7 |
| #121 | #163 | Compiling doctests: core types, angles and units | G12 (part) |
| #124 | #164 | Compiling doctests: geodesy | G12 |
| #128 | #165 | Ratcheting line-coverage floor in CI | — |
| #127 | #166 | Surviving mutants in the core crate killed | — |
| #129 | #167 | Reconcile README, this ledger and `AGENTS.md` with the 0.15.0 surface | — |

## 0.15.0 gate checklist

0.15.0 is not cut until all of these hold. Evidence is from `master@4dcd177`
(2026-10-12), the surface the release PR (#133) ships:

- [x] Every ledger finding above is closed or explicitly deferred here with a
  reason: G17 (#135) and G18 (#149) stay open and are deferred to a patch
  release, as their rows say; every other row is closed.
- [x] `mise run check` and `mise run ffi-check` green — CI run 38323226790 and
  FFI run 38323226739 at `4dcd177`.
- [x] `cargo test --all-features --doc` executes a non-zero number of doctests —
  250 pass.
- [x] `mise run bindings` regenerates cleanly for all four languages — the FFI
  run above generates Python, Kotlin and Swift (and checks the committed Swift
  bindings are current) and builds the WASM / TypeScript package, and each
  language's tests pass.
- [x] `cargo run --all-features --example flagship` passes and exercises the
  full surface — "All 8 shipped Rust capability groups passed."
- [x] Coverage threshold enabled in CI and met — a 98% line floor (measured 98.67%)
  in `.coverage-baseline`, held shrink-only by `mise run coverage-ratchet` (#128).
- [x] `cargo semver-checks` output reconciled against `CHANGELOG.md` — every
  break recorded; see below.

### `cargo semver-checks` against 0.14.2

`cargo semver-checks -p geocoordinates --baseline-version 0.14.2 --all-features`
(cargo-semver-checks 0.51.0) fails 17 major checks. Every break traces to a
pull request whose commit carries `!`, which the release PR's 0.15.0 changelog
lists as **breaking**:

| Break (lint) | Items | PR |
|---|---|---|
| `enum_variant_missing` | `Error::UnsupportedConversion`, `GeoidDataUnavailable`, `FeatureDisabled` | #143 |
| `struct_pub_field_missing`, `struct_pub_field_now_doc_hidden`, `inherent_method_missing` | `Coordinate` fields, `Coordinate::validate` | #144 |
| `trait_newly_sealed`, `trait_added_supertrait` | `LatLon` | #144 |
| `struct_pub_field_missing`, `inherent_method_missing`, `enum_missing`, `function_parameter_count_changed` | `Utm` / `Ups` fields, `try_to_coordinate`; `utm::Hemisphere` (now `UtmHemisphere`); `zone_for` | #145 |
| `struct_missing`, `struct_pub_field_missing`, `inherent_method_missing` | `Dd`; `Dms` / `Ddm` fields, `validate`, `try_to_*` | #146 |
| `inherent_method_missing` | `Mgrs::precision_m` | #148 |
| `constructible_struct_adds_field`, `copy_impl_added`, `struct_pub_field_missing`, `inherent_method_missing`, `enum_unit_variant_changed_kind`, `enum_variant_added`, `enum_discriminants_undefined_non_unit_variant` | `FormatOptions` fields, `uses_decimal_comma`; `Representation` | #150 |
| `struct_pub_field_missing`, `inherent_method_missing`, `function_missing` | `Wgs84` / `Gcj02` / `Bd09` / `BaiduMercator` fields, `validate`, `try_to_*`; `can_convert` | #151 |
| `struct_pub_field_missing`, `constructible_struct_adds_private_field`, `inherent_method_missing`, `method_parameter_count_changed` | `Ellipsoid` and `DatumTransform` fields, `Ellipsoid::validate`, `Helmert::inverse`, `DatumTransform::{to_wgs84, transform, inverse}`; `Ecef::try_{from,to}_coordinate` | #152 |
| `struct_missing` | `parse::text::TextParseOptions` (now `parse::ParseOptions`) | #154 |
| `function_parameter_count_changed` | `from_wkt` | #155 |
| `function_missing` | `intersection`, `cross_track_distance`, `along_track_distance` (now `*_spherical`) | #156 |
| `struct_pub_field_missing` | `H3Cell.0`, `S2CellId.0` | #160 |

The behavioural breaks the tool cannot see (#157 rejecting no-fix NMEA, #162
moving rhumb lines onto the ellipsoid) are also marked breaking there. The
release PR's header line reads "API compatible changes"; the changelog's
**breaking** markers, not that line, are the record. Running
`cargo semver-checks` as a required CI check waits for the 1.0 freeze (below),
since every 0.x minor may break.

0.x means each minor may break. **0.15.0 breaks freely and deliberately**, so the
surface that gets dogfooded is the one intended for the 1.0 freeze.

## Deferred

Unscheduled. Promoted only when a concrete consumer appears.

- **`proj`** — PROJ-backed EPSG/datum long tail (`CrsId`, `proj::transform`).
  Blocked: needs the system `libproj` C library.
- **`geoid`** — EGM96/EGM2008/EGM2020 height undulation. Blocked: needs
  multi-megabyte external grid data files.
- **Ellipsoidal intersection and cross-track distance** (#130) — Karney's
  gnomonic iteration, which would give WGS-84-accurate answers where the
  `*_spherical` functions are exact only on the mean sphere. New math rather
  than a fix: the spherical forms say what they are in their names and bounds.
- **Automatic Maven Central release** (#131) — the JVM workflow uploads each
  release, but the deployment waits in the Central Portal for a manual release
  (see `PUBLISHING.md`).

`proj` and `geoid` have no source in-tree: #140 deleted the `todo!()` stubs (git
history keeps them). Each survives only as a commented-out Cargo feature, so neither can
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
