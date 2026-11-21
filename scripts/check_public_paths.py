#!/usr/bin/env python3
"""Fail when a public item of the crate is reachable by more than one path.

Reads the rustdoc JSON of the crate (`cargo +nightly rustdoc --lib -- -Z
unstable-options --output-format json`), walks every public module from the
crate root, follows each public `use` re-export to the item it names, and
records every path an item is reachable at, including each enum variant's
(so a glob re-export of an enum's variants is a second path to them). The
crate root's `prelude` module is skipped: it exists to re-export, so its
entries are the one sanctioned second path; a `prelude` nested deeper is not.
A re-export the index cannot resolve (another crate's item) is keyed by its
source path, so two such re-exports are two items unless they name the same
source, and a glob of one is recorded as a `*` path rather than skipped.

Exit status is 0 when every item has exactly one non-prelude path and the
walk finds at least `MIN_ITEMS` items (enum variants not counted), and 1 otherwise, listing each item
with all of its paths or the count against the floor. Run through
`mise run public-api-check`.
"""

import json
import sys
from collections import defaultdict

# Modules of the crate root whose re-exports are a deliberate second path.
SANCTIONED = {"prelude"}

# Floor on the number of public items the walk must find (all features on).
# An empty or truncated walk would otherwise pass. A change that removes
# public items lowers this in the same diff; one that adds items may raise it.
MIN_ITEMS = 140


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
    variants = set()
    seen_modules = set()
    sanctioned = {f"{crate}::{leaf}" for leaf in SANCTIONED}

    def record(key, target, name):
        paths[key].add(name)
        if target is not None and "enum" in target["inner"]:
            for variant_id in target["inner"]["enum"]["variants"]:
                record_variant(variant_id, f"{name}::{item(variant_id)['name']}")

    def record_variant(variant_id, name):
        variants.add(variant_id)
        paths[variant_id].add(name)

    def external_key(reexport):
        # An unresolved target has no item to key on; its source path names it.
        if reexport["id"] is None:
            return ("external", reexport["source"])
        return reexport["id"]

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
                    if target is None:
                        paths[external_key(reexport)].add(f"{path}::*")
                    elif "module" in target["inner"]:
                        walk(target, path)
                    elif "enum" in target["inner"]:
                        for variant_id in target["inner"]["enum"]["variants"]:
                            record_variant(
                                variant_id, f"{path}::{item(variant_id)['name']}"
                            )
                    continue
                name = f"{path}::{reexport['name']}"
                if target is not None and "module" in target["inner"]:
                    walk_module(target, name)
                else:
                    record(external_key(reexport), target, name)
                continue
            name = f"{path}::{child['name']}"
            if "module" in inner:
                walk_module(child, name)
            else:
                record(child_id, child, name)

    def walk_module(module, name):
        if name in sanctioned:
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
    items = len(paths) - len(variants)
    if items < MIN_ITEMS:
        print(
            f"public-api-check: found {items} public items, fewer than the "
            f"floor of {MIN_ITEMS}; if items were removed on purpose, lower "
            "MIN_ITEMS in scripts/check_public_paths.py in the same change",
            file=sys.stderr,
        )
        return 1
    print(
        f"public-api-check: {items} public items and {len(variants)} enum "
        "variants, each at exactly one path."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
