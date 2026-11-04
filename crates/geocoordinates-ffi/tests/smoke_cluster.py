"""Clustering part of the Python smoke test.

Not run on its own: `smoke.py` executes it with its own globals (`gc`), so it shares
that file's bindings import and setup.
"""

import datetime

pair = [gc.coordinate_new(0.0, 179.99, gc.Crs.WGS84), gc.coordinate_new(0.0, -179.99, gc.Crs.WGS84)]
middle = gc.centroid(pair)
assert abs(abs(middle.coord.lon) - 180.0) < 1e-9 and middle.max_error_m > 0.0, middle
found = gc.dbscan(pair + [gc.coordinate_new(40.0, 0.0, gc.Crs.WGS84)], 3000.0, 2)
assert found.clusters[0].members == [0, 1], found.clusters
assert found.labels[2] == gc.PointLabel.NOISE(), found.labels
buckets = gc.bucket_by_geohash(pair, 4)
assert buckets[0].key != buckets[1].key, buckets
when = datetime.datetime(2024, 1, 1, tzinfo=datetime.timezone.utc)
stay = [
    gc.Fix(coord=pair[0], accuracy=None, timestamp=when, source=None),
    gc.Fix(coord=pair[1], accuracy=None, timestamp=when + datetime.timedelta(hours=1), source=None),
]
visits = gc.segment_visits(stay, 3000.0, datetime.timedelta(minutes=30))
assert len(visits) == 1 and visits[0].cluster.count == 2, visits
assert gc.segment_trips(visits, datetime.timedelta(hours=1))[0].visits == [0]
pins = gc.ClusterIndex(pair, gc.cluster_index_options_default())
whole = gc.GeoBounds(south=-90.0, west=-180.0, north=90.0, east=180.0, crs=gc.Crs.WGS84)
top = pins.clusters(whole, 2)
assert len(top) == 1 and top[0].count == 2, top
assert sorted(pins.leaves(top[0].id, 10, 0)) == [0, 1]
added = pins.insert(gc.coordinate_new(40.0, 0.0, gc.Crs.WGS84))
assert len(pins.clusters(whole, 2)) == 2
pins.remove(added)
assert len(pins.clusters(whole, 2)) == 1
