# Stabilization ledger

`geocoordinates` shipped thirteen milestones incrementally to v0.14.0 without the
public surface ever being audited as a whole. **0.15.0 is the stabilization
release**: the release whose surface gets dogfooded on real applications before
the 1.0 freeze.

This file is the single contributor-facing record of that effort — what
"stabilized" means, what the audit found, which issue and pull request closed
each finding, and what remains open. `README.md` carries
the user-facing surface table. This file is not in the published crate
(the `include` allow-list in `Cargo.toml` leaves it out); it is a contributor plan, not API docs.

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
   `cargo test --all-features --doc` runs 299 of them (295 examples and 4
   `compile_fail` pins).
4. Every `#[uniffi::export]` is called by a Rust test, and carries `# Errors`
   when it is fallible, both enforced by
   `crates/geocoordinates-ffi/tests/export_coverage.rs`; the exports follow
   one naming scheme (G27); and the Python, Kotlin / Java, Swift and
   TypeScript bindings each run behavioural tests in CI (G14).
5. Antimeridian, pole and round-trip behavior is pinned by the characterization
   and property tests of #100 and by the regression test each C finding below
   added. Since G18 and G18b closed, neither round-trip property excludes any
   case.

## Baseline

Measured on `master` at the start of this effort (2026-09-01), and again at the
0.15.0 surface after the pre-1.0 audit (`master@8a55fcb`, 2026-10-17):

| Signal | 2026-09-01 | 0.15.0 |
|---|---|---|
| Tests | 199 passing (191 unit + 8 flagship example) | 820 core-crate (513 unit, 8 flagship, 295 doctests and 4 `compile_fail` doctests, all features) + 107 FFI-crate Rust tests |
| Coverage | 96.0% lines, 93.2% functions | 98.85% lines, 97.66% functions; CI fails under a 98% line floor |
| `cargo fmt --check` | clean | clean |
| `clippy -D warnings` (both crates) | clean | clean |
| `rustdoc -D missing_docs` | clean | clean, and denied in CI |
| **Doctests** | **0 executable** — one ` ```ignore ` fence crate-wide | 299 executable |
| **FFI CI** | **red since 2026-08-05** — fixed by #37 | green; Rust tests for every export plus behavioural tests in all four languages |
| Open issues | 0 | none besides #186, the reconciliation that records this column |

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
| C7b | `rhumb_bearing` from a pole returned 180° (north) / 0° (south) whatever the longitudes, while `rhumb_destination` ran down the start's own meridian, so `rhumb_destination(a, rhumb_bearing(a, b), rhumb_distance(a, b))` from `(90, 0)` to `(0, 90)` landed 10 000 km off; a non-meridional bearing from a pole was rejected though every direction from a pole is a meridian | `src/geodesy/rhumb.rs` (`rhumb_bearing`, `rhumb_destination`) | #175 / #192 | closed (#192: a bearing at a pole is read against the start's own meridian, as the geodesic functions do: from the north pole `β` runs down `λ + 180° − β`, from the south pole up `λ + β`; `rhumb_bearing` names the target's meridian and `rhumb_destination` accepts every bearing from a pole, so the round trip holds from every start, checked by a property test to 1 mm; #214: the zero-`distance` return in `rhumb_destination` applies to every start, not only a pole, and returns the latitude of `start` exactly and its longitude wrapped into `[-180, 180)` as every producer's is (a start at 180° comes back at −180°), before the rectifying-latitude series runs, checked by an exact-equality test from a non-pole start, both poles and a start on the antimeridian) |
| G10 | Utm/Ups forward failures are `OutOfRange`, reverse are `InvalidValue`; and 7 of 9 `Error` variants collapse into `GeoError::Other { detail }` across FFI, forcing string-matching | `src/grids/utm.rs` (`Utm` / `Ups` constructors and conversions); ffi `src/error.rs` (`From<gc::Error> for GeoError`) | #106 / #143, #114 / #145 | closed (#143: one-to-one FFI error mirror; #114: UTM/UPS forward → `OutsideDomain`, construction → `InvalidValue { field }`, inverse infallible) |
| G11 | Doc drift — `ROADMAP.md` and `src/lib.rs` described shipped modules as unshipped | `src/lib.rs` | — / #37 | closed (#37) |
| G12 | **Zero executable doc examples crate-wide** | every public item (`src/lib.rs` held the one ` ```ignore ` fence) | #121–#125 / #163, #153, #158, #164, #161 | closed (#121 core types, angles and units; #122 China datums and `convert`; #123 formatting and parsing; #124 geodesy; #125 projected, encoded and discrete grids: every public item carries a compiling doctest, and a fallible one shows an error case; 250 run) |
| G13 | `src/proj/mod.rs` and `src/height/mod.rs` are undeclared dead files holding four `todo!()`s | the two deleted files' `todo!()` bodies | #103 / #140 | closed (#103: stubs deleted) |
| G14 | FFI gate is thin: one Python smoke test, one Java smoke test, zero Rust tests in the FFI crate, none for Kotlin/Swift/TypeScript | `crates/geocoordinates-ffi/` | #101 / #136, #137 / #139, #126 / #159 | closed (#101: Rust tests for every export, enforced by `tests/export_coverage.rs`, and pinned `smoke.py` reference values; #137: CI runs them; #126: Kotlin, Swift and TypeScript behavioural tests in CI) |
| G15 | `parse/interchange.rs` was 80.4% lines, now 99.29% lines (inline test module included) — lifted by #134's interchange coverage; `china/mod.rs` was 60.0% lines, now 100.0% lines — closed incidentally by the named-method bridges of #37 | `src/parse/interchange.rs`, `src/china/mod.rs` | #100 / #134 | closed (#37, #134) |
| G16 | `Representation` covers only DD/DMS/DDM/Plus Code, so UTM, MGRS, Geohash and Maidenhead ship as types that cannot be **formatted**, and `parse_coordinate` cannot detect their tokens — format/parse are asymmetric with the grids subsystem | `src/format/mod.rs` (`Representation`), `src/parse/mod.rs` (`parse_coordinate`) | #117 / #150, #118 / #154 | closed (#150: `Representation` renders Plus Code, Geohash, Maidenhead and MGRS; UTM has no string form. #118: `parse_coordinate` always detects Plus Code and MGRS, decodes Geohash / Maidenhead when `ParseOptions::expect_grid` names them, and `FormatOptions::parse_options` re-parses every rendering; the same change rejects free text with no single reading — bare DDMM, a fraction before the last field, exponent notation — and splits decimal-comma pairs on the remaining `,` / `;`) |
| G17 | `from_kml` never returns on a truncated document whose last open element is a geometry or placemark (`"<kml><Point>"`, `"<kml><Placemark>"`): the `kml` 0.13.0 reader spins, and nothing here bounds it, so a 12-byte untrusted input stalls the caller. Found while lifting G15 | `src/parse/interchange.rs` `from_kml`, `src/parse/interchange/xml_scan.rs` | #135 / #206 | closed (#206: the KML markup pre-scan rejects, as `Error::Parse` of kind `Syntax`, any document with an element left unclosed, an end tag that does not close the innermost open element, an unterminated construct, or a `<!` construct other than a comment, CDATA section, or named DOCTYPE (which `quick-xml` errors on and the `Data` / `SchemaData` readers swallow), so every `kml` reader meets its end tag before end of input; a coordinate-less `Point`, which panics inside `kml` instead, is #205) |
| G17b | `from_kml` panics on a closed, balanced `Point` with no position (`"<Point></Point>"`, 15 bytes): the `kml` 0.13.0 reader builds a `Point` from `coords.remove(0)` unconditionally, so untrusted KML can abort a `panic = "abort"` process or unwind into an FFI caller. Found while tracing the `kml` reader for G17 | `src/parse/interchange.rs` `from_kml`, `src/parse/interchange/xml_scan.rs` (`check_kml_markup`) | #205 / #209 | closed (#209: the KML markup pre-scan holds every non-self-closing `Point` (any prefix) to the span the `kml` geometry reader reads, from its start tag to the first `Point` end tag, and rejects, as `Error::Parse` of kind `Syntax` (`kml: <Point> at byte N has no coordinates`), one whose last `<coordinates>` in that span is followed by neither text that still has a non-whitespace character once its references are unescaped as `quick-xml` unescapes them (coordinates written only as references, such as `&#49;&#44;&#50;`, still parse; a malformed reference counts as the raw text `kml` falls back to; a named entity other than the five predefined ones counts as whitespace, because a downstream crate enabling `quick-xml`'s `escape-html` makes `&nbsp;` and its kin unescape to whitespace in the single `quick-xml` Cargo builds) nor a CDATA section with a non-whitespace character; a coordinate-less `Point` the parser would skip, inside an unknown element or nested in another `Point`, is rejected too; the FFI `from_kml` changes behaviour only) |
| G18 | `Mgrs::to_coordinate` returns the center of the nominal square even where the grid-zone designator truncates it (zone edge, band edge, antimeridian), so decoding `01NAA` and re-encoding at the same precision gives `60NZF`. Found by #117's `format → parse → format` property | `src/grids/mgrs.rs` (`Mgrs::to_coordinate`), `src/grids/mgrs/clip.rs` (`settle`); formerly pinned as KNOWN WRONG in `src/format/tests/round_trip.rs` | #149 / #196 | closed (#196, in 0.15.0 rather than the patch release first planned: a square whose center the encoder does not map back decodes to the center of the largest circle inside both the square and its designator, Norway / Svalbard / UTM–UPS seam included, so it re-encodes to itself, with a bound to the square's farthest corner; the encoder's band letter no longer rounds onto a band edge, which emitted `36X` one ulp below 72°N; both round-trip properties drop their MGRS exclusion; the one encoder rounding gap #196 left is G18b, closed by #207) |
| G18b | A longitude within about 1e-15° west of 3°E (zone 31's central meridian, and the 31V / 32V Norway edge) between 56°N and 64°N projects onto easting 500 000 m exactly, so the encoder emits a `31V…` reference whose square holds no point west of 3°E; its decode falls back to the nominal center, east of 3°E, and re-encodes as `32V…`, so `encode → decode → encode` drifts. Encoder rounding unchanged by #149; found while fixing G18 | `src/grids/mgrs.rs` (`Mgrs::try_from_coordinate`), `src/grids/mgrs/clip.rs` (`encoder_easting`, `settle`'s no-owned-point fallback); the corner property `references_at_grid_zone_corners_round_trip` in `src/grids/mgrs/tests.rs` stopped its offsets at 1e-11° because of it | #198 / #207 | closed (#207: the encoder names a point west of a zone's central meridian by the largest easting below 500 000 m, so it takes the square west of the meridian, the side it lies on, and 31V's square holds it; decode is unchanged; the corner property runs its offsets down to 1e-16°, an ulp at 3°E) |
| G19 | `Approx<T>` dereferenced to `T`, so `approx.lat()` read as exact; it had no public constructor, so a consuming crate could not wrap its own approximation, and serde accepted a negative or NaN bound. Magnitudes in meters were split between `Length` and bare `f64` (`Approx::max_error_m`, `DatumTransform::accuracy_m`, `Accuracy` fields, `Ellipsoid` axes, `MgrsPrecision::meters() -> u32`), coordinate components lacked a unit suffix (`Ecef::x`, `Enu::east`, `Utm::easting()`, `BaiduMercator::x()`), and the exhaustive `LengthUnit` lacked the statute mile and yard | `src/approx.rs` (`Approx`), `src/units.rs` (`LengthUnit`), `src/geodesy/datum.rs` (`DatumTransform::accuracy`), `src/fix.rs` (`Accuracy`), `src/geodesy/ellipsoid.rs`, `src/grids/mgrs.rs` (`MgrsPrecision::side`) | #174 / #190 | closed (#190: no `Deref`; validated `Approx::new(value, max_error: Length)` and serde; every public magnitude is a `Length`; components are `f64` with an `_m` suffix; `LengthUnit::{Mile, Yard}`; the FFI keeps `f64` meters) |
| G19b | The public `Approx::new` that #190 added had no FFI form, and no `approx_*_new` or `*_validate` existed for the `ApproxCoordinate`, `ApproxWgs84` or `ApproxGcj02` records, against the rule that every public capability crosses the boundary and every invariant record has a `<record>_validate`. Found in review of #190 | `src/approx.rs` (`Approx::new`), ffi `src/core.rs` (`ApproxCoordinate`), `src/china.rs` (`ApproxWgs84`, `ApproxGcj02`), `src/lib.rs`; `crates/geocoordinates-ffi/README.md`, `AGENTS.md` | #215 / #219 | closed (#219, a decision with no code change: the three records are output-only, since every export that returns one is a decoder, a `convert` call or a China inverse and none takes one as input, so a foreign caller has nothing to hand a constructed or validated `Approx*` to; `Approx::new` stays Rust-only, where a consuming crate wraps its own approximation, and the `<record>_validate` rule now covers only records an export takes as input. An export that consumes an `Approx*` record must add its `*_validate` in the same change) |
| G20 | `Accuracy.horizontal` held values with different statistical meanings (a grid cell's hard bound, `HDOP × 5 m` at about one sigma, a `geo:` `u=` with no stated level) under one "CEP or reported radius" label and accepted negative or NaN magnitudes; `Confidence` silently clamped 1.5 to 1.0 and NaN to 0.0, in `From<f64>` and serde alike; `Fix`, `Accuracy` and `RawSource` were exhaustive structs with public fields, so no field could be added after 1.0; a GGA geoid separation survived only as the note `"geoid separation 46.9 m"` | `src/fix.rs` (`Accuracy`, `Confidence`, `Fix`, `RawSource`), `src/parse/mod.rs` (`grid_fix`), `src/parse/sensors.rs` (`parse_gga`) | #178 / #195 | closed (#195: `Accuracy::new(kind, horizontal, vertical) -> Result` with private fields, getters and validated serde; exhaustive `AccuracyKind { Bound, OneSigma, Cep50, Unspecified }`, set by every producer; `Confidence::new -> Result`, `TryFrom<f64>`, validated serde; `Fix` / `RawSource` `#[non_exhaustive]` with `Fix::from_coord` / `RawSource::new`; typed `RawSource::geoid_separation`; the FFI mirror adds `AccuracyKind`, `accuracy_new` / `accuracy_validate`, `confidence_validate`, `raw_source_new` and validates a `Fix` crossing in whole) |
| G21 | `Geohash`, `Maidenhead`, `PlusCode` and `Mgrs` parsed only through `TryFrom` / `FromStr` (for `Mgrs` the parser lived inside the `TryFrom` impl), against the rule that a trait impl is never the only access path; no identifier type implemented `Hash`, so `HashSet<H3Cell>` did not compile; `H3Cell` / `S2CellId` had no level getter; and the WGS-84 gate (five copies), the farthest-corner bound (two), the haversine angle (two), the transverse Mercator Krüger sums (spelled out beside `KrugerSeries`) and the UPS constants (recomputed per call) were duplicated | `src/grids/encoded/*.rs`, `src/grids/mgrs.rs` (`parse`), `src/dgg/mod.rs` (`H3Cell::resolution`, `S2CellId::level`), `src/grids/mod.rs` (`require_wgs84`), `src/geodesy/geodesic.rs` (`farthest_geodesic_m`), `src/geodesy/series.rs` (`tm_from_conformal`, `conformal_from_tm`), `src/grids/utm/ups.rs` (`POLAR`) | #180 / #201 | closed (#201: inherent `parse(&str) -> Result<Self>` on the four codes, with `TryFrom` / `FromStr` one-line sugar; `Hash` on all six ids, `Mgrs` hashing its canonical string; `H3Cell::resolution` / `S2CellId::level -> u8`; each duplicate is one `pub(crate)` helper; the FFI adds `h3_resolution`, `s2_level` and `mgrs_canonicalize`) |
| G22 | Cross-track, along-track and intersection existed only on the mean sphere (`*_spherical`, off by up to 0.56% of the distance), with the unsuffixed names reserved for ellipsoidal forms nobody had built | `src/geodesy/track.rs` (`cross_track_distance`, `along_track_distance`, `intersection`, `GeodesicIntersection`) | #130 / #202 | closed (#202: the three run on the ellipsoid of the inputs' own CRS and reject GCJ-02 / BD-09 and mixed CRSs; the foot of a perpendicular is found by re-centring on the current foot after Karney's gnomonic interception, square to the path within 16 nm over a 100,000-triple sweep, and an along-track foot near a point where perpendiculars meet is stated as defined only to about `1e-9 m / M`; a crossing is Karney's gnomonic iteration seeded from the mean-sphere crossings and then each start, within 0.11 µm of both geodesics over 50,000 random crossings, `GeodesicIntersection { Crossing, SameGeodesic, NoForwardCrossing }` with "ahead" meaning the shortest geodesic from the start follows its path; the FFI mirror adds `cross_track_distance_m`, `along_track_distance_m`, `intersection` and `GeodesicIntersectionResult`) |
| G23 | `Error::Parse(String)` carried no structure: a valid NMEA no-fix sentence got the same variant as a checksum failure or garbage, so a streaming caller had to match on message text; and the interchange parsers turned a position's typed `OutOfRange` / `InvalidValue` into a `Parse` string | `src/error.rs` (`Error::Parse`, `ParseErrorKind`), `src/parse/interchange.rs` (`into_fixes`), `src/parse/sensors.rs`, `src/parse/text/`; ffi `src/error.rs` (`GeoError::Parse`) | #181 / #203 | closed (#203: `Error::Parse { kind, detail }` with an exhaustive `ParseErrorKind { Syntax, Checksum, NoFix, Ambiguous, Unsupported, TooDeep }` chosen by every producer: NMEA checksum → `Checksum`, status `V` / mode `N` / GGA quality 0 → `NoFix`, an unhandled NMEA sentence or `geo:` `crs` → `Unsupported`, free text with no single reading → `Ambiguous`, WKT / KML / GeoJSON past their nesting limits → `TooDeep`, everything else `Syntax`; GeoJSON / WKT / KML return an out-of-range position as `OutOfRange` with its values (GPX cannot: the `gpx` reader range-checks `lat` / `lon` itself, so an out-of-range GPX point is `Syntax`), and an `InvalidValue` or `Parse` position error keeps its variant with the format and index in its `detail`; the FFI `GeoError::Parse` gains the mirrored `kind`) |
| C2 | The worldwide BD-09 nudge returned an invalid `Ok` past the antimeridian (GCJ-02 (0, 180) → lon ≈ 180.0065) and past the North Pole (GCJ-02 (90, 0) → lat ≈ 90.006), and the refined inverse failed on the valid BD-09 (0, −180); the `china` docs claimed every conversion is the identity outside China | `src/china/bd09.rs`, `src/china/mod.rs` | #109 / #151 | closed (#109: longitude wrapped into `[-180, 180)` with wrap-aware refinement; a latitude pushed past a pole is `OutsideDomain`; docs state BD-09 applies worldwide) |
| C3 | Refined GCJ-02 / BD-09 inverses claimed 0.5 m even when the fixed point hit its 100-step cap (GCJ-02 (40, 72.006) at the box edge: true residual ≈ 362 m); the stated fast bounds disagreed with their docs and with a measured sweep (GCJ-02 fast 5 m vs "~1–2 m" and a measured 5.15 m; BD-09 fast 1 m vs "decimeter-level", and ≈ 120 m at the antimeridian) | `src/china/gcj02.rs`, `src/china/bd09.rs` | #109 / #151 | closed (#109: one `hypot` convergence test; an unconverged run reports 0.5 m plus its residual; fast bounds re-measured: GCJ-02 6 m, BD-09 0.5 m, 125 m within 0.02° of the antimeridian; a GCJ-02 input just outside the box that an inside point also maps to, e.g. WGS (55.82477, 130.0) → GCJ (55.8271067, 130.0100627), is still returned unchanged but with a bound covering the inside preimage, not 0) |
| C17 | Baidu Mercator stretched latitudes beyond Baidu's own ±74° clamp, and its band `lookup` returned an `Option` that could never be `None` | `src/china/baidu_mercator.rs` | #109 / #151 | closed (#109: `try_from_bd09` rejects \|lat\| > 74° with `OutsideDomain`; `try_to_bd09` rejects a northing beyond the image of 74°; the unreachable 75° band is dropped and `lookup` is total) |
| D11 | `can_convert` / `hub_reachable` always returned `true` (a surviving mutant), `convert`'s `# Errors` listed only a never-produced variant, and the Helmert legs used `.expect("classic datum is catalogued")` | `src/convert.rs` | #109 / #151 | closed (#109: `can_convert` removed from the crate, prelude, and FFI; `# Errors` lists `OutsideDomain` and `InvalidValue`; routing is one exhaustive `HubRoute` lookup) |
| C8 | `PlusCode::encode` near lat 90 / lon 180 emitted a first digit `F` / `W`, and `Maidenhead::encode` at lon 179.99999999999997 a field `S` — codes their own validators reject | `src/grids/encoded/plus_code.rs`, `maidenhead.rs` | #111 / #147 | closed (#111: integer indices floored, clamped and wrapped as Google's `locationToIntegers`; Maidenhead indices clamped) |
| C8b | `Mgrs::try_from_coordinate(Coordinate::wgs84(-8e-15, 3.0)?, MgrsPrecision::M1)` emitted `31MEA0000000000`: band M with a southern northing rounded to exactly 10 000 000 m, a square no southern northing reaches, so the crate's own parser rejected the reference and `to_coordinate()` on the encoded value panicked on its `expect` | `src/grids/mgrs.rs` (`Mgrs::try_from_coordinate`, `encode_utm`, `band_letter`, `Mgrs::to_coordinate`) | #169 / #194 | closed (#169: `encode_utm` takes one hemisphere decision for both the northing and the band letter, writing a southern northing of exactly 10 000 000 m as the equator in band N; `try_from_coordinate` builds its value by parsing the string it emitted, so it cannot return a reference the parser rejects; `Mgrs` holds its decoded square center, so `to_coordinate` neither re-parses nor panics; property tests run `encode → Display → parse → to_coordinate` at every precision, worldwide and within 1e-6° of the equator) |
| C9 | Serde bypassed validation: `Coordinate` deserialized `{"lat": 91, ...}`, `Dms` minutes = 60, and the encoded grids garbage (Maidenhead panic, garbage geohash, dropped Plus Code bytes); `Geohash::try_from` accepted any length while `encode` capped at 22 | `src/coord.rs`, `src/angle.rs`, `src/grids/encoded/`, `src/grids/mgrs.rs` | #107 / #144, #108 / #146, #111 / #147, #116 / #148 | closed (#107: `Coordinate` deserializes through its validating constructor, and the encoded codes through a validating `try_from = "String"`; #108: `Dms` / `Ddm` deserialize through their validating constructors, so minutes = 60 is rejected; #111: geohash capped at 12 for encode and parse; #116: `Mgrs` serializes as its validated canonical string, so an invalid reference no longer deserializes and panics in `to_coordinate`) |
| C13 | Cell bounds scaled longitude by cos(center latitude) and used 111,320 m/deg, understating tall and polar cells; the format heuristic used 111,195 | `src/grids/encoded.rs`, `src/format/mod.rs` | #111 / #147 | closed (#111: `max_error_m` is the largest Karney distance from center to a corner; 111,195 survives only as `format`'s named `MEAN_METERS_PER_DEGREE` precision heuristic) |
| C15 | `Ecef::try_from_coordinate(coord, ellipsoid)` / `try_to_coordinate(ellipsoid, crs)` and `DatumTransform`'s free `from` / `to` fields let an ellipsoid contradict the CRS (a NAD27 point placed on WGS 84 was off by about 70 m and returned `Ok`); the Bowring single-step inverse lost up to 0.3 m of latitude at GNSS altitudes; `Helmert::inverse` was first-order but named as exact; `convert` reported the Helmert legs' bound as `0.0` | `src/geodesy/ecef.rs`, `src/geodesy/datum.rs`, `src/convert.rs` | #110 / #152 | closed (#110: the ellipsoid is `Ellipsoid::for_crs(coord.crs())`, and GCJ-02 / BD-09 are `CrsMismatch`; the inverse is Vermeille's closed form as refined in GeographicLib; `DatumTransform` holds only its CRSs, shift and accuracy behind a validating `new`, with `apply` / `catalog_to_wgs84` / `inverse_first_order`; each catalogued shift carries NIMA TR8350.2's published accuracy, which `convert` reports, and Pulkovo-1942, which has none, reports infinity) |
| C18 | `along_track_distance_spherical` returned `Ok(0 m)` for a point at a pole of the path's great circle, where every foot is equally near; `wrap_longitude(-0.0)` and `Coordinate::new(-0.0, -0.0, ..)` kept `-0.0`; a GCJ-02 / BD-09 `Crs` *argument* to `Ecef::try_to_coordinate` / `DatumTransform::new` was `CrsMismatch { expected: Wgs84 }`, though no coordinate was on the wrong datum and NAD27 would serve as well; the geodesic, rhumb and spherical `# Errors` listed an `OutOfRange` the sealed, valid-by-construction `LatLon` inputs can never produce | `src/geodesy/spherical.rs` (`along_track_distance_spherical`), `src/angle.rs` (`wrap_longitude`), `src/coord.rs` (`Coordinate::new`), `src/geodesy/ecef.rs` (`Ecef::try_to_coordinate`), `src/geodesy/datum.rs` (`DatumTransform::new`), `src/geodesy/geodesic.rs` (`validate_pair`) | #173 / #193 | closed (#193: the path's pole is `InvalidValue { field: "point" }`, and along-track is computed from n-vectors; a zero is always `+0.0`; a `Crs` argument with no ellipsoid is `InvalidValue { field: "crs" \| "from_crs" \| "to_crs" }`, while a coordinate carrying one stays `CrsMismatch`; the dead range check and its docs are gone) |
| C19 | `Enu`, `Ned` and `Aer` had public fields, so a NaN component, an elevation above 90° or a negative range could be built or deserialized; `Aer::try_to_coordinate` reported a bad azimuth as `field: "ENU"`; the frames rejected every datum but WGS-84, though `Ecef` handles every classic datum since C15 | `src/geodesy/frames.rs` (`Enu`, `Ned`, `Aer`, `validate_frame_coordinate`); ffi `src/geodesy.rs` | #179 / #199 | closed (#199: private fields behind `Enu::new` / `Ned::new` / `Aer::new`, getters and serde `try_from`; finite components with a finite length, azimuth normalized to `[0, 360)`, elevation in `[-90, 90]`, range a finite non-negative `Length`; each error names its field; any classic datum the origin and target share, with a mismatch or GCJ-02 / BD-09 a `CrsMismatch`; FFI `enu_validate` / `ned_validate` / `aer_validate` and the normalizing `aer_new`, and every export taking a frame record validates it) |
| G24 | Coordinates crossed public signatures three ways: measurements took `&impl LatLon`, `destination` / `midpoint` / `intermediate` / `rhumb_destination` / both intersections / `format` / `utm_zone_for` took `&Coordinate`, and encoders, frames and `convert` took `Coordinate` by value; `china::out_of_china(lat, lon)` took a bare, swappable `f64` pair and called `(NaN, NaN)` outside China; the `LatLon` docs promised formatters over any positioned type, though `format` takes only a `Coordinate` | `src/geodesy/{geodesic,rhumb,spherical,track}.rs`, `src/format/mod.rs` (`format`), `src/grids/utm.rs` (`utm_zone_for`), `src/china/mod.rs` (`out_of_china`), `src/coord.rs` (`LatLon`); ffi `src/china.rs` | #176 / #208 | closed (#208: `Coordinate` is passed by value in every canonical signature and the `LatLon` measurements take `impl LatLon` by value; `out_of_china` is replaced by `Wgs84::is_outside_china` / `Gcj02::is_outside_china`, mirrored as the validating `wgs84_is_outside_china` / `gcj02_is_outside_china`; the `LatLon` docs name the geodesy measurements only and send `format` callers through `to_coordinate`) |
| G25 | `FormatOptions` and `ParseOptions` were exhaustive structs with public fields, so no field (such as a datum on `ParseOptions`) could be added after 1.0; `Representation` accepted DMS second decimals up to 15 and DDM minute decimals up to 15, though past 9 and 12 the last digit drifts by one through `format → parse → format`; `Hemisphere`, `Axis`, `AxisOrder`, `DatumAmbiguity`, `LengthUnit`, `UtmHemisphere`, `SymbolStyle`, `HemisphereStyle`, `Representation` and both option structs implemented `Eq` but not `Hash`; `Height::meters` was `pub(crate)` and `LengthUnit::meters_per_unit` private | `src/format/mod.rs` (`FormatOptions`, `Representation`, `format`), `src/parse/options.rs` (`ParseOptions`), `src/angle.rs`, `src/fix.rs`, `src/grids/utm.rs`, `src/units.rs` (`Height`, `LengthUnit`); ffi `src/core.rs`, `src/text.rs` | #182 / #210 | closed (#210: both option structs `#[non_exhaustive]` with `Default`, `const` `with_*` builders and `#[serde(default)]`; `Representation::MAX_DD_DECIMALS` 15 / `MAX_DMS_SECOND_DECIMALS` 9 / `MAX_DDM_MINUTE_DECIMALS` 12, a count above its cap rejected by `format` with `InvalidValue`, and the round-trip property run at each cap; `Hash` wherever `Eq` holds; `Height` moves to `units` with a public `meters() -> Length`, and `meters_per_unit` is public; the FFI records stay plain data and add `height_meters` and `length_unit_meters_per_unit`) |
| G26 | `Utm::new` / `Ups::new` checked only numeric ranges, not the forward image: `Ups::new(North, 0.0, 0.0)` decoded near 64°N and `Utm::new(18, North, 500_000.0, 9_900_000.0)` near 89°N, neither of which projects back; zone 1's 150 km easting decoded into zone 60; and MGRS parsing accepted squares wholly outside their grid-zone designator (`AJA`, around 75°S; `32XNJ`; high-latitude columns outside the zone), decoding them to points that re-encode as another reference. Separately, `utm_zone_for` rounded a longitude a few ulps west of a zone meridian into the zone east of it | `src/grids/utm.rs` (`Utm::new`, `Utm::to_coordinate`, `zone_in_band`), `src/grids/utm/ups.rs` (`Ups::new`, `Ups::to_coordinate`), `src/grids/mgrs.rs` (`Mgrs::parse`), `src/grids/mgrs/clip.rs` (`settle`) | #183 / #211 | closed (#211: `new` accepts only a position whose decoded point the forward projection assigns to the same zone and hemisphere, else `InvalidValue` naming `"easting"` or `"northing"`, with 1e-9° of slack so every forward result stays constructible and serde stays lossless; `to_coordinate` moves a point within that slack onto the image, so zone 60's 180° edge decodes just west of 180°; `Mgrs::parse` rejects a square holding no point of its designator with `InvalidGridRef`; the zone index is checked against the meridian itself; property tests run every constructible `Utm` / `Ups` and every parseable `Mgrs` through the forward direction) |
| G27 | The FFI surface followed no single naming scheme: the readers `from_geojson`, `from_wkt`, `from_gpx`, `from_kml`, `from_nmea_sentence` and `from_geo_uri` became bare top-level `fromGeojson` and so on beside `parse_coordinate`, and `parse_text_with` had no `parse_text`; lengths carried `_m` but the bearings, `wrap_longitude` and `clamp_latitude` returned degrees with no `_deg`; `Utm`, `Ups`, `H3Cell`, `S2CellId`, `Ellipsoid` and `DatumTransform` had no `*_validate`; `Coordinate::from_lon_lat` had no FFI form; 28 fallible exports (every `h3_*` / `s2_*`, the geodesics, the track routines, the frame conversions) had no `# Errors`; the idiom table omitted `ApproxCoordinate`, and the root README pointed at an idiom explanation the bindings README did not hold | ffi `src/interchange.rs`, `src/text.rs` (`parse_geo_uri`, `parse_text`), `src/geodesy.rs` (`initial_bearing_deg`, `final_bearing_deg`, `rhumb_bearing_deg`, `ellipsoid_validate`), `src/geodesy/track.rs`, `src/angle.rs` (`wrap_longitude_deg`, `clamp_latitude_deg`), `src/grids.rs` (`utm_validate`, `ups_validate`), `src/dgg.rs` (`h3_validate`, `s2_validate`), `src/datum.rs` (`datum_transform_validate`), `src/core.rs` (`coordinate_from_lon_lat`), `src/lib.rs`; `crates/geocoordinates-ffi/README.md` | #185 / #212 | closed (#212: readers renamed `parse_*`, every returned angle suffixed `_deg`, a `*_validate` for every invariant record, `coordinate_from_lon_lat` and `parse_text` added, `# Errors` on every fallible export enforced by `tests/export_coverage.rs`, and the naming scheme and idiom table in the crate docs and the bindings README) |
| G28 | Items were reachable at two or three public paths: `coord`, `approx`, `units` and `error` were `pub` and re-exported at the root, every `geodesy` and `grids` submodule was `pub` beside its re-exports (`geodesy::geodesic` even re-exported the rhumb and spherical routines), and `AxisOrder` sat at `fix::`, `parse::` and `prelude::`; the prelude glob-exported `Result<T>`, so `fn main() -> Result<(), Box<dyn std::error::Error>>` failed after `use geocoordinates::prelude::*;`; `parse::text::parse` / `parse_with` duplicated `parse_coordinate_with` and `parse_with` silently ignored `ParseOptions::expect_grid`; the crate doc's conversion rule contradicted `Dms::from_degrees` and the grid parsers | `src/lib.rs` (the root re-exports, `prelude`), `src/error.rs` (`Result`), `src/geodesy/mod.rs`, `src/grids/mod.rs`, `src/parse/mod.rs`, `src/parse/text/mod.rs` (`parse_text`, `parse_text_with`), `scripts/check_public_paths.py` | #168 / #187 | closed (#187: implementation modules are private and every item is re-exported flat from the crate root or one subsystem module, at exactly one path besides its prelude entry, which `mise run public-api-check` enforces in CI; `pub type Result<T, E = Error>`, no longer in the prelude, and a prelude doctest pins `Result<(), Box<dyn std::error::Error>>`; `utm::zone_for` / `utm::central_meridian_deg` became `grids::utm_zone_for` / `grids::utm_central_meridian_deg`; `parse::parse_text` / `parse_text_with` replace `parse::text::*`, and `parse_text_with` rejects `expect_grid` with `InvalidValue { field: "expect_grid" }`; `AGENTS.md` and the crate doc state the constructor verbs, `Length` for magnitudes, the `OutOfRange` pair rule, `Coordinate` by value and `#[non_exhaustive]` option structs) |
| G29 | docs.rs would have omitted every feature-gated item (`dgg`, `parse::interchange`, `parse::sensors`): `Cargo.toml` had no `[package.metadata.docs.rs]` and `default = []`, and no item used `doc(cfg)`; no CI job built the crate with no features or with any single feature; the `exclude` list shipped `.github/`, `.mise.toml`, `hk.pkl` and the other contributor files, while the crate README linked to files the package left out | `Cargo.toml` (`include`, `[package.metadata.docs.rs]`), `src/lib.rs`, `src/dgg/mod.rs`, `src/parse/mod.rs` (the `doc(cfg)` attributes), `.mise.toml` (`features`), `.github/workflows/ci.yml` (`Feature matrix`), `README.md` | #177 / #197 | closed (#197: docs.rs builds with all features and `--cfg docsrs`, and every gated item carries a feature badge; an `include` allow-list ships only the sources, licenses, README and CHANGELOG; `mise run features` checks no features and each feature alone with cargo-hack, then tests with no features, as a CI job; every README link is absolute) |
| G30 | Binding artifacts did not carry the release's version (the FFI crate stuck at 0.14.0, `package.json` and the Gradle build at 0.1.0); the npm package pointed `main` / `types` at raw TypeScript that plain Node cannot import; `pyproject.toml` required Python 3.8 and described only the China datums; and the release PR ran no packaging check after it was first opened, while its header claimed "API compatible changes" | `Cargo.toml` (`[workspace.package] version`), `crates/geocoordinates-ffi/Cargo.toml`, `crates/geocoordinates-ffi/web/package.json`, `crates/geocoordinates-ffi/web/src/index.ts`, `crates/geocoordinates-ffi/pyproject.toml`, `crates/geocoordinates-ffi/jvm/build.gradle.kts`, `release-plz.toml` (`semver_check`), `.github/workflows/release-plz.yml`, `.mise.toml` (`version-check`, `version-sync`) | #184 / #200 | closed (#200: one `[workspace.package]` version that both crates inherit and every binding reads or is stamped with, held by `mise run version-check` in CI; the npm package compiles to `dist/` with an `exports` map, and CI imports the packed tarball from plain Node; `pyproject.toml` requires Python 3.9 and describes the full surface; the release PR dispatches CI, FFI and the binding dry runs whenever it is created or updated, and `semver_check = false` drops the misleading header, leaving the changelog's `[**breaking**]` markers as the record) |
| G31 | The JVM release uploaded each deployment to the Sonatype Central Portal without requesting its release, so no version reached Maven Central although every run reported success | `crates/geocoordinates-ffi/jvm/build.gradle.kts` (`publishToMavenCentral`), `.github/workflows/release-jvm.yml`, `PUBLISHING.md` | #131 / #204 | closed (#204: `publishToMavenCentral(SonatypeHost.CENTRAL_PORTAL, automaticRelease = true)`, so the portal publishes a deployment once it validates; the deployments of v0.14.2 and earlier stay pending in the portal for a maintainer to publish or drop, as `PUBLISHING.md` says, and the README's † note stays until a release resolves on Maven Central) |
| C20 | The free-text parser rewrote an uppercase `S` after a seconds number into the seconds unit, so `"10d30m15S, 20d10m5E"` came back as 10.504° **North** with confidence 0.9 | `src/parse/text/normalize.rs` (`rewrite_unit_letters`) | #170 / #188 | closed (#188: only a lowercase `s` after a seconds number is the unit, as `SymbolStyle::Letters` writes it; an uppercase `S` stays a hemisphere letter, so the example parses as South with confidence 1.0) |
| C21 | Untrusted interchange input was unbounded: a GeoJSON position with fewer than two numbers was silently dropped, shifting every later index, and WKT parentheses and KML elements nested without limit through recursive walks, so a deep document could overflow the stack | `src/parse/interchange.rs` (`from_geojson`, `from_wkt`, `MAX_WKT_DEPTH`), `src/parse/interchange/xml_scan.rs` (`MAX_KML_DEPTH`) | #171 / #189 | closed (#189: a short GeoJSON position is an `Error::Parse` naming its true index; WKT deeper than 128 levels and KML deeper than 32 are rejected by a pre-scan, now `ParseErrorKind::TooDeep` since G23; the crate's own geometry walks use an explicit stack, and 100 000-deep inputs return `Err` promptly) |
| C22 | `Gcj02::to_wgs84_fast` claimed its 6 m bound even where the step crossed the GCJ-02 box edge (`Gcj02::new(40.0, 72.006)`: forward image about 340 m off); `convert` routed BD-09 → GCJ-02 through the WGS-84 hub, doubling the bound and picking up the GCJ-02 inverse's residual near the box edge; and BD-09 had no fast inverse to WGS-84 | `src/china/gcj02.rs` (`Gcj02::to_wgs84_fast`, `fast_bound_m`), `src/convert.rs` (`convert`), `src/china/bd09.rs` (`Bd09::try_to_wgs84_fast`); ffi `src/china.rs` (`bd09_to_wgs84_fast`) | #172 / #191 | closed (#191: the fast GCJ-02 inverse checks its candidate once through the forward offset and reports 6 m plus any residual beyond it; `convert` takes the direct `Bd09::try_to_gcj02_refined` path, bounded at 0.5 m; `Bd09::try_to_wgs84_fast` composes the two fast inverses and sums their bounds, mirrored as `bd09_to_wgs84_fast`) |
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
- `Crs`, `LengthUnit`, `DatumAmbiguity` and `AccuracyKind` are exhaustive on purpose, each with a
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
| #168 | #187 | One canonical path per public item; `Result` out of the prelude | G28 |
| #170 | #188 | An uppercase `S` after a seconds number reads as South | C20 |
| #174 | #190 | Honest `Approx` API and `Length` for every public magnitude | G19 |
| #171 | #189 | Short GeoJSON positions rejected; WKT / KML nesting bounded | C21 |
| #175 | #192 | Rhumb bearing from a pole inverts with `rhumb_destination` | C7b |
| #172 | #191 | Measured China inverse bounds, direct BD-09 → GCJ-02, fast BD-09 inverse | C22 |
| #169 | #194 | MGRS encodes only references its decoder accepts | C8b |
| #173 | #193 | Honest errors for along-track at the path pole and bad `Crs` arguments; no `-0.0` | C18 |
| #178 | #195 | Validated, kind-tagged `Accuracy` and an extensible `Fix` model | G20 |
| #177 | #197 | docs.rs all features, `include` allow-list, per-feature CI | G29 |
| #149 | #196 | MGRS decodes a truncated square inside its grid-zone designator | G18 |
| #179 | #199 | Local frames valid by construction on any classic datum | C19 |
| #184 | #200 | One workspace version, compiled npm package, release-PR packaging runs | G30 |
| #180 | #201 | Inherent `parse` on grid codes, `Hash` on ids, H3 / S2 level getters | G21 |
| #130 | #202 | Ellipsoidal cross-track, along-track and intersection | G22 |
| #181 | #203 | Structured parse errors with `ParseErrorKind` | G23 |
| #131 | #204 | Maven Central deployments released automatically | G31 |
| #198 | #207 | MGRS keeps a point west of a central meridian in the square west of it | G18b |
| #135 | #206 | Unclosed KML rejected instead of hanging the `kml` reader | G17 |
| #176 | #208 | Uniform argument conventions: `Coordinate` by value, `is_outside_china` | G24 |
| #182 | #210 | Non-exhaustive option builders, round-trip decimal caps, `Hash` derives | G25 |
| #205 | #209 | A KML `Point` with no coordinates rejected instead of panicking | G17b |
| #183 | #211 | `Utm::new`, `Ups::new` and `Mgrs::parse` accept only the forward image | G26 |
| #185 | #212 | FFI naming normalized; validation, capability and docs gaps closed | G27 |
| #186 | #213 | Reconcile this ledger, README, `AGENTS.md` and the changelog with the audited surface | — |

## 0.15.0 gate checklist

0.15.0 is not cut until all of these hold. Evidence is from `master@8a55fcb`
(2026-10-17), the surface the release PR (#133) ships once the pre-1.0 audit
has landed:

- [x] Every ledger finding above is closed: no row is open and none is
  deferred. G17 (#135) and G18 (#149), deferred to a patch release at
  `4dcd177`, closed inside 0.15.0 (#206, #196), together with the G17b and
  G18b defects found while fixing them.
- [x] `mise run check` and `mise run ffi-check` green — CI run 39014700571 at
  `8a55fcb`, and CI run 39013401942 and FFI run 39013401997 at #212's head
  `ef195ff`, whose tree is identical to `8a55fcb`.
- [x] `cargo test --all-features --doc` executes a non-zero number of doctests —
  299 pass (295 examples and 4 `compile_fail` pins).
- [x] `mise run bindings` regenerates cleanly for all four languages — the FFI
  run above generates Python, Kotlin and Swift (and checks the committed Swift
  bindings are current) and builds the WASM / TypeScript package, and each
  language's tests pass.
- [x] `cargo run --all-features --example flagship` passes and exercises the
  full surface — "All 8 shipped Rust capability groups passed."
- [x] Coverage threshold enabled in CI and met — a 98% line floor (measured 98.85%)
  in `.coverage-baseline`, held shrink-only by `mise run coverage-ratchet` (#128).
- [x] `cargo semver-checks` output reconciled against the release PR's
  changelog — every break recorded; see below.

### `cargo semver-checks` against 0.14.2

`cargo semver-checks -p geocoordinates --baseline-version 0.14.2 --all-features`
(cargo-semver-checks 0.51.0, at `8a55fcb`) fails 22 major checks and no minor
ones. Every break traces to a pull request whose commit carries `!`, which the
release PR's 0.15.0 changelog lists as **breaking**:

| Break (lint) | Items | PR |
|---|---|---|
| `enum_variant_missing` | `Error::UnsupportedConversion`, `GeoidDataUnavailable`, `FeatureDisabled` | #143 |
| `struct_pub_field_missing`, `struct_pub_field_now_doc_hidden`, `inherent_method_missing`, `trait_newly_sealed`, `trait_added_supertrait` (`Sealed`) | `Coordinate` fields, `Coordinate::validate`; `LatLon` | #144 |
| `struct_pub_field_missing`, `struct_pub_field_now_doc_hidden`, `inherent_method_missing`, `enum_missing` | `Utm` / `Ups` fields, `try_to_coordinate`; `utm::Hemisphere` (now `UtmHemisphere`) | #145 |
| `struct_missing`, `struct_pub_field_missing`, `struct_pub_field_now_doc_hidden`, `inherent_method_missing` | `Dd` (also in the prelude); `Dms` / `Ddm` fields, `validate`, `try_to_*` | #146 |
| `inherent_method_missing` | `Mgrs::precision_m` | #148 |
| `copy_impl_added`, `struct_pub_field_missing`, `inherent_method_missing`, `enum_unit_variant_changed_kind`, `enum_variant_added`, `enum_discriminants_undefined_non_unit_variant` | `FormatOptions` fields, `uses_decimal_comma`; `Representation` (`Geohash`, `Maidenhead`, `Mgrs` added) | #150 |
| `struct_pub_field_missing`, `struct_pub_field_now_doc_hidden`, `inherent_method_missing`, `function_missing` | `Wgs84` / `Gcj02` / `Bd09` / `BaiduMercator` fields, `validate`, `try_to_*`; `can_convert` (also in the prelude) | #151 |
| `struct_pub_field_missing`, `struct_pub_field_now_doc_hidden`, `constructible_struct_adds_private_field`, `inherent_method_missing`, `method_parameter_count_changed` | `Ellipsoid` and `DatumTransform` fields, `Ellipsoid::validate`, `Helmert::inverse`, `DatumTransform::{to_wgs84, transform, inverse}`; `Ecef::try_{from,to}_coordinate` | #152 |
| `struct_missing` | `parse::text::TextParseOptions` (now `parse::ParseOptions`) | #154 |
| `function_parameter_count_changed` | `from_wkt` | #155 |
| `struct_pub_field_missing`, `struct_pub_field_now_doc_hidden` | `H3Cell.0`, `S2CellId.0` | #160 |
| `module_missing`, `struct_missing`, `enum_missing`, `trait_missing`, `function_missing` | the modules `coord`, `approx`, `units`, `error`, `geodesy::{geodesic, ecef, ellipsoid, frames, datum}`, `grids::{encoded, mgrs, utm}` and `parse::text`, with every item reachable only through them (`geodesy::geodesic::*`, `grids::utm::{zone_for, central_meridian_deg}`, `parse::text::{parse, parse_with}`); the root re-exports of `Wgs84`, `Gcj02`, `Bd09`, `BaiduMercator`, `Fix`, `Accuracy`, `RawSource`, `Confidence`; `parse::AxisOrder` | #187 |
| `constructible_struct_adds_field`, `constructible_struct_adds_private_field`, `struct_pub_field_missing`, `inherent_method_missing`, `enum_variant_added` | `Ecef.{x,y,z}` (now `x_m` / `y_m` / `z_m`); the `Enu` / `Ned` / `Aer` and `Accuracy` fields and the private `Ellipsoid.semi_major` / `DatumTransform.accuracy` renamed or retyped to `Length`; `Approx::max_error_m`, `Ellipsoid::semi_minor_m`; `LengthUnit::{Mile, Yard}` | #190 |
| `constructible_struct_adds_private_field`, `struct_marked_non_exhaustive` | `Accuracy` fields; `Fix`, `RawSource` | #195 |
| `constructible_struct_adds_private_field`, `struct_pub_field_now_doc_hidden` | `Enu` / `Ned` / `Aer` fields | #199 |
| `enum_tuple_variant_changed_kind` | `Error::Parse` | #203 |
| `function_missing`, `trait_added_supertrait` (`Copy`), `trait_no_longer_dyn_compatible` | `china::out_of_china`; `LatLon` | #208 |
| `struct_marked_non_exhaustive` | `FormatOptions` | #210 |

The 0.15.0 baseline at `4dcd177` also listed `intersection`,
`cross_track_distance` and `along_track_distance` as missing (#156 renamed them
`*_spherical`). #202 brought those names back at `geodesy::`, now measured on
the datum ellipsoid, so the tool no longer flags them; the change it cannot
see is still breaking and still marked so.

The breaks the tool cannot see are also marked breaking there: behaviour
(#157 rejecting no-fix NMEA; #162 moving rhumb lines onto the ellipsoid; #192
reading a rhumb bearing at a pole against the start's meridian; #193 returning
`InvalidValue` for the path pole and for a `Crs` argument with no ellipsoid;
#202 measuring `cross_track_distance` / `along_track_distance` /
`intersection` on the datum ellipsoid and rejecting GCJ-02 / BD-09; #203
reporting an out-of-range GeoJSON / WKT / KML position as `Error::OutOfRange`
rather than `Error::Parse`; #211 rejecting `Utm` / `Ups` positions and MGRS
squares outside the forward image) and signature types the lint set does not
compare (#208 taking `impl LatLon` and `Coordinate` by value where 0.14.2 took
references). #212 renames FFI exports only, which no Rust caller sees; it is
marked breaking for the bindings.

The fixes that change a result without breaking a contract are not marked
breaking: #188 (an uppercase `S` after seconds reads as South), #189 (a short
GeoJSON position, once silently dropped, is an error), #191 (a GCJ-02 fast
bound that grows at the box edge, and `convert`'s direct BD-09 → GCJ-02 path),
#194, #196 and #207 (MGRS references that now round-trip), and #206 / #209
(KML that hung or panicked now returns an error). Each corrects an output the
0.14.2 documentation did not promise; #201 only adds items.

Since #200 set `semver_check = false` in `release-plz.toml`, the release PR
carries no semver header at all; the changelog's `[**breaking**]` markers, which
come from the Conventional Commit `!`, are the record. Running
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
- **`tz`** — timezone from coordinates, behind `tzf-rs` or similar. An open
  question from #220, not admitted and not reserved in `Cargo.toml`. Capsule
  depends on `tzf-rs` directly today, so this consumer is what a decision would
  weigh; admitting it takes its own issue.

`proj` and `geoid` have no source in-tree: #140 deleted the `todo!()` stubs (git
history keeps them). Each survives only as a commented-out Cargo feature, so neither can
be reached from the released surface.

## Out of scope

- **Container / byte parsing of JPEG, TIFF and HEIF** — gamut-exif, rawshift and
  kamadak-exif own reading EXIF/XMP out of a file. This crate starts from tag values
  already read (the `exif` feature on the roadmap below).
- **Polygon / line geometry ops** (area, point-in-polygon, simplification, hull) —
  use [`geo`](https://crates.io/crates/geo) directly. Geographic bounds and the
  centroid of a set of points are not polygon ops, and are on the roadmap below.
- **Rendering static map images** — the `map` feature computes view math and
  GeoJSON layers; clients draw them.

## Roadmap — photo-library geo

#220 admitted the geo features a photo library needs, on the evidence of a concrete
consumer: Capsule reads GPS by hand and has no index, clusters or offline place names,
and gamut-exif already bridges its `GpsInfo` to `Coordinate`.

0.15.0 (#133) ships as-is. These features land additively after it, so the 0.15
surface is not reopened. Each is its own cargo feature, off by default, with its own
per-feature CI job. Until one lands, its name is only a commented-out placeholder in
`Cargo.toml`, so none can be reached from the released surface.

| Issue | Feature | Capability | Blocked by |
|---|---|---|---|
| #221 | `exif` | Already-read EXIF/XMP GPS tag values to a `Fix` | — |
| #222 | `index` | Antimeridian-aware geographic bounds; spatial index queries | — |
| #223 | `reverse`, `reverse-bundled` | Offline reverse geocoding behind a trait; `reverse-bundled` embeds the data | #222 |
| #224 | `cluster` | Geodesic centroid and point clustering | #222 |
| #225 | `map` | Web Mercator, tile and viewport math with GeoJSON layer output | #222, #224 |
| #226 | `geotag` | Track-based geotagging | #221 |
| #227 | `privacy` | Deterministic location coarsening and exclusion zones | #222 |

### `exif`: what real files showed

`exif` is implemented on master. `src/exif/device_tests.rs` transcribes GPS IFDs from
`drewnoakes/metadata-extractor-images` at `7b56b64` (iPhone 5, 6 and 6 Plus, Pixel 2,
DJI Phantom 4, Oppo R7 Plus, OnePlus 8). Three of them shaped the rules: the iPhone 6
stores the date `2014:09:259`, so an unreadable date gives no timestamp rather than
losing the fix; the OnePlus 8 writes an all-`0/0` block for "no fix", which reads as
`NoFix`; the Pixel 2 has a DOP alone, and because `Accuracy` needs a magnitude, a DOP
stays in `CaptureGeo::dop` with no `Accuracy`. Open gap: that corpus has no
Chinese-market file taken inside China, so the `PossiblyGcj02` flag is pinned by
constructed values only.

### `index`: the backing tree

`index` is implemented on master, and #222 chose an in-crate k-d tree over unit vectors
(`src/index/kdtree.rs`) over `rstar` and `kiddo`. `benches/compare-backends` runs all
three on the same points and queries, each prefiltering by chord and refining with one
Karney geodesic, and asserts that their answers agree. Timings below are one release
run on one machine: 1,000 queries of uniform points on the sphere, read for the gap
between backends and not as absolute numbers.

| Backend | Points | Build | `within` 50 km, per query | `nearest` 10, per query |
|---|---|---|---|---|
| in-crate k-d tree | 10⁵ | 22.7 ms | 1.69 µs | 8.56 µs |
| `rstar` 0.12 | 10⁵ | 29.3 ms | 1.89 µs | 9.55 µs |
| `kiddo` 5 (immutable) | 10⁵ | 6.7 ms | 1.39 µs | 13.95 µs |
| in-crate k-d tree | 10⁶ | 225 ms | 14.96 µs | 10.06 µs |
| `rstar` 0.12 | 10⁶ | 297 ms | 12.25 µs | 10.78 µs |
| `kiddo` 5 (immutable) | 10⁶ | 109 ms | 11.29 µs | 13.99 µs |

No backend wins on every column, and the query gaps are small because the geodesic
refinement dominates a query. So the choice falls to what each one adds:

| Backend | Crates added (`cargo tree`) | Declared `rust-version` | Licence |
|---|---|---|---|
| in-crate k-d tree | 0 | the crate's own, 1.85 | the crate's own, MIT OR Apache-2.0 |
| `rstar` 0.12 | 8 | 1.63 | MIT OR Apache-2.0 |
| `kiddo` 5 | 43 | 1.85 | MIT OR Apache-2.0 |

None changes the licence or raises the MSRV, so `feature-rust-version` has no `index`
entry. The tree stays private behind `SpatialIndex`, so swapping it later is not a
breaking change. `cargo bench --features index --bench spatial_index` times the shipped
index at both sizes against a brute-force scan.

## After 0.15.0 — the 1.0 freeze

Once the surface has survived dogfooding, freeze the public API and adopt strict
semver: no breaking changes without a major bump, `cargo semver-checks` enforced
as a required check, and the coverage threshold ratcheted rather than relaxed.
