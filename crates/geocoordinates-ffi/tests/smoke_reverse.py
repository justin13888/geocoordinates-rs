"""Reverse-geocoding part of the Python smoke test, including a foreign
`ReverseGeocoder` implementation.

Not run on its own: `smoke.py` executes it with its own globals (`gc`, `pathlib`),
so it shares that file's bindings import and setup.
"""

fixture = pathlib.Path(__file__).resolve().parents[3] / "src" / "reverse" / "data" / "fixture.gaz"
gazetteer = gc.Gazetteer.from_bytes(fixture.read_bytes())
assert gazetteer.len() == 4 and gazetteer.languages() == ["fr", "ja"]
assert gc.Gazetteer.open(str(fixture)).source_date() == "2026-11-03"

options = gc.reverse_options_default()
options.languages = ["ja"]
paris = gazetteer.reverse(gc.coordinate_new(48.86, 2.35, gc.Crs.WGS84), options)
assert paris.name == "Paris" and paris.country_code == "FR", paris
assert paris.localized_names == {"ja": "パリ"}, paris.localized_names
assert paris.feature_class == gc.FeatureClass.CAPITAL and paris.query_max_error_m == 0.0
assert gazetteer.reverse(gc.coordinate_new(0.0, -30.0, gc.Crs.WGS84), gc.reverse_options_default()) is None
try:
    gc.Gazetteer.from_bytes(b"GCGZ")
    raise AssertionError("expected a Parse error for a truncated file")
except gc.GeoError.Parse as e:
    assert e.kind == gc.ParseErrorKind.SYNTAX, e.kind


class Everywhere(gc.ReverseGeocoder):
    """A foreign geocoder: every point is in one fixed place, as a platform
    provider adapter would answer."""

    def __init__(self, country_code):
        self.country_code = country_code

    def reverse(self, coordinate, options):
        if coordinate.lat < -80.0:
            return None
        here = gc.coordinate_new(0.0, 0.0, gc.Crs.WGS84)
        return gc.place_new("Everywhere", here, self.country_code, coordinate, 0.0)


at = gc.coordinate_new(0.0, 1.0, gc.Crs.WGS84)
found = gc.reverse_geocode(Everywhere("XX"), at, gc.reverse_options_default())
assert found.name == "Everywhere" and found.distance_m > 100_000.0, found
assert gc.reverse_geocode(Everywhere("XX"), gc.coordinate_new(-85.0, 0.0, gc.Crs.WGS84), gc.reverse_options_default()) is None
assert gc.reverse_geocode(gc.gazetteer_as_reverse_geocoder(gazetteer), paris.query, options).name == "Paris"


class Lying(gc.ReverseGeocoder):
    """A foreign geocoder that returns a place that breaks the rules."""

    def reverse(self, coordinate, options):
        place = gc.place_new("Here", coordinate, "XX", coordinate, 0.0)
        place.country_code = "not a code"
        return place


try:
    gc.reverse_geocode(Lying(), at, gc.reverse_options_default())
    raise AssertionError("expected InvalidValue for a foreign place with a bad country")
except gc.GeoError.InvalidValue as e:
    assert e.field == "country_code", e.field
