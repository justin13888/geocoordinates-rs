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
| `Approx<T>` (generic wrapper) | flat records `ApproxWgs84` / `ApproxGcj02` (the China inverses) and `ApproxCoordinate` (`convert` and every grid / cell decoder), each carrying `max_error_m`; output-only, so `Approx::new` is Rust-only |
| `LatLon` trait / `impl LatLon` | functions take the concrete `Coordinate` record |
| `Wgs84::is_outside_china` / `Gcj02::is_outside_china` methods | `wgs84_is_outside_china` / `gcj02_is_outside_china` |
| `Length` + operator overloads | lengths as `f64` meters (`haversine_distance_m`) |
| `From` / `TryFrom` conversions | named free functions (`wgs84_to_gcj02`, …) |
| Inherent methods and constructors | free functions prefixed with the type (`coordinate_new`, `coordinate_from_lon_lat`, `utm_to_coordinate`, `utm_zone_for` for `Utm::zone_for`, …) |
| `Coordinate::try_to_crs` | `convert`, the export's name from before the method existed |
| `ParseOptions::for_format` | `format_options_parse_options`, the export's name from before the method moved to `ParseOptions` |
| `CaptureGeo::from_exif` / `from_exif_with` / `from_xmp` / `from_xmp_with` | `parse_exif` / `parse_exif_with` / `parse_xmp` / `parse_xmp_with`, named like the other readers |
| `Height::length` | `height_meters`, the height as `f64` meters |
| `SpatialIndex<T>` (generic, holds its entries) | the object `SpatialIndex` over coordinates, whose results carry each entry's build-list position for the caller's own payload lookup |
| `ClusterIndex` (`insert` / `remove` take `&mut self`) | the object `ClusterIndex`, whose updates take a lock |
| `Approx<Coordinate>` inside a cluster | flat `centroid` and `centroid_max_error_m` fields on `Cluster` and `MapCluster` |
| `[ExifRational; 3]` (a tag of three rationals) | the named records `ExifDms` and `ExifTime` |
| `ReverseGeocoder` trait | the foreign trait `ReverseGeocoder`: Swift, Kotlin, Python or TypeScript code implements it (a `CLGeocoder` adapter, say), `reverse_geocode` calls any implementation and checks the `Place` it returns, and `gazetteer_as_reverse_geocoder` passes a `Gazetteer` as one |
| `Approx<Coordinate>` inside a `Place` | flat `query` and `query_max_error_m` fields |
| `Gazetteer::bundled` (`reverse-bundled`) | none: the bindings do not compile the 6 MB dataset in; load a gazetteer file with `Gazetteer.from_bytes` or `Gazetteer.open` |
| `map::clusters_to_geojson` (takes checked `MapCluster`s) | `clusters_in_view_geojson`, the view query and the layer in one call, since a `MapCluster` record cannot become a checked core cluster again |
| `map::MAX_LATITUDE` / `map::MAX_TILE_ZOOM` (constants) | `web_mercator_max_latitude_deg` / `tile_max_zoom` |
| `geotag::Track` (holds its fixes) | the object `Track` |
| `geotag::estimate_offset` (takes `(SystemTime, Coordinate)` tuples) | `estimate_offset_ms`, over `GeotagReference` records |
| `privacy::apply` (a name too generic for a flat namespace) | `apply_exclusion_zones` |
| `privacy::ExclusionZone` (constructors over a private shape) | the enum `ExclusionZone` with `Circle` and `Bounds` variants, checked by `exclusion_zone_validate` |
| A type valid by construction | a plain record, checked as it crosses into Rust, with a `<record>_validate` function to check a hand-built one |

Exact forward conversions return a bare datum record; approximate inverses
return an `Approx*` record and keep the `_fast` / `_refined` suffix.

## Naming

Every export follows one scheme, so a name reads the same in every language
(the generators only change its case, e.g. `parse_geojson` → `parseGeojson`):

- **Units in the name.** A function returning a bare length names its unit:
  `_m` (`geodesic_distance_m`), or `_meters` in exactly two exports,
  `height_meters` and `mgrs_precision_meters`. One returning an angle ends in
  `_deg` (`initial_bearing_deg`, `final_bearing_deg`, `rhumb_bearing_deg`,
  `wrap_longitude_deg`, `clamp_latitude_deg`, `utm_central_meridian_deg`) or
  names degrees as its target (`dms_to_degrees`, `ddm_to_degrees`,
  `normalize_degrees`). Length and angle parameters carry the same suffix
  (`distance_m`, `bearing_deg`), and a screen-space parameter in pixels ends
  in `_px` (`tile_px`, `padding_px`). A record field holding meters ends in
  `_m` the same way, whether it is a magnitude or a projected or Cartesian
  component: `Ecef` `x_m` / `y_m` / `z_m`, `Enu` `east_m` / `north_m` /
  `up_m`, `Ned` `north_m` / `east_m` / `down_m`, `Utm` and `Ups`
  `easting_m` / `northing_m`, `BaiduMercator` and `WebMercator` `x_m` /
  `y_m`, `Helmert` `tx_m` / `ty_m` / `tz_m`, `Aer` `range_m`, and
  `ApproxWgs84` `max_error_m`. Exceptions, where the unit is already fixed elsewhere or there is
  none:
  - a constructor's parameters take the names of the record fields they fill,
    so `coordinate_new` and `coordinate_from_lon_lat` take `lat` / `lon` like
    the `Coordinate` record (decimal degrees);
  - a function that names degrees as its input takes them unsuffixed
    (`dms_from_degrees(value)`, `ddm_from_degrees(value)`,
    `normalize_degrees(deg)`);
  - the `Length` converters name the unit they convert between, with meters
    always the other side (`length_from_unit(value)` returns meters,
    `length_to_unit(meters)` takes them, and `length_unit_meters_per_unit`
    returns the ratio);
  - the `ExifGps` and `XmpGps` tag-value records name each field after the
    GPS tag it holds, so their meter-valued `altitude` (`GPSAltitude`) and
    `h_positioning_error` (`GPSHPositioningError`) stay unsuffixed: they
    carry the tag's raw value, not a crate magnitude;
  - a dimensionless value has no unit to name: a confidence in `[0, 1]`
    (`confidence_new(value)`), a fraction of a path
    (`intermediate(fraction)`), a fractional zoom level (`zoom` in
    `world_pixel_from_coordinate`, `world_pixel_to_coordinate`,
    `clusters_in_view` and `clusters_in_view_geojson`, `max_zoom` in
    `fit_bounds`), a sign (`hemisphere_sign` returns `-1.0` or `+1.0`), and
    the ellipsoid shape parameters (`ellipsoid_flattening`,
    `ellipsoid_eccentricity_sq`).
- **Ingestion is `parse_*`.** `parse_coordinate`, `parse_coordinate_with`,
  `parse_text`, `parse_text_with`, `parse_geo_uri`, `parse_geojson`,
  `parse_wkt`, `parse_gpx`, `parse_kml` and `parse_nmea_sentence` each read
  text into a `Fix` (or a list of them), and `parse_exif`, `parse_exif_with`,
  `parse_xmp` and `parse_xmp_with` read GPS tag values into a `CaptureGeo`. Rust and FFI use the same
  names for the text readers.
- **`<record>_validate` for every invariant record.** `Coordinate`, `Wgs84`,
  `Gcj02`, `Bd09`, `BaiduMercator`, `Dms`, `Ddm`, `Ellipsoid`, `Enu`, `Ned`,
  `Aer`, `Ecef`, `DatumTransform`, `Utm`, `Ups`, `H3Cell` (`h3_validate`),
  `S2CellId` (`s2_validate`), `Accuracy`, `Confidence`, `GeoBounds`, `Cluster`,
  `Visit`, `Place`, `WebMercator`, `Tile`, `Viewport`, `Camera`, `Precision` and
  `ExclusionZone` each have one. No
  `<record>_new` is added that would only copy its arguments in and validate
  them; one exists where construction derives or normalizes a field
  (`aer_new`, `raw_source_new`, and `place_new`, which computes the distance), beside the constructors that predate that
  rule (`coordinate_new`, `accuracy_new`, `confidence_new`,
  `datum_transform_new`). The `Approx*` records are the exception: every
  export that returns one is a decoder, `convert`, `centroid` or a China inverse,
  and no export takes one as input, so they have no `_new` or `_validate` and
  `Approx::new` has no FFI form. `CaptureGeo` and `NorthBearing` are output-only
  the same way, and the `ExifGps` / `XmpGps` inputs hold no invariant: they are
  checked as they are read.
- **`<grid>_canonicalize` for grid codes.** `plus_code_canonicalize`,
  `geohash_canonicalize`, `maidenhead_canonicalize`, `mgrs_canonicalize` and
  `quadkey_canonicalize` validate a code and return its canonical form.
- **Errors.** Every function that can fail throws the one `GeoError` type and
  documents which variants it raises under `# Errors`.
- **Exactness survives the boundary.** Approximate inverses keep their
  `_fast` / `_refined` suffixes, and names stay language-neutral.

`tests/export_coverage.rs` holds the surface to this scheme by reading the
crate's own source (comments and string literals blanked first, so a mention
counts for nothing). It fails when an export is not called by a Rust test, or
a fallible export by no test that also checks an error; when a fallible export
has no non-empty `# Errors`; when a name breaks the scheme above (a bare `f64`
return or parameter with no unit suffix, `_meters` outside its two exports, a
text-to-`Fix` reader not named `parse_*`, …); when the number of exports, or of
fallible ones, differs from the count pinned in the test, so none can disappear
unnoticed; and when a function or constant in the core crate's public API
snapshot has no FFI form, save the shrink-only gap lists beside the test.

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

## Building

The released packages (`geocoordinates-rs` on PyPI and npm,
`io.github.justin13888:geocoordinates` on Maven Central, and `GeoCoordinates` for
SwiftPM) are built by CI. To generate the bindings yourself from a checkout of
the repository, with [mise](https://mise.jdx.dev/) installed:

```bash
mise run bindings-python   # or bindings-kotlin / bindings-swift: builds the release
                           # cdylib, then generates the sources into bindings/<lang>/
mise run bindings-web      # TypeScript + wasm, built in crates/geocoordinates-ffi/web/
mise run bindings          # all four
```

How to package a wheel, the XCFramework or the JAR locally, and how each
release is published, are in
[`PUBLISHING.md`](https://github.com/justin13888/geocoordinates-rs/blob/master/PUBLISHING.md#local-builds).
