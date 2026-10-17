# geocoordinates (language bindings)

[UniFFI](https://mozilla.github.io/uniffi-rs/) bindings for the
[`geocoordinates`](https://crates.io/crates/geocoordinates) crate — low-level
geospatial coordinate primitives: China datums (GCJ-02/BD-09), angle encodings,
coordinate parsing/formatting, and geodesy utilities.

The Rust API is exposed across the FFI boundary to **Python, Kotlin, Swift, and
TypeScript** (Java consumes the Kotlin/JVM artifact) with **full capability
parity**: every public capability has one canonical FFI-expressible form,
though not every Rust-side signature crosses.
Generics, traits, operator overloads, and `From`/`TryFrom` conversions do not
cross FFI, so they are re-expressed as flat records and free functions;
approximate inverses keep their `_fast` / `_refined` names and carry an
explicit `max_error_m`.

## How each Rust idiom is re-expressed

| Rust idiom | FFI form |
|---|---|
| `Approx<T>` (generic wrapper) | flat records `ApproxWgs84` / `ApproxGcj02` (the China inverses) and `ApproxCoordinate` (`convert` and every grid / cell decoder), each carrying `max_error_m` |
| `LatLon` trait / `impl LatLon` | functions take the concrete `Coordinate` record |
| `Wgs84::is_outside_china` / `Gcj02::is_outside_china` methods | `wgs84_is_outside_china` / `gcj02_is_outside_china` |
| `Length` + operator overloads | lengths as `f64` meters (`haversine_distance_m`) |
| `From` / `TryFrom` conversions | named free functions (`wgs84_to_gcj02`, …) |
| Inherent methods and constructors | free functions prefixed with the type (`coordinate_new`, `coordinate_from_lon_lat`, `utm_to_coordinate`, …) |
| A type valid by construction | a plain record, checked as it crosses into Rust, with a `<record>_validate` function to check a hand-built one |

Exact forward conversions return a bare datum record; approximate inverses
return an `Approx*` record and keep the `_fast` / `_refined` suffix.

## Naming

Every export follows one scheme, so a name reads the same in every language
(the generators only change its case, e.g. `parse_geojson` → `parseGeojson`):

- **Units in the name.** A function returning a bare length names its unit
  (`geodesic_distance_m`, `height_meters`), and one returning an angle ends in
  `_deg` (`initial_bearing_deg`, `final_bearing_deg`, `rhumb_bearing_deg`,
  `wrap_longitude_deg`, `clamp_latitude_deg`, `utm_central_meridian_deg`) or
  names degrees as its target (`dms_to_degrees`, `normalize_degrees`). Length
  and angle parameters carry the same suffix (`distance_m`, `bearing_deg`).
- **Ingestion is `parse_*`.** `parse_coordinate`, `parse_coordinate_with`,
  `parse_text`, `parse_text_with`, `parse_geo_uri`, `parse_geojson`,
  `parse_wkt`, `parse_gpx`, `parse_kml` and `parse_nmea_sentence` each read
  text into a `Fix` (or a list of them). The core crate's readers are named
  `from_geojson` and so on inside a module that says what they produce; a
  top-level foreign function has no such module.
- **`<record>_validate` for every invariant record.** `Coordinate`, `Wgs84`,
  `Gcj02`, `Bd09`, `BaiduMercator`, `Dms`, `Ddm`, `Ellipsoid`, `Enu`, `Ned`,
  `Aer`, `DatumTransform`, `Utm`, `Ups`, `H3Cell` (`h3_validate`), `S2CellId`
  (`s2_validate`), `Accuracy` and `Confidence` each have one. No
  `<record>_new` is added that would only copy its arguments in and validate
  them; one exists where construction derives or normalizes a field
  (`aer_new`, `raw_source_new`), beside the constructors that predate that
  rule (`coordinate_new`, `ellipsoid_new`, `accuracy_new`, `confidence_new`,
  `datum_transform_new`).
- **`<grid>_canonicalize` for grid codes.** `plus_code_canonicalize`,
  `geohash_canonicalize`, `maidenhead_canonicalize` and `mgrs_canonicalize`
  validate a code and return its canonical form.
- **Errors.** Every function that can fail throws the one `GeoError` type and
  documents which variants it raises.

## Surface

Tracks the core crate's public surface, subsystem by subsystem. See the root
[`README`'s API surface table](https://github.com/justin13888/geocoordinates-rs/blob/master/README.md#api-surface)
for what's covered and the Rust/FFI parity of each subsystem against the
stabilization bar in
[`STABILIZATION.md`](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md).

```python
import geocoordinates_ffi as gc

gcj = gc.wgs84_to_gcj02(gc.Wgs84(lat=39.915, lon=116.404))
wgs = gc.gcj02_to_wgs84_refined(gcj)          # approximate inverse
print(wgs.lat, wgs.lon, "±", wgs.max_error_m, "m")
```

The WebAssembly/TypeScript bindings live in [`web/`](https://github.com/justin13888/geocoordinates-rs/tree/master/crates/geocoordinates-ffi/web).
