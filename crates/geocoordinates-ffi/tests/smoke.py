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

# --- Angle conversions: DD <-> DMS decomposition + round trip ---
dms = gc.dd_to_dms(gc.Dd(value=40.7127753), gc.Axis.LATITUDE)
assert dms.degrees == 40 and dms.minutes == 42, (dms.degrees, dms.minutes)
assert approx(dms.seconds, 45.99108, 1e-3), dms.seconds
assert dms.hemisphere == gc.Hemisphere.NORTH, dms.hemisphere
assert approx(gc.dms_to_dd(dms).value, 40.7127753, 1e-9)
# -0.0 reads as the positive hemisphere, never South/West.
neg_zero = gc.dd_to_dms(gc.Dd(value=-0.0), gc.Axis.LATITUDE)
assert neg_zero.hemisphere == gc.Hemisphere.NORTH, neg_zero.hemisphere

# --- Angle normalization (antimeridian / poles) ---
assert approx(gc.wrap_longitude(180.0), -180.0, 1e-9)  # half-open
assert approx(gc.clamp_latitude(91.0), 90.0, 1e-9)
assert approx(gc.normalize_degrees(-1.0), 359.0, 1e-9)

# --- Length unit conversions (flattened to meters) ---
assert approx(gc.length_from_unit(1.0, gc.LengthUnit.KILOMETER), 1000.0, 1e-9)
assert approx(gc.length_to_unit(1852.0, gc.LengthUnit.NAUTICAL_MILE), 1.0, 1e-9)

# --- Coordinate validation + Null Island ---
gc.coordinate_validate(gc.coordinate_wgs84(40.0, -74.0))  # in range -> no raise
try:
    gc.coordinate_validate(gc.coordinate_wgs84(91.0, 0.0))
    raise AssertionError("expected OutOfRange for latitude 91")
except gc.GeoError.OutOfRange:
    pass
assert gc.coordinate_is_null_island(gc.coordinate_wgs84(0.0, 0.0)) is True
assert gc.coordinate_is_null_island(a) is False

# --- Fix observation family (SystemTime maps to a native timestamp) ---
fix = gc.fix_from_coord(a)
assert approx(fix.coord.lat, WGS[0], 1e-9)
assert fix.accuracy is None and fix.timestamp is None and fix.source is None
assert approx(gc.confidence_new(2.0).value, 1.0, 1e-12)  # clamped into [0, 1]

# --- Coordinate formatting (DD / DMS, plus accuracy-derived precision) ---
dd_opts = gc.FormatOptions(
    representation=gc.Representation.DECIMAL_DEGREES,
    precision=6,
    symbol_style=gc.SymbolStyle.UNICODE,
    hemisphere_style=gc.HemisphereStyle.SIGNED,
    locale=None,
)
assert (
    gc.format_coordinate(gc.coordinate_wgs84(40.7128, -74.006), dd_opts)
    == "40.712800, -74.006000"
)
dms_opts = gc.FormatOptions(
    representation=gc.Representation.DMS,
    precision=0,
    symbol_style=gc.SymbolStyle.UNICODE,
    hemisphere_style=gc.HemisphereStyle.CARDINAL,
    locale=None,
)
assert (
    gc.format_coordinate(gc.coordinate_wgs84(10.5, 20.25), dms_opts)
    == "10°30′00″N 20°15′00″E"
)
# format_fix derives precision from accuracy when precision is None (~1 km -> 2 dp)
auto_opts = gc.FormatOptions(
    representation=gc.Representation.DECIMAL_DEGREES,
    precision=None,
    symbol_style=gc.SymbolStyle.UNICODE,
    hemisphere_style=gc.HemisphereStyle.SIGNED,
    locale=None,
)
km_fix = gc.Fix(
    coord=gc.coordinate_wgs84(40.7128, -74.006),
    accuracy=gc.Accuracy(horizontal_m=1000.0, vertical_m=None),
    timestamp=None,
    source=None,
)
assert gc.format_fix(km_fix, auto_opts) == "40.71, -74.01"

# --- Parsing (geo: URI, free text, explicit options) ---
geo = gc.parse_coordinate("geo:48.2,16.3,183;crs=wgs84;u=40")
assert approx(geo.coord.lat, 48.2, 1e-9) and approx(geo.coord.lon, 16.3, 1e-9)
assert geo.source.axis_order is None  # RFC 5870 fixes lat-first
assert geo.accuracy.horizontal_m == 40.0
text = gc.parse_coordinate("40.7128, -74.006")
assert approx(text.coord.lat, 40.7128, 1e-9)
assert text.source.axis_order is not None  # free text records the assumed order
# Explicit options: European decimal comma, whitespace-separated.
comma_opts = gc.TextParseOptions(
    default_axis_order=gc.AxisOrder.LAT_LON, decimal_comma=True
)
comma = gc.parse_text_with("40,7128 -74,006", comma_opts)
assert approx(comma.coord.lat, 40.7128, 1e-9)
assert approx(comma.coord.lon, -74.006, 1e-9)

# --- Plus Code (Open Location Code) ---
assert gc.plus_code_encode(gc.coordinate_wgs84(47.0000625, 8.0000625), 10) == "8FVC2222+22"
area = gc.plus_code_decode("8FVC2222+22")
assert approx(area.lat, 47.0000625, 1e-6) and approx(area.lon, 8.0000625, 1e-6)
assert 0.0 < area.max_error_m < 20.0
try:
    gc.plus_code_decode("not a code")
    raise AssertionError("expected error for an invalid Plus Code")
except gc.GeoError.Other:
    pass
# parse_coordinate detects a Plus Code; format renders one.
pc_fix = gc.parse_coordinate("8FVC2222+22")
assert approx(pc_fix.coord.lat, 47.0000625, 1e-6)
assert pc_fix.source.axis_order is None
pc_opts = gc.FormatOptions(
    representation=gc.Representation.PLUS_CODE,
    precision=None,
    symbol_style=gc.SymbolStyle.UNICODE,
    hemisphere_style=gc.HemisphereStyle.SIGNED,
    locale=None,
)
assert (
    gc.format_coordinate(gc.coordinate_wgs84(47.0000625, 8.0000625), pc_opts)
    == "8FVC2222+22"
)

# --- Geohash and Maidenhead ---
assert gc.geohash_encode(gc.coordinate_wgs84(42.6, -5.6), 5) == "ezs42"
gh = gc.geohash_decode("ezs42")
assert gh.max_error_m > 0.0
assert gc.maidenhead_encode(gc.coordinate_wgs84(40.5, -75.0), 2) == "FN20"
mh = gc.maidenhead_decode("FN20")
assert approx(mh.lat, 40.5, 1e-9) and approx(mh.lon, -75.0, 1e-9)

# --- Geodesy: ellipsoid, ECEF, ENU/AER ---
wgs84 = gc.ellipsoid_wgs84()
assert approx(gc.ellipsoid_semi_minor_m(wgs84), 6_356_752.314_245, 1e-3)
ecef = gc.ecef_from_coordinate(gc.coordinate_wgs84(0.0, 0.0), wgs84)
assert approx(ecef.x, 6_378_137.0, 1e-3) and approx(ecef.y, 0.0, 1e-3)
assert approx(gc.ecef_to_coordinate(ecef, wgs84, gc.Crs.WGS84).lat, 0.0, 1e-9)
origin = gc.coordinate_wgs84(40.0, -75.0)
enu = gc.enu_from_coordinate(gc.coordinate_wgs84(40.1, -75.0), origin)
assert enu.north > 10_000.0
aer = gc.enu_to_aer(gc.Enu(east=0.0, north=100.0, up=0.0))
assert approx(aer.azimuth_deg, 0.0, 1e-9) and approx(aer.range_m, 100.0, 1e-9)

# --- Geodesics ---
assert (
    gc.geodesic_distance_m(gc.coordinate_wgs84(40.7128, -74.006), gc.coordinate_wgs84(51.5074, -0.1278))
    > 5_000_000.0
)
assert approx(
    gc.initial_bearing(gc.coordinate_wgs84(0.0, 0.0), gc.coordinate_wgs84(0.0, 1.0)), 90.0, 1e-6
)
dest = gc.destination(gc.coordinate_wgs84(0.0, 0.0), 90.0, 111_319.49)
assert approx(dest.lon, 1.0, 1e-4)
assert (
    gc.intersection(gc.coordinate_wgs84(0.0, -10.0), 90.0, gc.coordinate_wgs84(-10.0, 0.0), 0.0)
    is not None
)

# --- Classic datums (Helmert / DatumTransform) ---
assert gc.datum_transform_to_wgs84(gc.Crs.WGS84) is None  # no shift needed
assert gc.datum_transform_to_wgs84(gc.Crs.GCJ02) is None  # use China typed conversions
dt = gc.datum_transform_to_wgs84(gc.Crs.NAD27)
assert dt is not None and approx(dt.helmert.tx_m, -8.0, 1e-12)
# Translation-only Helmert round-trips exactly.
nad27 = gc.Coordinate(lat=40.0, lon=-100.0, height=None, crs=gc.Crs.NAD27)
w = gc.datum_transform_apply(dt, nad27)
assert w.crs == gc.Crs.WGS84 and approx(w.lat, 40.000_009_482_759, 1e-7)
back = gc.datum_transform_apply(gc.datum_transform_inverse(dt), w)
assert back.crs == gc.Crs.NAD27 and approx(back.lat, 40.0, 1e-9) and approx(back.lon, -100.0, 1e-9)
# apply_ecef + inverse identity at the ECEF level.
ident = gc.helmert_inverse(gc.helmert_identity())
e = gc.helmert_apply_ecef(ident, gc.Ecef(x=4_000_000.0, y=-2_000_000.0, z=4_500_000.0))
assert approx(e.x, 4_000_000.0, 1e-6)

# --- Runtime conversion dispatch ---
assert gc.can_convert(gc.Crs.BD09, gc.Crs.TOKYO) is True
# WGS-84 -> GCJ-02 is exact (bound 0); the inverse is approximate (bound > 0).
fwd = gc.convert(gc.coordinate_wgs84(WGS[0], WGS[1]), gc.Crs.GCJ02)
assert fwd.max_error_m == 0.0 and fwd.coord.crs == gc.Crs.GCJ02
assert approx(fwd.coord.lat, GCJ[0], 1e-9)
inv = gc.convert(gc.coordinate_gcj02(GCJ[0], GCJ[1]), gc.Crs.WGS84)
assert inv.max_error_m > 0.0 and approx(inv.coord.lat, WGS[0], 1e-5)
# GCJ-02 -> BD-09 stays exact via the direct path.
gb = gc.convert(gc.coordinate_gcj02(GCJ[0], GCJ[1]), gc.Crs.BD09)
assert gb.max_error_m == 0.0 and approx(gb.coord.lat, BD[0], 1e-9)

# --- UTM / UPS / MGRS ---
u = gc.utm_from_coordinate(gc.coordinate_wgs84(40.0, -75.0))
assert u.zone == 18 and u.hemisphere == gc.UtmHemisphere.NORTH
assert approx(u.easting, 500_000.0, 1e-3) and approx(u.northing, 4_427_757.2188, 1e-3)
roundtrip = gc.utm_to_coordinate(u)
assert approx(roundtrip.lat, 40.0, 1e-9) and approx(roundtrip.lon, -75.0, 1e-9)
try:
    gc.utm_from_coordinate(gc.coordinate_wgs84(85.0, 0.0))  # polar -> error
    raise AssertionError("expected OutOfRange for a polar latitude")
except gc.GeoError.OutOfRange:
    pass
ups = gc.ups_from_coordinate(gc.coordinate_wgs84(85.0, 0.0))
assert ups.hemisphere == gc.UtmHemisphere.NORTH and approx(ups.easting, 2_000_000.0, 1e-3)
assert gc.mgrs_from_coordinate(gc.coordinate_wgs84(40.0, -75.0), 1) == "18TWK0000027757"
assert gc.mgrs_precision_m("18TWK000277") == 100
dec = gc.mgrs_to_coordinate("18TWK0000027757")
assert dec.max_error_m == 0.5 and approx(dec.coord.lat, 40.0, 1e-3)
try:
    gc.mgrs_to_coordinate("not-an-mgrs")
    raise AssertionError("expected error for a bad MGRS string")
except gc.GeoError.Other:  # InvalidGridRef maps to the Other catch-all
    pass

# --- Interchange formats (GeoJSON / WKT / GPX / KML) ---
gj = gc.from_geojson('{"type":"Point","coordinates":[2.0,9.0,100.0]}')
assert len(gj) == 1 and approx(gj[0].coord.lat, 9.0, 1e-9) and approx(gj[0].coord.lon, 2.0, 1e-9)
assert gj[0].source.axis_order == gc.AxisOrder.LON_LAT
wktfixes = gc.from_wkt("LINESTRING(0 0, 10 20)")
assert len(wktfixes) == 2 and approx(wktfixes[1].coord.lat, 20.0, 1e-9)
gpxfixes = gc.from_gpx(
    '<?xml version="1.0"?><gpx version="1.1" creator="t"'
    ' xmlns="http://www.topografix.com/GPX/1/1"><wpt lat="48.2" lon="16.3"/></gpx>'
)
assert len(gpxfixes) == 1 and approx(gpxfixes[0].coord.lat, 48.2, 1e-9)
assert gpxfixes[0].source.axis_order == gc.AxisOrder.LAT_LON
kmlfixes = gc.from_kml(
    '<?xml version="1.0"?><kml xmlns="http://www.opengis.net/kml/2.2"><Document>'
    "<Placemark><Point><coordinates>16.3,48.2</coordinates></Point></Placemark></Document></kml>"
)
assert len(kmlfixes) == 1 and approx(kmlfixes[0].coord.lon, 16.3, 1e-9)
try:
    gc.from_geojson("not json")
    raise AssertionError("expected a parse error")
except gc.GeoError.Other:  # Error::Parse maps to the Other catch-all
    pass

# --- NMEA 0183 sentences ---
nmea = gc.from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")
assert approx(nmea.coord.lat, 48.0 + 7.038 / 60.0, 1e-9)
assert approx(nmea.coord.lon, 11.0 + 31.0 / 60.0, 1e-9)
assert nmea.source.axis_order is None  # NMEA is lat-first
assert approx(nmea.accuracy.horizontal_m, 0.9 * 5.0, 1e-9)  # HDOP x nominal UERE
try:
    gc.from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E")  # bad checksum
    raise AssertionError("expected a checksum error")
except gc.GeoError.Other:
    pass

# --- H3 discrete global grid (S2 is native-Rust only; not in the FFI) ---
h3 = gc.h3_encode(gc.coordinate_wgs84(40.7128, -74.006), 9)
assert h3.value == 617733151020810239  # canonical H3 index
h3dec = gc.h3_decode(h3)
assert h3dec.max_error_m > 0.0
assert approx(h3dec.coord.lat, 40.7128, 0.01) and approx(h3dec.coord.lon, -74.006, 0.01)

print("python smoke OK")
