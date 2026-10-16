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


def wgs84_coord(lat: float, lon: float) -> "gc.Coordinate":
    return gc.coordinate_new(lat, lon, gc.Crs.WGS84)


def gcj02_coord(lat: float, lon: float) -> "gc.Coordinate":
    return gc.coordinate_new(lat, lon, gc.Crs.GCJ02)


def bd09_coord(lat: float, lon: float) -> "gc.Coordinate":
    return gc.coordinate_new(lat, lon, gc.Crs.BD09)


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

# --- Fallible conversion: error path + success path ---
try:
    gc.baidu_mercator_from_coordinate(wgs84_coord(WGS[0], WGS[1]))
    raise AssertionError("expected CrsMismatch for a WGS-84 coordinate")
except gc.GeoError.CrsMismatch as e:
    # The reference systems cross as the typed Crs enum, not as strings.
    assert e.expected == gc.Crs.BD09 and e.found == gc.Crs.WGS84, (e.expected, e.found)
ok =gc.baidu_mercator_from_coordinate(bd09_coord(BD[0], BD[1]))
assert approx(ok.x, merc.x, 1e-6) and approx(ok.y, merc.y, 1e-6)

# --- China records: validation, Coordinate bridges, and domain edges ---
gc.wgs84_validate(gc.Wgs84(lat=WGS[0], lon=WGS[1]))
try:
    gc.gcj02_validate(gc.Gcj02(lat=91.0, lon=0.0))
    raise AssertionError("expected OutOfRange for an out-of-range record")
except gc.GeoError.OutOfRange:
    pass
c = gc.bd09_to_coordinate(gc.Bd09(lat=BD[0], lon=BD[1]))
assert c.crs == gc.Crs.BD09 and gc.bd09_from_coordinate(c) == gc.Bd09(lat=BD[0], lon=BD[1])
try:
    gc.wgs84_from_coordinate(c)
    raise AssertionError("expected CrsMismatch for a BD-09 coordinate")
except gc.GeoError.CrsMismatch:
    pass
# BD-09 wraps at the antimeridian and refuses to cross a pole.
edge = gc.gcj02_to_bd09(gc.Gcj02(lat=0.0, lon=180.0))
assert approx(edge.lon, -179.9935, 1e-3), edge.lon
try:
    gc.gcj02_to_bd09(gc.Gcj02(lat=90.0, lon=0.0))
    raise AssertionError("expected OutsideDomain past the pole")
except gc.GeoError.OutsideDomain as e:
    assert e.operation == "BD-09", e.operation
# Baidu Mercator is defined only within 74 degrees of latitude.
try:
    gc.baidu_mercator_from_bd09(gc.Bd09(lat=80.0, lon=0.0))
    raise AssertionError("expected OutsideDomain beyond 74 degrees")
except gc.GeoError.OutsideDomain as e:
    assert e.operation == "Baidu Mercator", e.operation

# --- Distance (Length flattened to meters) ---
a = wgs84_coord(WGS[0], WGS[1])
assert gc.haversine_distance_m(a, a) == 0.0
one_deg_north = wgs84_coord(WGS[0] + 1.0, WGS[1])
d = gc.haversine_distance_m(a, one_deg_north)
assert math.isclose(d, 111194.9, abs_tol=50.0), d  # ~mean-earth-radius * pi/180

# --- Angle conversions: degrees <-> DMS / DDM decomposition + round trip ---
dms = gc.dms_from_degrees(40.7127753, gc.Axis.LATITUDE)
assert dms.degrees == 40 and dms.minutes == 42, (dms.degrees, dms.minutes)
assert approx(dms.seconds, 45.99108, 1e-3), dms.seconds
assert dms.hemisphere == gc.Hemisphere.NORTH, dms.hemisphere
assert approx(gc.dms_to_degrees(dms), 40.7127753, 1e-9)
ddm = gc.ddm_from_degrees(-74.006, gc.Axis.LONGITUDE)
assert ddm.degrees == 74 and approx(ddm.minutes, 0.36, 1e-9), (ddm.degrees, ddm.minutes)
assert approx(gc.ddm_to_degrees(gc.dms_to_ddm(gc.ddm_to_dms(ddm))), -74.006, 1e-9)
# -0.0 reads as the positive hemisphere, never South/West.
neg_zero = gc.dms_from_degrees(-0.0, gc.Axis.LATITUDE)
assert neg_zero.hemisphere == gc.Hemisphere.NORTH, neg_zero.hemisphere
# A hand-built record is validated on use: 60 minutes is out of range.
gc.dms_validate(dms)
bad_dms = gc.Dms(degrees=40, minutes=60, seconds=0.0, hemisphere=gc.Hemisphere.NORTH)
for call in (gc.dms_validate, gc.dms_to_degrees):
    try:
        call(bad_dms)
        raise AssertionError("expected GeoError.InvalidValue for minutes = 60")
    except gc.GeoError.InvalidValue as e:
        assert e.field == "minutes", e.field
try:
    gc.ddm_validate(gc.Ddm(degrees=91, minutes=0.0, hemisphere=gc.Hemisphere.SOUTH))
    raise AssertionError("expected GeoError.InvalidValue for 91 degrees latitude")
except gc.GeoError.InvalidValue as e:
    assert e.field == "degrees", e.field

# --- Angle normalization (antimeridian / poles) ---
assert approx(gc.wrap_longitude(180.0), -180.0, 1e-9)  # half-open
assert approx(gc.clamp_latitude(91.0), 90.0, 1e-9)
assert approx(gc.normalize_degrees(-1.0), 359.0, 1e-9)
assert gc.normalize_degrees(-1e-20) == 0.0  # never the excluded bound 360

# --- Length unit conversions (flattened to meters) ---
assert approx(gc.length_from_unit(1.0, gc.LengthUnit.KILOMETER), 1000.0, 1e-9)
assert approx(gc.length_to_unit(1852.0, gc.LengthUnit.NAUTICAL_MILE), 1.0, 1e-9)
assert gc.length_from_unit(1.0, gc.LengthUnit.MILE) == 1609.344
assert gc.length_from_unit(1.0, gc.LengthUnit.YARD) == 0.9144

# --- Coordinate validation + Null Island ---
gc.coordinate_validate(wgs84_coord(40.0, -74.0))  # in range -> no raise
try:
    gc.coordinate_new(91.0, 0.0, gc.Crs.WGS84)
    raise AssertionError("expected OutOfRange from coordinate_new for latitude 91")
except gc.GeoError.OutOfRange as e:
    assert e.lat == 91.0 and e.lon == 0.0, (e.lat, e.lon)
# A hand-built record bypasses coordinate_new; validation still catches it.
try:
    gc.coordinate_validate(gc.Coordinate(lat=91.0, lon=0.0, height=None, crs=gc.Crs.WGS84))
    raise AssertionError("expected OutOfRange for latitude 91")
except gc.GeoError.OutOfRange as e:
    assert e.lat == 91.0 and e.lon == 0.0, (e.lat, e.lon)
assert gc.coordinate_is_null_island(wgs84_coord(0.0, 0.0)) is True
assert gc.coordinate_is_null_island(a) is False

# --- Fix observation family (SystemTime maps to a native timestamp) ---
fix = gc.fix_from_coord(a)
assert approx(fix.coord.lat, WGS[0], 1e-9)
assert fix.accuracy is None and fix.timestamp is None and fix.source is None
assert approx(gc.confidence_new(2.0).value, 1.0, 1e-12)  # clamped into [0, 1]

# --- Coordinate formatting (DD / DMS, plus accuracy-derived precision) ---
dd_opts = gc.FormatOptions(
    representation=gc.Representation.DECIMAL_DEGREES(
        decimals=6, hemisphere=gc.HemisphereStyle.SIGNED
    ),
    decimal_separator=gc.DecimalSeparator.POINT,
)
assert (
    gc.format_coordinate(wgs84_coord(40.7128, -74.006), dd_opts)
    == "40.712800, -74.006000"
)
dms_opts = gc.FormatOptions(
    representation=gc.Representation.DMS(
        second_decimals=0,
        symbols=gc.SymbolStyle.UNICODE,
        hemisphere=gc.HemisphereStyle.CARDINAL,
    ),
    decimal_separator=gc.DecimalSeparator.POINT,
)
assert (
    gc.format_coordinate(wgs84_coord(10.5, 20.25), dms_opts)
    == "10°30′00″N 20°15′00″E"
)
# The default options leave the decimal count unset, so format_fix derives it
# from accuracy (~1 km -> 2 dp).
auto_opts = gc.format_options_default()
assert auto_opts.decimal_separator == gc.DecimalSeparator.POINT
km_fix = gc.Fix(
    coord=wgs84_coord(40.7128, -74.006),
    accuracy=gc.Accuracy(horizontal_m=1000.0, vertical_m=None),
    timestamp=None,
    source=None,
)
assert gc.format_fix(km_fix, auto_opts) == "40.71, -74.01"
# The locale heuristic picks the separator; a comma rendering re-parses with
# format_options_parse_options (which carries decimal_separator=COMMA).
assert gc.decimal_separator_for_locale("de-DE") == gc.DecimalSeparator.COMMA
assert gc.decimal_separator_for_locale("de-CH") == gc.DecimalSeparator.POINT
comma_dd = gc.FormatOptions(
    representation=gc.Representation.DECIMAL_DEGREES(
        decimals=2, hemisphere=gc.HemisphereStyle.SIGNED
    ),
    decimal_separator=gc.decimal_separator_for_locale("de-DE"),
)
assert gc.format_coordinate(wgs84_coord(40.7128, -74.006), comma_dd) == "40,71 -74,01"
# Every grid with a string form renders through format_coordinate.
grid_cases = [
    (gc.Representation.GEOHASH(length=5), (42.6, -5.6), "ezs42"),
    (gc.Representation.MAIDENHEAD(pairs=2), (40.5, -75.0), "FN20"),
    (
        gc.Representation.MGRS(precision=gc.MgrsPrecision.M1),
        (40.0, -75.0),
        "18TWK0000027757",
    ),
]
for representation, (lat, lon), expected in grid_cases:
    grid_opts = gc.FormatOptions(
        representation=representation, decimal_separator=gc.DecimalSeparator.POINT
    )
    assert gc.format_coordinate(wgs84_coord(lat, lon), grid_opts) == expected

# --- Parsing (geo: URI, free text, explicit options) ---
geo = gc.parse_coordinate("geo:48.2,16.3,183;crs=wgs84;u=40")
assert approx(geo.coord.lat, 48.2, 1e-9) and approx(geo.coord.lon, 16.3, 1e-9)
assert geo.source.axis_order == gc.AxisOrder.LAT_LON  # RFC 5870 lists lat first
assert geo.accuracy.horizontal_m == 40.0
text = gc.parse_coordinate("40.7128, -74.006")
assert approx(text.coord.lat, 40.7128, 1e-9)
assert text.source.axis_order is not None  # free text records the assumed order
# Explicit options: European decimal comma, whitespace- or comma-separated.
default_parse = gc.parse_options_default()
assert default_parse.decimal_separator == gc.DecimalSeparator.POINT
assert default_parse.expect_grid is None
comma_opts = gc.ParseOptions(
    default_axis_order=gc.AxisOrder.LAT_LON,
    decimal_separator=gc.DecimalSeparator.COMMA,
    expect_grid=None,
)
for comma_text in ["40,7128 -74,006", "40,7128, -74,006"]:
    comma = gc.parse_text_with(comma_text, comma_opts)
    assert approx(comma.coord.lat, 40.7128, 1e-9)
    assert approx(comma.coord.lon, -74.006, 1e-9)
# A decimal-comma rendering re-parses with the format options' own parse options.
comma_back = gc.parse_coordinate_with(
    "40,71 -74,01", gc.format_options_parse_options(comma_dd)
)
assert approx(comma_back.coord.lat, 40.71, 1e-9)
# Text with no single reading is rejected rather than guessed at.
for ambiguous in ["1234.5, 10", "40.5 30, 10", "4e1 3e1"]:
    try:
        gc.parse_coordinate(ambiguous)
        raise AssertionError(f"expected {ambiguous!r} to be rejected")
    except gc.GeoError.Parse:
        pass
# MGRS is always detected; a geohash only when the options expect one.
mgrs_fix = gc.parse_coordinate("18TWK0000027757")
assert mgrs_fix.source.axis_order is None
assert mgrs_fix.accuracy.horizontal_m < 1.0
try:
    gc.parse_coordinate("ezs42")
    raise AssertionError("a geohash must not be detected unasked")
except gc.GeoError.Parse:
    pass
for representation, (lat, lon), expected in grid_cases:
    grid_opts = gc.FormatOptions(
        representation=representation, decimal_separator=gc.DecimalSeparator.POINT
    )
    grid_parse = gc.format_options_parse_options(grid_opts)
    assert grid_parse.expect_grid is not None
    cell = gc.parse_coordinate_with(expected, grid_parse)
    # The rendered point lies within the decoded cell's reported bound.
    assert gc.geodesic_distance_m(cell.coord, wgs84_coord(lat, lon)) <= (
        cell.accuracy.horizontal_m
    ), expected

# --- Plus Code (Open Location Code) ---
assert gc.plus_code_encode(wgs84_coord(47.0000625, 8.0000625), 10) == "8FVC2222+22"
area = gc.plus_code_decode("8FVC2222+22")
# Grid decodes share the ApproxCoordinate shape with MGRS/H3/S2 and `convert`.
assert area.coord.crs == gc.Crs.WGS84
assert approx(area.coord.lat, 47.0000625, 1e-6) and approx(area.coord.lon, 8.0000625, 1e-6)
assert approx(area.max_error_m, 8.418_606, 1e-5), area.max_error_m  # geodesic corner distance
try:
    gc.plus_code_decode("not a code")
    raise AssertionError("expected error for an invalid Plus Code")
except gc.GeoError.InvalidGridRef as e:
    assert e.detail == "invalid Plus Code: not a code", e.detail
# parse_coordinate detects a Plus Code; format renders one.
pc_fix = gc.parse_coordinate("8FVC2222+22")
assert approx(pc_fix.coord.lat, 47.0000625, 1e-6)
assert pc_fix.source.axis_order is None
pc_opts = gc.FormatOptions(
    representation=gc.Representation.PLUS_CODE(length=10),
    decimal_separator=gc.DecimalSeparator.POINT,
)
assert (
    gc.format_coordinate(wgs84_coord(47.0000625, 8.0000625), pc_opts)
    == "8FVC2222+22"
)

# --- Geohash and Maidenhead ---
assert gc.geohash_encode(wgs84_coord(42.6, -5.6), 5) == "ezs42"
gh = gc.geohash_decode("ezs42")
assert gh.coord.crs == gc.Crs.WGS84
assert approx(gh.coord.lat, 42.604_980_468_75, 1e-12) and approx(gh.coord.lon, -5.603_027_343_75, 1e-12)
assert approx(gh.max_error_m, 3_034.794_838, 1e-5), gh.max_error_m
assert gc.maidenhead_encode(wgs84_coord(40.5, -75.0), 2) == "FN20"
mh = gc.maidenhead_decode("FN20")
assert mh.coord.crs == gc.Crs.WGS84
assert approx(mh.coord.lat, 40.5, 1e-9) and approx(mh.coord.lon, -75.0, 1e-9)
assert approx(mh.max_error_m, 101_592.817_121, 1e-5), mh.max_error_m
# Canonicalize validates a code string and returns its canonical form.
assert gc.plus_code_canonicalize(" 87g7x2vv+2v ") == "87G7X2VV+2V"
assert gc.geohash_canonicalize("EZS42") == "ezs42"
assert gc.maidenhead_canonicalize("jn58TD") == "JN58td"
try:
    gc.geohash_canonicalize("ezs42ezs42ezs")
    raise AssertionError("expected error for a geohash longer than 12")
except gc.GeoError.InvalidGridRef:
    pass

# --- Geodesy: ellipsoid, ECEF, ENU/AER ---
wgs84 = gc.ellipsoid_wgs84()
assert approx(gc.ellipsoid_semi_minor_m(wgs84), 6_356_752.314_245, 1e-3)
assert gc.ellipsoid_new(6_378_137.0, 298.257_223_563) == wgs84
try:
    gc.ellipsoid_new(-1.0, 298.257_223_563)
    raise AssertionError("expected InvalidValue for a negative semi-major axis")
except gc.GeoError.InvalidValue:
    pass
# The ellipsoid comes from the coordinate's CRS: NAD27 sits on Clarke 1866.
assert gc.ellipsoid_for_crs(gc.Crs.NAD27) == gc.ellipsoid_clarke_1866()
assert gc.ellipsoid_for_crs(gc.Crs.GCJ02) is None
ecef = gc.ecef_from_coordinate(wgs84_coord(0.0, 0.0))
assert approx(ecef.x, 6_378_137.0, 1e-3) and approx(ecef.y, 0.0, 1e-3)
assert approx(gc.ecef_to_coordinate(ecef, gc.Crs.WGS84).lat, 0.0, 1e-9)
nad27_origin = gc.Coordinate(lat=0.0, lon=0.0, height=None, crs=gc.Crs.NAD27)
assert approx(gc.ecef_from_coordinate(nad27_origin).x, 6_378_206.4, 1e-6)
try:
    gc.ecef_from_coordinate(gc.Coordinate(lat=39.9, lon=116.4, height=None, crs=gc.Crs.GCJ02))
    raise AssertionError("expected CrsMismatch: GCJ-02 has no ellipsoid")
except gc.GeoError.CrsMismatch:
    pass
origin = wgs84_coord(40.0, -75.0)
enu = gc.enu_from_coordinate(wgs84_coord(40.1, -75.0), origin)
# 0.1 deg of meridian arc at 40 N; the target sits below the tangent plane.
assert approx(enu.north, 11_103.553_727, 1e-5), enu.north
assert approx(enu.east, 0.0, 1e-6) and approx(enu.up, -9.689_709, 1e-5), (enu.east, enu.up)
aer = gc.enu_to_aer(gc.Enu(east=0.0, north=100.0, up=0.0))
assert approx(aer.azimuth_deg, 0.0, 1e-9) and approx(aer.range_m, 100.0, 1e-9)

# --- Geodesics ---
ny_london = gc.geodesic_distance_m(
    wgs84_coord(40.7128, -74.006), wgs84_coord(51.5074, -0.1278)
)
assert approx(ny_london, 5_585_233.578_931, 1e-3), ny_london  # Karney on WGS-84
assert approx(
    gc.initial_bearing(wgs84_coord(0.0, 0.0), wgs84_coord(0.0, 1.0)), 90.0, 1e-6
)
dest = gc.destination(wgs84_coord(0.0, 0.0), 90.0, 111_319.49)
assert approx(dest.lon, 1.0, 1e-4)
# The equator (eastbound) and the prime meridian (northbound) meet at Null Island.
res = gc.intersection_spherical(
    wgs84_coord(0.0, -10.0), 90.0, wgs84_coord(-10.0, 0.0), 0.0
)
assert res.is_CROSSING(), res
hit = res.point
assert hit.crs == gc.Crs.WGS84
assert approx(hit.lat, 0.0, 1e-12) and approx(hit.lon, 0.0, 1e-12), (hit.lat, hit.lon)
# From the north pole, 180 degrees runs down the given meridian to (0, 10).
res = gc.intersection_spherical(wgs84_coord(90.0, 10.0), 180.0, wgs84_coord(0.0, 0.0), 90.0)
assert res.is_CROSSING() and approx(res.point.lon, 10.0, 1e-9), res
# Two eastbound paths along the equator share one great circle.
res = gc.intersection_spherical(wgs84_coord(0.0, 0.0), 90.0, wgs84_coord(0.0, 5.0), 90.0)
assert res.is_SAME_GREAT_CIRCLE(), res
xt = gc.cross_track_distance_spherical_m(
    wgs84_coord(1.0, 5.0), wgs84_coord(0.0, 0.0), wgs84_coord(0.0, 10.0)
)
assert approx(xt, -111_195.080_233, 1e-3), xt
at = gc.along_track_distance_spherical_m(
    wgs84_coord(1.0, 5.0), wgs84_coord(0.0, 0.0), wgs84_coord(0.0, 10.0)
)
assert approx(at, 555_975.401_168, 1e-3), at
# Geodesics run on the CRS's own ellipsoid and reject the obfuscated datums.
tokyo = gc.Coordinate(lat=35.0, lon=139.0, height=None, crs=gc.Crs.TOKYO)
assert gc.destination(tokyo, 0.0, 1_000.0).crs == gc.Crs.TOKYO
mars = gc.Coordinate(lat=30.0, lon=110.0, height=None, crs=gc.Crs.GCJ02)
try:
    gc.geodesic_distance_m(mars, mars)
    raise AssertionError("expected CrsMismatch for GCJ-02")
except gc.GeoError.CrsMismatch:
    pass

# --- Classic datums (Helmert / DatumTransform) ---
assert gc.datum_transform_catalog_to_wgs84(gc.Crs.WGS84) is None  # no shift needed
assert gc.datum_transform_catalog_to_wgs84(gc.Crs.GCJ02) is None  # use China conversions
dt = gc.datum_transform_catalog_to_wgs84(gc.Crs.NAD27)
assert dt is not None and approx(dt.helmert.tx_m, -8.0, 1e-12)
# TR8350.2 publishes +-5, +-5, +-6 m for the NAD27 mean shift; none for Pulkovo-1942.
assert approx(dt.accuracy_m, 86 ** 0.5, 1e-12), dt.accuracy_m
assert gc.datum_transform_catalog_to_wgs84(gc.Crs.PULKOVO42).accuracy_m is None
# Translation-only Helmert round-trips to sub-millimeter geodetic precision.
nad27 = gc.Coordinate(lat=40.0, lon=-100.0, height=None, crs=gc.Crs.NAD27)
w = gc.datum_transform_apply(dt, nad27)
assert w.crs == gc.Crs.WGS84 and approx(w.lat, 40.000_009_482_759, 1e-7)
back = gc.datum_transform_apply(gc.datum_transform_inverse_first_order(dt), w)
assert back.crs == gc.Crs.NAD27 and approx(back.lat, 40.0, 1e-8) and approx(back.lon, -100.0, 1e-8)
# A hand-built transform cannot name a system without an ellipsoid.
try:
    gc.datum_transform_new(gc.Crs.GCJ02, gc.Crs.WGS84, gc.helmert_identity(), None)
    raise AssertionError("expected InvalidValue: GCJ-02 is not a datum")
except gc.GeoError.InvalidValue as e:
    assert e.field == "from_crs", e.field
# convert reports the Helmert leg's published accuracy as its bound.
assert approx(gc.convert(nad27, gc.Crs.WGS84).max_error_m, 86 ** 0.5, 1e-12)
# apply_ecef + inverse identity at the ECEF level.
ident = gc.helmert_inverse_first_order(gc.helmert_identity())
e = gc.helmert_apply_ecef(ident, gc.Ecef(x=4_000_000.0, y=-2_000_000.0, z=4_500_000.0))
assert approx(e.x, 4_000_000.0, 1e-6)

# --- Runtime conversion dispatch ---
assert gc.convert(bd09_coord(BD[0], BD[1]), gc.Crs.TOKYO).coord.crs == gc.Crs.TOKYO
# WGS-84 -> GCJ-02 is exact (bound 0); the inverse is approximate (bound > 0).
fwd = gc.convert(wgs84_coord(WGS[0], WGS[1]), gc.Crs.GCJ02)
assert fwd.max_error_m == 0.0 and fwd.coord.crs == gc.Crs.GCJ02
assert approx(fwd.coord.lat, GCJ[0], 1e-9)
inv = gc.convert(gcj02_coord(GCJ[0], GCJ[1]), gc.Crs.WGS84)
assert inv.max_error_m == 0.5 and approx(inv.coord.lat, WGS[0], 1e-9)  # refined inverse bound
assert inv.coord.crs == gc.Crs.WGS84 and approx(inv.coord.lon, WGS[1], 1e-9)
# GCJ-02 -> BD-09 stays exact via the direct path.
gb = gc.convert(gcj02_coord(GCJ[0], GCJ[1]), gc.Crs.BD09)
assert gb.max_error_m == 0.0 and approx(gb.coord.lat, BD[0], 1e-9)

# --- UTM / UPS / MGRS ---
u = gc.utm_from_coordinate(wgs84_coord(40.0, -75.0))
assert u.zone == 18 and u.hemisphere == gc.UtmHemisphere.NORTH
assert approx(u.easting, 500_000.0, 1e-3) and approx(u.northing, 4_427_757.2188, 1e-3)
roundtrip = gc.utm_to_coordinate(u)
assert approx(roundtrip.lat, 40.0, 1e-9) and approx(roundtrip.lon, -75.0, 1e-9)
try:
    gc.utm_from_coordinate(wgs84_coord(85.0, 0.0))  # polar -> error
    raise AssertionError("expected OutsideDomain for a polar latitude")
except gc.GeoError.OutsideDomain as e:
    assert e.operation == "UTM"
# Zone 1 west of the antimeridian decodes wrapped; a past-the-pole northing is rejected.
west = gc.utm_to_coordinate(gc.Utm(zone=1, hemisphere=gc.UtmHemisphere.NORTH, easting=150_000.0, northing=0.0))
assert approx(west.lon, 179.856227657, 1e-8)
try:
    gc.utm_to_coordinate(gc.Utm(zone=1, hemisphere=gc.UtmHemisphere.NORTH, easting=500_000.0, northing=9_999_000.0))
    raise AssertionError("expected InvalidValue for a northing past the pole")
except gc.GeoError.InvalidValue as e:
    assert e.field == "northing"
assert gc.utm_zone_for(wgs84_coord(60.0, 5.0)) == 32  # Norway exception
assert gc.utm_central_meridian_deg(18) == -75.0
try:
    gc.utm_central_meridian_deg(0)
    raise AssertionError("expected InvalidValue for zone 0")
except gc.GeoError.InvalidValue as e:
    assert e.field == "zone"
ups = gc.ups_from_coordinate(wgs84_coord(85.0, 0.0))
assert ups.hemisphere == gc.UtmHemisphere.NORTH and approx(ups.easting, 2_000_000.0, 1e-3)
assert gc.mgrs_from_coordinate(wgs84_coord(40.0, -75.0), gc.MgrsPrecision.M1) == "18TWK0000027757"
assert gc.mgrs_precision("18TWK000277") == gc.MgrsPrecision.M100
assert gc.mgrs_precision_meters(gc.MgrsPrecision.KM10) == 10_000
dec = gc.mgrs_to_coordinate("18TWK0000027757")
assert approx(dec.max_error_m, math.sqrt(0.5) / 0.9996, 1e-12)  # half-diagonal / UTM k0
assert approx(dec.coord.lat, 40.0, 1e-3)
try:
    gc.mgrs_to_coordinate("not-an-mgrs")
    raise AssertionError("expected error for a bad MGRS string")
except gc.GeoError.InvalidGridRef as e:
    assert e.detail == "not-an-mgrs", e.detail

# --- Interchange formats (GeoJSON / WKT / GPX / KML) ---
gj = gc.from_geojson('{"type":"Point","coordinates":[2.0,9.0,100.0]}')
assert len(gj) == 1 and approx(gj[0].coord.lat, 9.0, 1e-9) and approx(gj[0].coord.lon, 2.0, 1e-9)
assert gj[0].source.axis_order == gc.AxisOrder.LON_LAT
assert gj[0].coord.height == gc.Height.ELLIPSOIDAL(100.0)
# Each fix's raw source is its own position, not the whole document.
assert gj[0].source.raw == '{"type":"Point","coordinates":[2.0,9.0,100.0]}', gj[0].source.raw
# WKT defines no axis order, so the caller states it.
wktfixes = gc.from_wkt("LINESTRING(0 0, 10 20)", gc.AxisOrder.LON_LAT)
assert len(wktfixes) == 2 and approx(wktfixes[1].coord.lat, 20.0, 1e-9)
assert wktfixes[1].source.raw == "POINT(10 20)"
latlon = gc.from_wkt("POINT(20 10)", gc.AxisOrder.LAT_LON)
assert approx(latlon[0].coord.lat, 20.0, 1e-9) and latlon[0].source.axis_order == gc.AxisOrder.LAT_LON
gpxfixes = gc.from_gpx(
    '<?xml version="1.0"?><gpx version="1.1" creator="t"'
    ' xmlns="http://www.topografix.com/GPX/1/1"><wpt lat="48.2" lon="16.3"><ele>183</ele>'
    "<time>2024-03-01T12:00:05Z</time><hdop>1.5</hdop></wpt></gpx>"
)
assert len(gpxfixes) == 1 and approx(gpxfixes[0].coord.lat, 48.2, 1e-9)
assert gpxfixes[0].source.axis_order is None  # GPX labels its axes
assert gpxfixes[0].coord.height == gc.Height.ORTHOMETRIC(183.0)
assert gpxfixes[0].timestamp is not None
assert approx(gpxfixes[0].accuracy.horizontal_m, 1.5 * 5.0, 1e-9)  # HDOP x nominal UERE
kmlfixes = gc.from_kml(
    '<?xml version="1.0"?><kml xmlns="http://www.opengis.net/kml/2.2"><Document>'
    "<Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark></Document></kml>"
)
assert len(kmlfixes) == 1 and approx(kmlfixes[0].coord.lon, 16.3, 1e-9)
assert kmlfixes[0].coord.height == gc.Height.ORTHOMETRIC(183.0)
try:
    gc.from_geojson('{"type":"LineString","coordinates":[[0,0],[181,0]]}')
    raise AssertionError("expected an out-of-range position to be rejected")
except gc.GeoError.Parse as e:
    assert "invalid position at index 1" in e.detail, e.detail
try:
    gc.from_geojson("not json")
    raise AssertionError("expected a parse error")
except gc.GeoError.Parse:
    pass

# --- NMEA 0183 sentences ---
nmea = gc.from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")
assert approx(nmea.coord.lat, 48.0 + 7.038 / 60.0, 1e-9)
assert approx(nmea.coord.lon, 11.0 + 31.0 / 60.0, 1e-9)
assert nmea.source.axis_order == gc.AxisOrder.LAT_LON  # NMEA lists lat first
assert approx(nmea.accuracy.horizontal_m, 0.9 * 5.0, 1e-9)  # HDOP x nominal UERE
assert nmea.timestamp is None  # GGA has a time of day but no date
rmc = gc.from_nmea_sentence("$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10")
assert rmc.timestamp is not None
assert rmc.timestamp.timestamp() == 764_426_119  # 1994-03-23T12:35:19Z
# A sentence reporting no fix is an error, never a position.
for no_fix, reason in [
    ("$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07", "void"),
    ("$GPGLL,4916.45,N,12311.12,W,225444,V", "void"),
    ("$GPGGA,123519,,,,,0,00,,,M,,M,,", "quality 0"),
]:
    try:
        gc.from_nmea_sentence(no_fix)
        raise AssertionError(f"expected {no_fix!r} to be rejected")
    except gc.GeoError.Parse as e:
        assert reason in e.detail, e.detail
try:
    gc.from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E")  # bad checksum
    raise AssertionError("expected a checksum error")
except gc.GeoError.Parse as e:
    assert e.detail.startswith("NMEA checksum mismatch"), e.detail

# --- H3 and S2 discrete global grids ---
h3 = gc.h3_encode(wgs84_coord(40.7128, -74.006), 9)
assert h3.value == 617733151020810239  # canonical H3 index
h3dec = gc.h3_decode(h3)
assert approx(h3dec.max_error_m, 205.872_541, 1e-5), h3dec.max_error_m  # cell radius
s2 = gc.s2_encode(wgs84_coord(40.7128, -74.006), 20)
assert s2.value == 9_926_595_630_970_437_632
s2dec = gc.s2_decode(s2)
assert approx(s2dec.max_error_m, 6.337_541, 1e-5), s2dec.max_error_m  # max corner distance
assert approx(h3dec.coord.lat, 40.712_378_204, 1e-8) and approx(h3dec.coord.lon, -74.005_642_577, 1e-8)
assert approx(s2dec.coord.lat, 40.712_762_829, 1e-8) and approx(s2dec.coord.lon, -74.006_004_053, 1e-8)
try:
    gc.h3_encode(gcj02_coord(40.7128, -74.006), 9)
    raise AssertionError("expected CrsMismatch for GCJ-02 H3 input")
except gc.GeoError.CrsMismatch:
    pass
try:
    gc.s2_encode(bd09_coord(40.7128, -74.006), 20)
    raise AssertionError("expected CrsMismatch for BD-09 S2 input")
except gc.GeoError.CrsMismatch:
    pass
try:
    gc.h3_encode(wgs84_coord(40.7128, -74.006), 16)
    raise AssertionError("expected InvalidValue for H3 resolution 16")
except gc.GeoError.InvalidValue as e:
    assert e.field == "H3 resolution" and e.detail == "must be in 0..=15", (e.field, e.detail)
try:
    gc.h3_decode(gc.H3Cell(value=0))
    raise AssertionError("expected InvalidCellId for H3 index 0")
except gc.GeoError.InvalidCellId as e:
    assert e.grid == "H3" and e.value == 0, (e.grid, e.value)

print("python smoke OK")
