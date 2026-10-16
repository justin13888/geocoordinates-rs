// Entry point of the `geocoordinates-rs` npm package.
//
// It replaces the `src/index.web.ts` that `ubrn` generates, which imports the
// `.wasm` file as a module: only a bundler with a wasm plugin resolves that, so
// plain Node could not import the package. This entry locates the wasm with
// `new URL(..., import.meta.url)` instead, which Node, browsers, and the common
// bundlers (webpack 5, Vite, esbuild, Rollup) all understand.
export * from "./generated/web/geocoordinates_ffi.js";

import * as geocoordinates_ffi from "./generated/web/geocoordinates_ffi.js";
import initAsync, { type InitInput } from "./generated/web/wasm-bindgen/index.js";

/**
 * Loads the WebAssembly module and initializes the bindings. Call it once, and
 * await it, before calling anything else in the package.
 *
 * With no argument the module is read from the package itself: from disk under
 * Node, and fetched over HTTP in a browser. Pass the module's bytes, a compiled
 * `WebAssembly.Module`, or a URL or `Response` to load it from elsewhere.
 */
export async function uniffiInitAsync(wasm?: InitInput | Promise<InitInput>): Promise<void> {
  await initAsync({ module_or_path: wasm ?? (await bundledWasm()) });
  // Checks the bindings were generated against this exact library; idempotent.
  geocoordinates_ffi.default.initialize();
}

// The package's own `index_bg.wasm`. `fetch` cannot read `file:` URLs, so under
// Node (where the package sits on disk) the bytes are read with `node:fs`. The
// specifier is a variable, so bundlers targeting a browser never try to resolve it.
async function bundledWasm(): Promise<InitInput> {
  const url = new URL("./generated/web/wasm-bindgen/index_bg.wasm", import.meta.url);
  if (url.protocol !== "file:") {
    return url;
  }
  const fs = "node:fs/promises";
  const { readFile } = (await import(/* @vite-ignore */ /* webpackIgnore: true */ fs)) as {
    readFile(path: URL): Promise<Uint8Array>;
  };
  return readFile(url);
}

/** The bindings as one namespace object, matching the generated default export. */
export default {
  geocoordinates_ffi,
};
