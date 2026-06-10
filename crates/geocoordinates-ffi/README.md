# geocoordinates (language bindings)

[UniFFI](https://mozilla.github.io/uniffi-rs/) bindings for the
[`geocoordinates`](https://crates.io/crates/geocoordinates) geospatial library —
China datums (GCJ-02 / BD-09), geodetic transforms.

A deliberately curated, flattened subset of the Rust API is exposed across the FFI
boundary to **Python, Kotlin, Swift, and TypeScript** (Java consumes the Kotlin/JVM
artifact). Generics, traits, operator overloads, and `From`/`TryFrom` conversions do
not cross FFI, so they are re-expressed as flat records and free functions; approximate
inverses keep their `_fast` / `_refined` names and carry an explicit `max_error_m`.

## Surface

WGS-84 ↔ GCJ-02 ↔ BD-09 conversions (exact forward, approximate inverse with error
bounds), Baidu Web Mercator, `out_of_china`, and haversine distance.

```python
import geocoordinates_ffi as gc

gcj = gc.wgs84_to_gcj02(gc.Wgs84(lat=39.915, lon=116.404))
wgs = gc.gcj02_to_wgs84_refined(gcj)          # approximate inverse
print(wgs.lat, wgs.lon, "±", wgs.max_error_m, "m")
```

The WebAssembly/TypeScript bindings live in [`web/`](web/).
