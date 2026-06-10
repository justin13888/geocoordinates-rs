#!/usr/bin/env python3
"""Smoke test for the generated Python bindings.

Exercises the curated FFI surface against the crate's own reference vectors
(`src/test_support.rs`): an exact forward conversion, an approximate inverse
(checking the `max_error_m` bound), the Baidu Mercator round trip, the
`out_of_china` gate, the fallible try-conversion (error path), and distance.

Run from the repo root after building + generating:

    cargo build --release -p geocoordinates-ffi
    cargo run -p geocoordinates-ffi --bin uniffi-bindgen -- \
        generate --library target/release/libgeocoordinates_ffi.dylib \
        --language python --out-dir bindings/python
    cp target/release/libgeocoordinates_ffi.* bindings/python/   # .so on Linux
    python crates/geocoordinates-ffi/tests/smoke.py
"""

import math
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path("bindings/python")))
import geocoordinates_ffi as gc  # noqa: E402


def approx(a: float, b: float, tol: float) -> bool:
    return abs(a - b) <= tol


# Reference triple from src/test_support.rs (coordtransform-rs), (lat, lon):
WGS = (39.915, 116.404)
GCJ = (39.91640428150164, 116.41024449916938)
BD = (39.922699552216216, 116.41662724378733)

# --- Exact forward: WGS-84 -> GCJ-02 (deterministic, must match to f64) ---
g = gc.wgs84_to_gcj02(gc.Wgs84(lat=WGS[0], lon=WGS[1]))
assert approx(g.lat, GCJ[0], 1e-9), g.lat
assert approx(g.lon, GCJ[1], 1e-9), g.lon

# --- Exact forward: WGS-84 -> BD-09 ---
b = gc.wgs84_to_bd09(gc.Wgs84(lat=WGS[0], lon=WGS[1]))
assert approx(b.lat, BD[0], 1e-9), b.lat
assert approx(b.lon, BD[1], 1e-9), b.lon

# --- Approximate inverse: GCJ-02 -> WGS-84, refined (< 0.5 m) ---
w = gc.gcj02_to_wgs84_refined(gc.Gcj02(lat=GCJ[0], lon=GCJ[1]))
assert approx(w.lat, WGS[0], 1e-5), w.lat  # < 0.5 m ~= 4.5e-6 deg
assert approx(w.lon, WGS[1], 1e-5), w.lon
assert 0.0 <= w.max_error_m <= 0.5, w.max_error_m  # flattened Approx bound

# --- Baidu Web Mercator round trip (exact both ways) ---
src = gc.Bd09(lat=BD[0], lon=BD[1])
merc = gc.baidu_mercator_from_bd09(src)
back = gc.baidu_mercator_to_bd09(merc)
assert approx(back.lat, BD[0], 1e-6), back.lat
assert approx(back.lon, BD[1], 1e-6), back.lon

# --- out_of_china gate ---
assert gc.out_of_china(51.5074, -0.1278) is True  # London
assert gc.out_of_china(WGS[0], WGS[1]) is False  # Beijing

# --- Fallible try-conversion: error path + success path ---
try:
    gc.baidu_mercator_try_from_coordinate(gc.coordinate_wgs84(WGS[0], WGS[1]))
    raise AssertionError("expected CrsMismatch for a WGS-84 coordinate")
except gc.GeoError.CrsMismatch:
    pass
ok = gc.baidu_mercator_try_from_coordinate(gc.coordinate_bd09(BD[0], BD[1]))
assert approx(ok.x, merc.x, 1e-6) and approx(ok.y, merc.y, 1e-6)

# --- Distance (Length flattened to meters) ---
a = gc.coordinate_wgs84(WGS[0], WGS[1])
assert gc.haversine_distance_m(a, a) == 0.0
one_deg_north = gc.coordinate_wgs84(WGS[0] + 1.0, WGS[1])
d = gc.haversine_distance_m(a, one_deg_north)
assert math.isclose(d, 111194.9, abs_tol=50.0), d  # ~mean-earth-radius * pi/180

print("python smoke OK")
