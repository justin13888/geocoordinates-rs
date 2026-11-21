"""Fixture tests for `scripts/check_public_paths.py`.

Each test builds a small rustdoc JSON index (the subset of the format the
script reads), writes it to a file, and runs the script's `main` on it with the
item floor lowered to fit the fixture. Run through `mise run scripts-test`.
"""

import contextlib
import io
import json
import pathlib
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import check_public_paths  # noqa: E402


class Crate:
    """A rustdoc JSON index under construction, rooted at module `fixture`."""

    def __init__(self):
        self.index = {}
        self.root = self.module("fixture")

    def add(self, name, inner, visibility="public"):
        item_id = len(self.index) + 1
        self.index[str(item_id)] = {
            "name": name,
            "visibility": visibility,
            "inner": inner,
        }
        return item_id

    def module(self, name, parent=None, visibility="public"):
        item_id = self.add(name, {"module": {"items": []}}, visibility)
        if parent is not None:
            self.child(parent, item_id)
        return item_id

    def struct(self, name, parent, visibility="public"):
        item_id = self.add(name, {"struct": {}}, visibility)
        self.child(parent, item_id)
        return item_id

    def enum(self, name, parent, variants):
        ids = [self.add(v, {"variant": {}}, "default") for v in variants]
        item_id = self.add(name, {"enum": {"variants": ids}})
        self.child(parent, item_id)
        return item_id

    def reexport(self, parent, name, target, source, is_glob=False):
        use = {"name": name, "id": target, "source": source, "is_glob": is_glob}
        self.child(parent, self.add(None, {"use": use}))

    def child(self, parent, item_id):
        self.index[str(parent)]["inner"]["module"]["items"].append(item_id)

    def check(self, min_items=1):
        """The script's exit status and its stderr, under the given item floor."""
        doc = {"root": self.root, "index": self.index}
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "fixture.json"
            path.write_text(json.dumps(doc), encoding="utf-8")
            err = io.StringIO()
            with (
                mock.patch.object(check_public_paths, "MIN_ITEMS", min_items),
                contextlib.redirect_stdout(io.StringIO()),
                contextlib.redirect_stderr(err),
            ):
                status = check_public_paths.main(["check_public_paths.py", str(path)])
        return status, err.getvalue()


class CheckPublicPaths(unittest.TestCase):
    def test_one_path_per_item_passes(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        crate.struct("Point", geo)
        self.assertEqual(crate.check(), (0, ""))

    def test_second_path_fails(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        point = crate.struct("Point", geo)
        crate.reexport(crate.root, "Point", point, "geo::Point")
        status, err = crate.check()
        self.assertEqual(status, 1)
        self.assertIn("fixture::Point  |  fixture::geo::Point", err)

    def test_root_prelude_is_sanctioned(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        point = crate.struct("Point", geo)
        prelude = crate.module("prelude", crate.root)
        crate.reexport(prelude, "Point", point, "crate::geo::Point")
        self.assertEqual(crate.check(), (0, ""))

    def test_nested_prelude_is_not_sanctioned(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        point = crate.struct("Point", geo)
        prelude = crate.module("prelude", geo)
        crate.reexport(prelude, "Point", point, "crate::geo::Point")
        status, err = crate.check()
        self.assertEqual(status, 1)
        self.assertIn("fixture::geo::prelude::Point", err)

    def test_glob_of_a_module_is_walked(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        inner = crate.module("inner", crate.root, visibility="crate")
        crate.struct("Point", inner)
        crate.reexport(geo, "inner", inner, "crate::inner", is_glob=True)
        crate.reexport(crate.root, "inner", inner, "inner", is_glob=True)
        status, err = crate.check()
        self.assertEqual(status, 1)
        self.assertIn("fixture::Point  |  fixture::geo::Point", err)

    def test_glob_reexport_is_counted(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        kind = crate.enum("Kind", geo, ["Near", "Far"])
        crate.reexport(geo, "Kind", kind, "self::Kind", is_glob=True)
        status, err = crate.check()
        self.assertEqual(status, 1)
        self.assertIn("fixture::geo::Kind::Near  |  fixture::geo::Near", err)
        self.assertIn("fixture::geo::Far  |  fixture::geo::Kind::Far", err)

    def test_enum_variants_alone_pass(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        crate.enum("Kind", geo, ["Near", "Far"])
        self.assertEqual(crate.check(), (0, ""))

    def test_unresolved_glob_is_counted(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        crate.reexport(geo, "other", None, "other", is_glob=True)
        crate.reexport(crate.root, "other", None, "other", is_glob=True)
        status, err = crate.check()
        self.assertEqual(status, 1)
        self.assertIn("fixture::*  |  fixture::geo::*", err)

    def test_two_external_reexports_are_distinct(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        crate.reexport(geo, "Alpha", None, "other::Alpha")
        crate.reexport(geo, "Beta", None, "other::Beta")
        self.assertEqual(crate.check(), (0, ""))

    def test_one_external_reexport_at_two_paths_fails(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        crate.reexport(geo, "Alpha", None, "other::Alpha")
        crate.reexport(crate.root, "Alpha", None, "other::Alpha")
        status, err = crate.check()
        self.assertEqual(status, 1)
        self.assertIn("fixture::Alpha  |  fixture::geo::Alpha", err)

    def test_item_floor_fails_a_short_walk(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        crate.struct("Point", geo)
        status, err = crate.check(min_items=2)
        self.assertEqual(status, 1)
        self.assertIn("fewer than the floor of 2", err)

    def test_enum_variants_do_not_count_toward_the_floor(self):
        crate = Crate()
        geo = crate.module("geo", crate.root)
        crate.enum("Kind", geo, ["Near", "Far"])
        status, err = crate.check(min_items=2)
        self.assertEqual(status, 1)
        self.assertIn("found 1 public items", err)


if __name__ == "__main__":
    unittest.main()
