// Entry point of the `geocoordinates-rs` npm package.
//
// It replaces the `src/index.web.ts` that `ubrn` generates, which imports the
// `.wasm` file as a module: only a bundler with a wasm plugin resolves that, so
// plain Node could not import the package. This entry locates the wasm with
// `new URL(..., import.meta.url)` instead, which Node, browsers, webpack 5, and
// Vite understand. esbuild and Rollup need a plugin for it or an explicit wasm
// source passed to `uniffiInitAsync` (see README.md).
export * from "./generated/web/geocoordinates_ffi.js";

import * as geocoordinates_ffi from "./generated/web/geocoordinates_ffi.js";
import type {
  ExclusionZone,
  Fix,
  FormatOptions,
  GeotagOptions,
  GeotagReference,
  TrackLike,
  Trip,
  Visit,
  ZoneAction,
} from "./generated/web/geocoordinates_ffi.js";
import initAsync, { type InitInput } from "./generated/web/wasm-bindgen/index.js";

/**
 * Thrown, before anything crosses into WebAssembly, for a `Date` earlier than
 * 1970-01-01T00:00:00Z.
 *
 * The wasm build cannot represent an instant before the Unix epoch: lifting one
 * panics in Rust, which surfaces as an untyped `RuntimeError: unreachable` trap
 * that skips the call's cleanup inside the module. Every entry point that
 * takes a `Date` (in `Fix.timestamp`, `GeotagReference.photoTime`,
 * `Visit.arrival` / `departure`, and `Track`'s `locate(at)`) checks it here
 * first and throws this instead. The Python, Kotlin and Swift bindings accept
 * pre-epoch instants.
 *
 * Like a `GeoError.InvalidValue`, it names the rejected value's `field` in
 * snake_case (`"timestamp"`, `"photo_time"`, `"arrival"`, `"departure"`, `"at"`).
 *
 * @example
 * ```ts
 * try {
 *   new Track([{ ...fixFromCoord(coord), timestamp: new Date("1969-07-20T20:17:40Z") }]);
 * } catch (e) {
 *   if (e instanceof PreEpochTimestampError) console.log(e.field, e.timestamp);
 * }
 * ```
 */
export class PreEpochTimestampError extends Error {
  /** The snake_case name of the field or parameter that held the `Date`. */
  readonly field: string;
  /** The rejected instant. */
  readonly timestamp: Date;

  constructor(field: string, timestamp: Date) {
    super(
      `${field}: ${timestamp.toISOString()} is before 1970-01-01T00:00:00Z, which the WebAssembly bindings cannot represent`,
    );
    this.name = "PreEpochTimestampError";
    this.field = field;
    this.timestamp = timestamp;
  }
}

// Throws for a `Date` before the epoch; `undefined` (no timestamp) passes.
function checkInstant(field: string, instant: Date | undefined): void {
  if (instant !== undefined && instant.getTime() < 0) {
    throw new PreEpochTimestampError(field, instant);
  }
}

function checkFixes(fixes: Iterable<Fix>): void {
  for (const fix of fixes) {
    checkInstant("timestamp", fix.timestamp);
  }
}

function checkVisits(visits: Iterable<Visit>): void {
  for (const visit of visits) {
    checkInstant("arrival", visit.arrival);
    checkInstant("departure", visit.departure);
  }
}

// Each export below shadows the generated one of the same name (a module's own
// exports take precedence over `export *`), and differs from it only in throwing
// `PreEpochTimestampError` before a pre-epoch `Date` reaches wasm.

/**
 * A GPS track, as the generated `Track`.
 *
 * @throws {PreEpochTimestampError} for a pre-epoch fix timestamp.
 */
export class Track extends geocoordinates_ffi.Track {
  constructor(fixes: Array<Fix>) {
    checkFixes(fixes);
    super(fixes);
  }

  /** @throws {PreEpochTimestampError} when `at` is before the epoch. */
  override locate(at: Date, options: GeotagOptions): Fix | undefined {
    checkInstant("at", at);
    return super.locate(at, options);
  }
}

/** @throws {PreEpochTimestampError} for a pre-epoch `fix.timestamp`. */
export function formatFix(fix: Fix, options: FormatOptions): string {
  checkFixes([fix]);
  return geocoordinates_ffi.formatFix(fix, options);
}

/** @throws {PreEpochTimestampError} for a pre-epoch `fix.timestamp`. */
export function applyExclusionZones(
  fix: Fix,
  zones: Array<ExclusionZone>,
  action: ZoneAction,
): Fix | undefined {
  checkFixes([fix]);
  return geocoordinates_ffi.applyExclusionZones(fix, zones, action);
}

/** @throws {PreEpochTimestampError} for a pre-epoch `photoTime`. */
export function estimateOffsetMs(track: TrackLike, references: Array<GeotagReference>): bigint {
  for (const reference of references) {
    checkInstant("photo_time", reference.photoTime);
  }
  return geocoordinates_ffi.estimateOffsetMs(track, references);
}

/** @throws {PreEpochTimestampError} for a pre-epoch fix timestamp. */
export function segmentVisits(
  fixes: Array<Fix>,
  distanceM: number,
  dwell: number,
): Array<Visit> {
  checkFixes(fixes);
  return geocoordinates_ffi.segmentVisits(fixes, distanceM, dwell);
}

/** @throws {PreEpochTimestampError} for a pre-epoch arrival or departure. */
export function visitValidate(visit: Visit): void {
  checkVisits([visit]);
  geocoordinates_ffi.visitValidate(visit);
}

/** @throws {PreEpochTimestampError} for a pre-epoch arrival or departure. */
export function segmentTrips(visits: Array<Visit>, gap: number): Array<Trip> {
  checkVisits(visits);
  return geocoordinates_ffi.segmentTrips(visits, gap);
}

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

/**
 * The bindings as one namespace object, matching the generated default export,
 * with the same pre-epoch checks as the named exports.
 */
const checked: typeof geocoordinates_ffi = {
  ...geocoordinates_ffi,
  Track,
  applyExclusionZones,
  estimateOffsetMs,
  formatFix,
  segmentTrips,
  segmentVisits,
  visitValidate,
};

export default {
  geocoordinates_ffi: checked,
};
