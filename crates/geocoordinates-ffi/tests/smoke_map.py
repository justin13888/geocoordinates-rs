"""Map-view part of the Python smoke test.

Not run on its own: `smoke.py` executes it with its own globals (`gc`), so it shares
that file's bindings import and setup.
"""

import json

denver = gc.coordinate_new(40.0, -105.0, gc.Crs.WGS84)
tile = gc.tile_from_coordinate(denver, 14)
assert (tile.x, tile.y, tile.z) == (3413, 6202, 14), tile
assert gc.tile_quadkey(gc.Tile(z=10, x=486, y=332)) == "0313102310"
fiji = gc.GeoBounds(south=-21.0, west=176.0, north=-12.0, east=-161.5, crs=gc.Crs.WGS84)
view = gc.Viewport(width_px=512.0, height_px=512.0, tile_px=512.0)
camera = gc.fit_bounds(fiji, view, 0.0, 18.0)
assert abs(camera.zoom - 4.0) < 1e-9 and abs(camera.center.lon + 172.75) < 1e-9, camera
shown = gc.viewport_bounds(camera, view)
assert shown.west > shown.east, shown  # across the antimeridian
pins = gc.ClusterIndex([gc.coordinate_new(0.0, 0.0, gc.Crs.WGS84)] * 2, gc.cluster_index_options_default())
whole = gc.GeoBounds(south=-90.0, west=-180.0, north=90.0, east=180.0, crs=gc.Crs.WGS84)
layer = json.loads(gc.clusters_in_view_geojson(pins, whole, 3.5, gc.DisplayDatum.WGS84))
assert layer["features"][0]["properties"]["point_count"] == 2, layer
mars = gc.display_coordinate(gc.coordinate_new(39.9087, 116.3975, gc.Crs.WGS84), gc.DisplayDatum.GCJ02)
assert gc.tap_to_wgs84(mars).max_error_m == 0.5
