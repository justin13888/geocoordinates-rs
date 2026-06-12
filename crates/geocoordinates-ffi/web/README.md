# geocoordinates (WebAssembly / TypeScript)

WebAssembly bindings for the [`geocoordinates`](https://crates.io/crates/geocoordinates)
geospatial library — China datums (GCJ-02 / BD-09), geodetic transforms — generated from
the `geocoordinates-ffi` UniFFI surface with
[`uniffi-bindgen-react-native`](https://github.com/jhugman/uniffi-bindgen-react-native)
(`ubrn`) targeting `wasm32-unknown-unknown`.

This package is the **hand-authored source** of the npm package. The actual bindings
(`src/generated/`, `src/index.web.ts`) and the wasm wrapper crate (`rust_modules/`) are
**generated** and git-ignored; rebuild them with `npm run build`.

## Build

Prerequisites: a Rust toolchain with the `wasm32-unknown-unknown` target, `wasm-pack`
on `PATH`, and Node.js.

```bash
npm ci
npm run build      # generate TS + wasm, then type-check (see build.sh)
```

`build.sh` documents the two `ubrn` pre-1.0 quirks it works around (the wrapper crate is
regenerated each run, and its `opt-level` is emitted as a string).

## Usage

The wasm module loads asynchronously, so initialize once before calling the API:

```ts
import GeoCoordinates, { uniffiInitAsync, wgs84ToGcj02, coordinateWgs84 } from "geocoordinates-rs";

await uniffiInitAsync();

const gcj = wgs84ToGcj02({ lat: 39.9, lon: 116.4 });
```

Bundler notes:

- **Webpack**: enable `experiments.asyncWebAssembly = true`.
- The package targets ES2021+ (the runtime uses `FinalizationRegistry`).

> Node.js consumers are not the target of this WASM build (the generated loader is
> web/bundler-shaped). A dedicated Node target is tracked separately.
