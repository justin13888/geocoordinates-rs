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
