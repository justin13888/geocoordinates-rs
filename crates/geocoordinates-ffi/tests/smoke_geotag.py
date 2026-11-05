"""Geotag part of the Python smoke test.

Not run on its own: `smoke.py` executes it with its own globals (`gc`), so it shares
that file's bindings import and setup.
"""

import datetime

utc = datetime.timezone.utc


def at(second: int) -> datetime.datetime:
    return datetime.datetime.fromtimestamp(second, tz=utc)


def timed(lat: float, lon: float, second: int) -> "gc.Fix":
    coord = gc.coordinate_new(lat, lon, gc.Crs.WGS84)
    return gc.Fix(coord=coord, accuracy=None, timestamp=at(second), source=None)


track = gc.Track([timed(0.0, 0.01, 1_100), timed(0.0, 0.0, 1_000)])
assert (track.len(), track.dropped_untimed()) == (2, 0)
options = gc.geotag_options_default()
assert options.max_gap == datetime.timedelta(minutes=30), options
options.clock_offset_ms = -10_000  # a camera 10 s fast
located = track.locate(at(1_035), options)
assert abs(located.coord.lon - 0.0025) < 1e-9, located
assert located.timestamp == at(1_025), located.timestamp
assert located.source.raw.startswith("geotag:interpolated"), located.source
assert track.locate(at(5_000), gc.geotag_options_default()) is None
reference = gc.GeotagReference(photo_time=at(1_650), location=gc.coordinate_new(0.0, 0.005, gc.Crs.WGS84))
assert gc.estimate_offset_ms(track, [reference]) == -600_000
try:
    gc.estimate_offset_ms(track, [])
    raise AssertionError("no references must raise")
except gc.GeoError.InvalidValue as e:
    assert e.field == "references", e
