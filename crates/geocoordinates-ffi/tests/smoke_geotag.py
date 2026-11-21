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

# pre_epoch_and_subsecond_timestamps_round_trip_through_a_track: an instant
# before 1970 with a fractional second crosses both ways unchanged. The fixes
# are 0.0005 degrees (about 56 m) apart, inside the default 50 m/s speed limit.
# Every instant is at least a second before the epoch: one inside the last
# second crosses with its sign flipped (#342).
before = datetime.datetime(1969, 12, 31, 23, 59, 56, 250_000, tzinfo=utc)
after = datetime.datetime(1969, 12, 31, 23, 59, 58, 250_000, tzinfo=utc)
pre_epoch = gc.Track(
    [
        gc.Fix(coord=gc.coordinate_new(0.0, 0.0005, gc.Crs.WGS84), accuracy=None, timestamp=after, source=None),
        gc.Fix(coord=gc.coordinate_new(0.0, 0.0, gc.Crs.WGS84), accuracy=None, timestamp=before, source=None),
    ]
)
assert [f.timestamp for f in pre_epoch.fixes()] == [before, after], pre_epoch.fixes()
midway = datetime.datetime(1969, 12, 31, 23, 59, 57, 250_000, tzinfo=utc)
between = pre_epoch.locate(midway, gc.geotag_options_default())
assert between.timestamp == midway, between.timestamp
assert abs(between.coord.lon - 0.00025) < 1e-12, between.coord
