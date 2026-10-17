#!/usr/bin/env bash
# Packs the built package exactly as `npm publish` would, installs the tarball
# into an empty project, and imports it from plain Node: no tsx, no bundler, no
# access to this directory's sources. Fails when the published package would be
# unimportable (an entry pointing at TypeScript, a file missing from `files`, an
# extensionless import Node's ES module loader rejects).
#
# Run after `npm run build`. Needs network access to install `@ubjs/core`.
set -euo pipefail
cd "$(dirname "$0")/.."
here="$PWD"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

tarball="$(npm pack --silent --pack-destination "$work" | tail -n 1)"
echo "packed $tarball"

cd "$work"
npm init -y >/dev/null
npm install --silent --no-audit --no-fund "./$tarball"
cp "$here/test/packed-import.mjs" "$here/test/packed-init-source.mjs" .
node packed-import.mjs
# The other wasm sources uniffiInitAsync accepts, one process each (the glue
# initializes once per process).
for mode in bytes module url served; do
  node packed-init-source.mjs "$mode"
done
