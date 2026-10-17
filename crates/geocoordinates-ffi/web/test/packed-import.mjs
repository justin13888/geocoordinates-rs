// Imports the installed `geocoordinates-rs` tarball from plain Node, as a
// consumer would, and makes one call of each kind through it. Run by
// packed-import.sh from an empty project the tarball is installed into; the
// behavioural suite proper is bindings.test.ts.
import assert from "node:assert/strict";
import { createRequire } from "node:module";

import bindings, {
  Crs,
  GeoError,
  coordinateNew,
  uniffiInitAsync,
  wgs84ToGcj02,
} from "geocoordinates-rs";

await uniffiInitAsync();
// A second call is harmless: the wasm loads and the bindings initialize once.
await uniffiInitAsync();

// Reference values from src/test_support.rs, as in bindings.test.ts.
const gcj = wgs84ToGcj02({ lat: 39.915, lon: 116.404 });
assert.ok(Math.abs(gcj.lat - 39.91640428150164) <= 1e-9, `GCJ-02 lat ${gcj.lat}`);
assert.ok(Math.abs(gcj.lon - 116.41024449916938) <= 1e-9, `GCJ-02 lon ${gcj.lon}`);

assert.throws(
  () => coordinateNew(91.0, 0.0, Crs.Wgs84),
  (e) => GeoError.OutOfRange.instanceOf(e),
);

assert.equal(typeof bindings.geocoordinates_ffi.wgs84ToGcj02, "function");

// The tarball ships the version the workspace does (see `mise run version-check`).
const { version } = createRequire(import.meta.url)("geocoordinates-rs/package.json");
console.log(`geocoordinates-rs ${version} imports and runs in plain Node ${process.version}`);
