# geocoordinates (WebAssembly / TypeScript)

WebAssembly bindings for the [`geocoordinates`](https://crates.io/crates/geocoordinates)
crate — low-level geospatial coordinate primitives: China datums (GCJ-02/BD-09), angle
encodings, coordinate parsing/formatting, and geodesy utilities — generated from
the `geocoordinates-ffi` UniFFI surface with
[`uniffi-bindgen-react-native`](https://github.com/jhugman/uniffi-bindgen-react-native)
(`ubrn`) targeting `wasm32-unknown-unknown`.

This package is the **hand-authored source** of the npm package: the entry point
`src/index.ts`, the build script, and the tests. The bindings (`src/generated/`), the wasm
wrapper crate (`rust_modules/`), and the compiled package (`dist/`) are **generated** and
git-ignored; rebuild them with `npm run build`. The published package is `dist/` alone:
ES modules with `.d.ts` types, behind an `exports` map.

Its version is the workspace version in the repository's root `Cargo.toml`; run
`mise run version-sync` after changing that, and `mise run version-check` confirms it.

## Build

Prerequisites: a Rust toolchain with the `wasm32-unknown-unknown` target, `wasm-pack`
on `PATH`, and Node.js.

```bash
npm ci
npm run build        # generate TS + wasm, type-check, and compile to dist/ (see build.sh)
npm test             # run test/*.test.ts in Node against the built wasm
npm run test:packed  # npm pack, install the tarball in an empty project, import it in plain Node
```

`build.sh` documents the two `ubrn` pre-1.0 quirks it works around (the wrapper crate is
regenerated each run, and its `opt-level` is emitted as a string).

## Usage

The wasm module loads asynchronously, so initialize once before calling the API:

```ts
import { uniffiInitAsync, wgs84ToGcj02 } from "geocoordinates-rs";

await uniffiInitAsync();

const gcj = wgs84ToGcj02({ lat: 39.9, lon: 116.4 });
```

`uniffiInitAsync()` finds the package's own `index_bg.wasm` through
`new URL(..., import.meta.url)`: under Node it reads the file from disk, and in a
browser it fetches it. Bundlers that understand that pattern (webpack 5, Vite, esbuild,
Rollup) emit the wasm as an asset with no wasm plugin or configuration. To load the
module from elsewhere, pass its bytes, a compiled `WebAssembly.Module`, or a URL:
`await uniffiInitAsync(new URL("/static/geocoordinates.wasm", location.href))`.

The package is ES modules only (no CommonJS build) and targets ES2021+ (the runtime uses
`FinalizationRegistry`).
