#!/usr/bin/env bash
# Build the WebAssembly bindings for `geocoordinates`.
#
# `ubrn` (uniffi-bindgen-react-native) is pre-1.0 and has two quirks the steps
# below work around:
#
#   1. `ubrn build web` regenerates the wasm wrapper crate on every run, so we
#      cannot generate, patch, and build in a single `ubrn` invocation. Instead
#      we generate first (--no-cargo --no-wasm-pack), patch, then build with
#      wasm-pack directly.
#   2. The generated wrapper's `[profile.release] opt-level = "3"` is a *string*,
#      which modern cargo rejects (numeric opt-levels must be unquoted). We
#      coerce it to the integer `3`.
#
# Prerequisites: a Rust toolchain with the `wasm32-unknown-unknown` target, and
# `wasm-pack` on PATH (it manages its own matching wasm-bindgen + wasm-opt).
set -euo pipefail
cd "$(dirname "$0")"

# 1. Generate the TypeScript bindings + the standalone wasm wrapper crate.
npm run --silent generate

# 2. Coerce the string opt-level the ubrn template emits to an integer.
perl -i -pe 's/^opt-level = "3"/opt-level = 3/' rust_modules/wasm/Cargo.toml

# 3. Compile the wrapper to wasm32 and run wasm-bindgen/wasm-opt. The generated
#    src/index.web.ts imports ./generated/web/wasm-bindgen/{index.js,index_bg.wasm}.
wasm-pack build rust_modules/wasm --release --target web \
  --out-dir "$PWD/src/generated/web/wasm-bindgen" --out-name index

# wasm-pack drops a `.gitignore` (`*`) and its own `package.json` in the out-dir.
# The `.gitignore` makes `npm pack` exclude the wasm artifacts, and the nested
# package.json confuses module resolution — remove both.
rm -f src/generated/web/wasm-bindgen/.gitignore \
      src/generated/web/wasm-bindgen/package.json

# 4. Drop ubrn's generated entry: it imports the `.wasm` as a module, which only a
#    bundler resolves. The hand-written src/index.ts is the package entry instead.
rm -f src/index.web.ts

# 5. Type-check and compile src/ to dist/ (ES modules + `.d.ts`). tsc does not
#    copy the wasm-bindgen glue or the wasm itself, so copy them alongside.
rm -rf dist
npm run --silent compile
mkdir -p dist/generated/web/wasm-bindgen
cp src/generated/web/wasm-bindgen/index.js \
   src/generated/web/wasm-bindgen/index.d.ts \
   src/generated/web/wasm-bindgen/index_bg.wasm \
   src/generated/web/wasm-bindgen/index_bg.wasm.d.ts \
   dist/generated/web/wasm-bindgen/

# 6. The generated TypeScript imports its siblings without a file extension,
#    which bundler resolution accepts and Node's ES module loader rejects. Give
#    every extensionless relative specifier in dist/ its `.js`, so the package
#    loads in plain Node and its types resolve under `moduleResolution: node16`.
find dist -name '*.js' -o -name '*.d.ts' | xargs perl -i -pe \
  's{(\bfrom\s+|\bimport\s*\(\s*)(["'"'"'])(\.\.?/[^"'"'"']*?)(?<!\.js)(?<!\.wasm)\2}{$1$2$3.js$2}g'

echo "✓ WebAssembly bindings built at src/generated/web/ and compiled to dist/"
