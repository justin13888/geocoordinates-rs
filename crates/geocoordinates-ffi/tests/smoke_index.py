"""Geographic bounds and spatial index part of the Python smoke test.

Not run on its own: `smoke.py` executes it with its own globals (`gc`), so it shares
that file's bindings import and setup.
"""

# Geographic bounds and the geodesic spatial index: an object across the boundary.
fiji = gc.geo_bounds_from_points(
    [gc.coordinate_new(-17.7, 178.0, gc.Crs.WGS84), gc.coordinate_new(-16.8, -179.9, gc.Crs.WGS84)]
)
assert gc.geo_bounds_crosses_antimeridian(fiji)
index = gc.SpatialIndex([gc.coordinate_new(0.0, 179.9, gc.Crs.WGS84), gc.coordinate_new(0.0, 0.0, gc.Crs.WGS84)])
assert index.len() == 2
hits = index.nearest(gc.coordinate_new(0.0, -179.9, gc.Crs.WGS84), 1)
assert hits[0].index == 0 and hits[0].distance_m > 0.0, hits
try:
    index.within(hits[0].coordinate, -1.0)
    raise AssertionError("expected InvalidValue for a negative radius")
except gc.GeoError.InvalidValue as e:
    assert e.field == "radius", e.field
