#!/usr/bin/env python3
"""Fail when a public item of the crate is reachable by more than one path.

Reads the rustdoc JSON of the crate (`cargo +nightly rustdoc --lib -- -Z
unstable-options --output-format json`), walks every public module from the
crate root, follows each public `use` re-export to the item it names, and
records every path an item is reachable at. The `prelude` module is skipped:
it exists to re-export, so its entries are the one sanctioned second path.

Exit status is 0 when every item has exactly one non-prelude path, and 1
otherwise, listing each item with all of its paths. Run through
`mise run public-api-check`.
"""

import json
import sys
from collections import defaultdict

# Modules whose re-exports are a deliberate second path to an item.
SANCTIONED = {"prelude"}


def main(argv):
    if len(argv) != 2:
        print(f"usage: {argv[0]} <crate rustdoc json>", file=sys.stderr)
        return 2
    with open(argv[1], encoding="utf-8") as handle:
        doc = json.load(handle)
    index = doc["index"]
    root = index[str(doc["root"])]
    crate = root["name"]

    def item(item_id):
        return index.get(str(item_id))

    paths = defaultdict(set)
    seen_modules = set()

    def walk(module, path):
        key = (id(module), path)
        if key in seen_modules:
            return
        seen_modules.add(key)
        for child_id in module["inner"]["module"]["items"]:
            child = item(child_id)
            if child is None or child.get("visibility") != "public":
                continue
            inner = child["inner"]
            if "use" in inner:
                reexport = inner["use"]
                target = item(reexport["id"]) if reexport["id"] is not None else None
                if reexport["is_glob"]:
                    if target is not None and "module" in target["inner"]:
                        walk(target, path)
                    continue
                name = f"{path}::{reexport['name']}"
                if target is not None and "module" in target["inner"]:
                    walk_module(target, name, reexport["name"])
                else:
                    paths[reexport["id"]].add(name)
                continue
            name = f"{path}::{child['name']}"
            if "module" in inner:
                walk_module(child, name, child["name"])
            else:
                paths[child_id].add(name)

    def walk_module(module, name, leaf):
        if leaf in SANCTIONED:
            return
        walk(module, name)

    walk(root, crate)

    duplicates = {
        item_id: sorted(found) for item_id, found in paths.items() if len(found) > 1
    }
    if duplicates:
        print(
            "public-api-check: these items are reachable by more than one "
            "non-prelude path; keep one canonical path for each:",
            file=sys.stderr,
        )
        for found in sorted(duplicates.values()):
            print("  " + "  |  ".join(found), file=sys.stderr)
        return 1
    print(f"public-api-check: {len(paths)} public items, each at exactly one path.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
