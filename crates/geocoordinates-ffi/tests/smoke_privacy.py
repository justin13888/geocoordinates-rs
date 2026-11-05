"""Location-privacy part of the Python smoke test.

Not run on its own: `smoke.py` executes it with its own globals (`gc`), so it shares
that file's bindings import and setup.
"""

home = gc.coordinate_new(57.64911, 10.40744, gc.Crs.WGS84)
precision = gc.precision_at_least(1_000.0)
assert precision == gc.Precision(grid=gc.SnapGrid.GEOHASH, level=5), precision
shared = gc.coarsen(home, precision)
assert shared.source.raw == "privacy:geohash:u4pru", shared.source
assert shared.accuracy.kind == gc.AccuracyKind.BOUND, shared.accuracy
assert gc.coarsen(shared.coord, precision) == shared  # idempotent

zones = [gc.ExclusionZone.CIRCLE(center=home, radius_m=200.0)]
assert gc.exclusion_zone_contains(zones[0], home)
fix = gc.Fix(coord=home, accuracy=None, timestamp=None, source=None)
assert gc.apply_exclusion_zones(fix, zones, gc.ZoneAction.STRIP()) is None
coarse = gc.apply_exclusion_zones(fix, zones, gc.ZoneAction.COARSEN(precision=precision))
assert coarse.coord == shared.coord, coarse
away = gc.Fix(coord=gc.coordinate_new(48.8584, 2.2945, gc.Crs.WGS84), accuracy=None, timestamp=None, source=None)
assert gc.apply_exclusion_zones(away, zones, gc.ZoneAction.STRIP()) == away
try:
    gc.exclusion_zone_validate(gc.ExclusionZone.CIRCLE(center=home, radius_m=-1.0))
    raise AssertionError("a negative radius must raise")
except gc.GeoError.InvalidValue as e:
    assert e.field == "radius", e
