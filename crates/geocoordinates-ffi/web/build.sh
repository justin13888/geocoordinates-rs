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

# 4. Type-check the generated bindings against @ubjs/core + the wasm typings.
npm run --silent typecheck

echo "✓ WebAssembly bindings built at src/generated/web/"
