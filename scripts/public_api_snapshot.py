#!/usr/bin/env python3
"""Write, or check, the committed snapshots of the crate's public API.

Usage: `public_api_snapshot.py <toolchain> [--check]`, run through
`mise run public-api-snapshot` (write) and `mise run public-api-check`
(check). Needs `cargo-public-api` on `PATH` and the nightly `<toolchain>`
whose rustdoc JSON that release reads.

Two snapshots record the surface, signatures, trait impls (auto traits and
derives included) and the prelude's entries: `public-api/geocoordinates/` with
every feature on, and `public-api/geocoordinates-default/` with none (the
default feature set is empty), so a change to what a feature gates shows up as
well. Blanket impls (`impl<T> Any for T`, and those dependencies add, such as
zerocopy's) are omitted: they say nothing about this crate and would churn on
a dependency bump.

Writing replaces both directories. Checking generates both into
`target/public-api-check/` and exits 1, printing a unified diff, when either
differs from the committed one, a file added or removed included.

Each snapshot is split into one file per top-level public module (`crate.txt`
for the crate root's own items), so every file stays under the repository's
file-length limit, which the listing in one file exceeds many times over.
Lines keep the order the tool printed them in. The listing is a run of items,
each an item line followed by the lines that belong to it: its variants,
fields and methods, and its impls with their members. An item line names a
path whose parent is a module or the crate (`pub struct
geocoordinates::china::Bd09`); a line whose path's parent is a type, and every
`impl` line, belongs to the item above it. Each item goes, with its lines, to
the file of the top-level module its path is in, so `impl From<Bd09> for
Coordinate`, which the tool lists under `Bd09`, stays in `china.txt`.

The prelude is the one exception. `cargo public-api` lists each prelude entry
with every impl of its target, printed under the target's canonical path, so
the prelude repeats lines the target's own module already holds. Only the
prelude's lines that name a `geocoordinates::prelude` path are kept: those are
the entries, which is what the prelude adds to the surface.
"""

import difflib
import re
import shutil
import subprocess
import sys
from pathlib import Path

CRATE = "geocoordinates"
SNAPSHOTS = Path("public-api")
SCRATCH = Path("target/public-api-check")
# Snapshot directory name, and the feature flags it is generated with.
VARIANTS = {f"{CRATE}": ["--all-features"], f"{CRATE}-default": []}

SEGMENT = r"[A-Za-z_][A-Za-z0-9_]*"
# A `pub` line, after any leading attributes, and the crate path it names; a
# segment may carry generics (`geocoordinates::Approx<T>::value`).
PUB_LINE = re.compile(
    rf"^(?:#\[[^\]]*\] )*pub (?:[a-z]+ )*({CRATE}(?:::{SEGMENT}(?:<[^<>]*>)?)*)"
)
MODULE = re.compile(rf"^pub mod {CRATE}::({SEGMENT})$")
PRELUDE = re.compile(rf"\b{CRATE}::prelude\b")


def main(argv):
    if len(argv) not in (2, 3) or (len(argv) == 3 and argv[2] != "--check"):
        print(f"usage: {argv[0]} <toolchain> [--check]", file=sys.stderr)
        return 2
    toolchain, check = argv[1], len(argv) == 3
    root = SCRATCH if check else SNAPSHOTS
    for name, flags in VARIANTS.items():
        listing = subprocess.run(
            ["cargo", f"+{toolchain}", "public-api", "-p", CRATE, *flags,
             "--omit", "blanket-impls", "--color", "never"],
            check=True, stdout=subprocess.PIPE, text=True,
        ).stdout
        sections = split(listing.splitlines())
        if sections is None:
            return 1
        write(root / name, sections)
    if not check:
        return 0
    if compare(SNAPSHOTS, SCRATCH):
        print("public-api-check: the public API matches both snapshots in public-api/.")
        return 0
    print(
        "public-api-check: the public API differs from public-api/ (diff above); "
        "if the change is intended, run 'mise run public-api-snapshot' and commit the result",
        file=sys.stderr,
    )
    return 1


def split(lines):
    """Group listing lines by the snapshot file they go to, or None on error."""
    if not lines:
        print("public-api-snapshot: cargo public-api listed nothing", file=sys.stderr)
        return None
    modules = {m.group(1) for line in lines if (m := MODULE.match(line))}
    sections = {}
    section = None
    for line in lines:
        item = item_path(line)
        if item is not None:
            section = item[1] if len(item) > 1 and item[1] in modules else "crate"
        if section is None:
            print(f"public-api-snapshot: no item above: {line}", file=sys.stderr)
            return None
        if section == "prelude" and not PRELUDE.search(line):
            continue
        sections.setdefault(section, []).append(line)
    return sections


def item_path(line):
    """The path an item line names, or None for a line belonging to an item."""
    found = PUB_LINE.match(line)
    if found is None:
        return None
    parts = re.sub(r"<[^<>]*>", "", found.group(1)).split("::")
    if len(parts) > 1 and parts[-2][0].isupper():
        return None  # a variant, field, or member of a type or trait
    return parts


def write(out, sections):
    """Replace the directory `out` with one file per section."""
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    for name, body in sections.items():
        (out / f"{name}.txt").write_text("\n".join(body) + "\n", encoding="utf-8")


def compare(committed, fresh):
    """Print a unified diff of every snapshot file that differs; True if none."""
    same = True
    for name in VARIANTS:
        old_dir, new_dir = committed / name, fresh / name
        files = {p.name for d in (old_dir, new_dir) if d.is_dir() for p in d.glob("*.txt")}
        for file in sorted(files):
            old, new = old_dir / file, new_dir / file
            old_lines = old.read_text(encoding="utf-8").splitlines(True) if old.exists() else []
            new_lines = new.read_text(encoding="utf-8").splitlines(True) if new.exists() else []
            if old_lines != new_lines:
                same = False
                sys.stdout.writelines(difflib.unified_diff(
                    old_lines, new_lines, str(old), f"{new} (generated)"))
    return same


if __name__ == "__main__":
    sys.exit(main(sys.argv))
