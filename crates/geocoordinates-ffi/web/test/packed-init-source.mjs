// Exercises the ways `uniffiInitAsync` can obtain the wasm module other than
// Node's default (reading the package's own file, covered by packed-import.mjs):
//
//   bytes   - an explicit `Uint8Array` of the module
//   module  - an explicit, already compiled `WebAssembly.Module`
//   url     - an explicit `http:` URL, which the glue fetches
//   served  - no argument, with the package itself loaded over `http:` as a
//             browser would load it, so the default locates the wasm by URL and
//             fetches it instead of reading it from disk
//
// The wasm-bindgen glue initializes once per process, so each mode runs in its
// own `node packed-init-source.mjs MODE`, started by packed-import.sh from the
// project the tarball is installed into.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { register } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const mode = process.argv[2];
const dist = join(process.cwd(), "node_modules", "geocoordinates-rs", "dist");
const wasmPath = join(dist, "generated", "web", "wasm-bindgen", "index_bg.wasm");

// Serves the installed package's dist/ and counts the wasm requests, so a mode
// that should fetch the module proves it did.
let wasmFetches = 0;
const server = createServer(async (req, res) => {
  const path = new URL(req.url, "http://localhost").pathname;
  if (path.endsWith(".wasm")) wasmFetches += 1;
  try {
    const body = await readFile(join(dist, path));
    const type = path.endsWith(".wasm") ? "application/wasm" : "text/javascript";
    res.writeHead(200, { "content-type": type }).end(body);
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const base = `http://127.0.0.1:${server.address().port}/`;

let pkg;
try {
  if (mode === "served") {
    // A loader that resolves `geocoordinates-rs` to the served dist/ and its
    // relative imports against `http:`, and every other bare import (its
    // runtime dependency) from the installed project as usual.
    const hooks = `
      export async function resolve(specifier, context, next) {
        if (specifier === "geocoordinates-rs") {
          return { url: ${JSON.stringify(base)} + "index.js", shortCircuit: true };
        }
        const parent = context.parentURL ?? "";
        if (parent.startsWith("http:")) {
          if (/^\\.\\.?\\//.test(specifier)) {
            return { url: new URL(specifier, parent).href, shortCircuit: true };
          }
          return next(specifier, { ...context, parentURL: ${JSON.stringify(pathToFileURL(join(process.cwd(), "/")).href)} });
        }
        return next(specifier, context);
      }
      export async function load(url, context, next) {
        if (!url.startsWith("http:")) return next(url, context);
        const res = await fetch(url);
        if (!res.ok) throw new Error(url + ": HTTP " + res.status);
        return { format: "module", source: await res.text(), shortCircuit: true };
      }`;
    register(`data:text/javascript,${encodeURIComponent(hooks)}`);
  }
  pkg = await import("geocoordinates-rs");

  switch (mode) {
    case "bytes":
      await pkg.uniffiInitAsync(await readFile(wasmPath));
      break;
    case "module":
      await pkg.uniffiInitAsync(await WebAssembly.compile(await readFile(wasmPath)));
      break;
    case "url":
      await pkg.uniffiInitAsync(new URL("generated/web/wasm-bindgen/index_bg.wasm", base));
      assert.equal(wasmFetches, 1, "the explicit URL was fetched once");
      break;
    case "served":
      await pkg.uniffiInitAsync();
      assert.equal(wasmFetches, 1, "the default fetched the wasm over http");
      break;
    default:
      throw new Error(`unknown mode ${mode}; expected bytes, module, url, or served`);
  }
} finally {
  server.close();
}

// Reference values from src/test_support.rs, as in packed-import.mjs.
const gcj = pkg.wgs84ToGcj02({ lat: 39.915, lon: 116.404 });
assert.ok(Math.abs(gcj.lat - 39.91640428150164) <= 1e-9, `GCJ-02 lat ${gcj.lat}`);
assert.ok(Math.abs(gcj.lon - 116.41024449916938) <= 1e-9, `GCJ-02 lon ${gcj.lon}`);
console.log(`uniffiInitAsync loads the wasm from ${mode}`);
