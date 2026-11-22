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

# an_instant_in_the_last_second_before_the_epoch_crosses_with_its_sign_lost:
# pins UniFFI's Timestamp window documented in the FFI README (#342). The wire
# format keeps the sign on the whole seconds, which are zero here, so the
# instant arrives as its mirror image after the epoch. When a UniFFI release
# keeps the sign this fails: drop the README caveat and the field docs with it.
in_window = datetime.datetime(1969, 12, 31, 23, 59, 59, 250_000, tzinfo=utc)
mirror = datetime.datetime(1970, 1, 1, 0, 0, 0, 750_000, tzinfo=utc)
windowed = gc.Track(
    [
        gc.Fix(coord=gc.coordinate_new(0.0, 0.0, gc.Crs.WGS84), accuracy=None, timestamp=in_window, source=None),
    ]
)
assert [f.timestamp for f in windowed.fixes()] == [mirror], windowed.fixes()
# Track.locate's `at`: a query in the window between fixes at -1.75 s and
# +0.25 s arrives as +0.75 s, after the last fix, so the track does not cover it.
two_before = datetime.datetime(1969, 12, 31, 23, 59, 58, 250_000, tzinfo=utc)
just_after = datetime.datetime(1970, 1, 1, 0, 0, 0, 250_000, tzinfo=utc)
spanning = gc.Track(
    [
        gc.Fix(coord=gc.coordinate_new(0.0, 0.0, gc.Crs.WGS84), accuracy=None, timestamp=two_before, source=None),
        gc.Fix(coord=gc.coordinate_new(0.0, 0.0001, gc.Crs.WGS84), accuracy=None, timestamp=just_after, source=None),
    ]
)
assert spanning.locate(two_before, gc.geotag_options_default()) is not None
assert spanning.locate(in_window, gc.geotag_options_default()) is None
# GeotagReference.photo_time: the track reached `here` at +0.25 s, so a photo
# taken there at -0.75 s would give +1000 ms; it arrives as +0.75 s and gives
# -500 ms. An instant outside the window, at -1.25 s, gives its true +1500 ms.
here = gc.coordinate_new(0.0, 0.0001, gc.Crs.WGS84)
outside = datetime.datetime(1969, 12, 31, 23, 59, 58, 750_000, tzinfo=utc)
assert gc.estimate_offset_ms(spanning, [gc.GeotagReference(photo_time=in_window, location=here)]) == -500
assert gc.estimate_offset_ms(spanning, [gc.GeotagReference(photo_time=outside, location=here)]) == 1_500
