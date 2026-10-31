"""Ingestion part of the Python smoke test: the interchange formats and NMEA.

Not run on its own: `smoke.py` executes it with its own globals (`gc` and the
`approx` helper), so it shares that file's bindings import and setup.
"""

# --- Interchange formats (GeoJSON / WKT / GPX / KML) ---
gj = gc.parse_geojson('{"type":"Point","coordinates":[2.0,9.0,100.0]}')
assert len(gj) == 1 and approx(gj[0].coord.lat, 9.0, 1e-9) and approx(gj[0].coord.lon, 2.0, 1e-9)
assert gj[0].source.axis_order == gc.AxisOrder.LON_LAT
assert gj[0].coord.height == gc.Height.ELLIPSOIDAL(100.0)
# Each fix's raw source is its own position, not the whole document.
assert gj[0].source.raw == '{"type":"Point","coordinates":[2.0,9.0,100.0]}', gj[0].source.raw
# WKT defines no axis order, so the caller states it.
wktfixes = gc.parse_wkt("LINESTRING(0 0, 10 20)", gc.AxisOrder.LON_LAT)
assert len(wktfixes) == 2 and approx(wktfixes[1].coord.lat, 20.0, 1e-9)
assert wktfixes[1].source.raw == "POINT(10 20)"
latlon = gc.parse_wkt("POINT(20 10)", gc.AxisOrder.LAT_LON)
assert approx(latlon[0].coord.lat, 20.0, 1e-9) and latlon[0].source.axis_order == gc.AxisOrder.LAT_LON
gpxfixes = gc.parse_gpx(
    '<?xml version="1.0"?><gpx version="1.1" creator="t"'
    ' xmlns="http://www.topografix.com/GPX/1/1"><wpt lat="48.2" lon="16.3"><ele>183</ele>'
    "<time>2024-03-01T12:00:05Z</time><hdop>1.5</hdop></wpt></gpx>"
)
assert len(gpxfixes) == 1 and approx(gpxfixes[0].coord.lat, 48.2, 1e-9)
assert gpxfixes[0].source.axis_order is None  # GPX labels its axes
assert gpxfixes[0].coord.height == gc.Height.ORTHOMETRIC(183.0)
assert gpxfixes[0].timestamp is not None
assert approx(gpxfixes[0].accuracy.horizontal_m, 1.5 * 5.0, 1e-9)  # HDOP x nominal UERE
assert gpxfixes[0].accuracy.kind == gc.AccuracyKind.ONE_SIGMA
kmlfixes = gc.parse_kml(
    '<?xml version="1.0"?><kml xmlns="http://www.opengis.net/kml/2.2"><Document>'
    "<Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark></Document></kml>"
)
assert len(kmlfixes) == 1 and approx(kmlfixes[0].coord.lon, 16.3, 1e-9)
assert kmlfixes[0].coord.height == gc.Height.ORTHOMETRIC(183.0)
try:
    gc.parse_geojson('{"type":"LineString","coordinates":[[0,0],[181,0]]}')
    raise AssertionError("expected an out-of-range position to be rejected")
except gc.GeoError.OutOfRange as e:
    assert e.lat == 0.0 and e.lon == 181.0, (e.lat, e.lon)
try:
    gc.parse_geojson("not json")
    raise AssertionError("expected a parse error")
except gc.GeoError.Parse as e:
    assert e.kind == gc.ParseErrorKind.SYNTAX, e.kind

# --- NMEA 0183 sentences ---
nmea = gc.parse_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")
assert approx(nmea.coord.lat, 48.0 + 7.038 / 60.0, 1e-9)
assert approx(nmea.coord.lon, 11.0 + 31.0 / 60.0, 1e-9)
assert nmea.source.axis_order == gc.AxisOrder.LAT_LON  # NMEA lists lat first
assert approx(nmea.accuracy.horizontal_m, 0.9 * 5.0, 1e-9)  # HDOP x nominal UERE
assert nmea.accuracy.kind == gc.AccuracyKind.ONE_SIGMA
assert nmea.source.geoid_separation_m == 46.9  # h = H + N
assert nmea.source.notes == []
assert nmea.timestamp is None  # GGA has a time of day but no date
rmc = gc.parse_nmea_sentence("$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10")
assert rmc.timestamp is not None
assert rmc.timestamp.timestamp() == 764_426_119  # 1994-03-23T12:35:19Z
# A sentence reporting no fix is an error, never a position.
for no_fix, reason in [
    ("$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07", "void"),
    ("$GPGLL,4916.45,N,12311.12,W,225444,V", "void"),
    ("$GPGGA,123519,,,,,0,00,,,M,,M,,", "quality 0"),
]:
    try:
        gc.parse_nmea_sentence(no_fix)
        raise AssertionError(f"expected {no_fix!r} to be rejected")
    except gc.GeoError.Parse as e:
        assert e.kind == gc.ParseErrorKind.NO_FIX and reason in e.detail, (e.kind, e.detail)
try:
    gc.parse_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E")  # bad checksum
    raise AssertionError("expected a checksum error")
except gc.GeoError.Parse as e:
    assert e.kind == gc.ParseErrorKind.CHECKSUM, (e.kind, e.detail)


# --- EXIF / XMP GPS tags: both paths reach the same capture ---
def whole(n):
    return gc.ExifRational.UNSIGNED(numerator=n, denominator=1)


ifd = gc.ExifGps(
    latitude=gc.ExifDms(degrees=whole(39), minutes=whole(54), seconds=whole(0)),
    latitude_ref="N",
    longitude=gc.ExifDms(degrees=whole(116), minutes=whole(24), seconds=whole(0)),
    longitude_ref="E",
    altitude=whole(10),
    altitude_ref=1,
    speed=whole(36),
    speed_ref="K",
)
capture = gc.parse_exif(ifd)
assert (capture.fix.coord.lat, capture.fix.coord.lon) == (39.9, 116.4), capture.fix.coord
assert capture.fix.coord.height == gc.Height.ORTHOMETRIC(meters=-10.0)
assert capture.fix.source.datum_ambiguity == gc.DatumAmbiguity.POSSIBLY_GCJ02
assert capture.speed_mps == 10.0
xmp = gc.parse_xmp(gc.XmpGps(latitude="39,54.0N", longitude="116,24,0E"))
assert (xmp.fix.coord.lat, xmp.fix.coord.lon) == (39.9, 116.4), xmp.fix.coord
ifd.latitude_ref = "Q"
try:
    gc.parse_exif(ifd)
    raise AssertionError("expected InvalidValue for a bad Ref")
except gc.GeoError.InvalidValue as e:
    assert e.field == "latitude_ref", e.field
try:
    gc.parse_xmp_with(
        gc.XmpGps(latitude="0,0.0N", longitude="0,0.0E"),
        gc.ExifOptions(null_island=gc.NullIslandPolicy.REJECT),
    )
    raise AssertionError("expected Parse for null island")
except gc.GeoError.Parse as e:
    assert e.kind == gc.ParseErrorKind.NO_FIX, e.kind
