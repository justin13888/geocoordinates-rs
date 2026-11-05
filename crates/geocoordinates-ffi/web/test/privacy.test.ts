// Location privacy through the TypeScript/WebAssembly bindings, run under Node:
// deterministic coarsening and exclusion zones built from TypeScript tagged enums.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { before, describe, it } from "node:test";

import initAsync from "../src/generated/web/wasm-bindgen/index.js";
import geocoordinatesFfi, {
  AccuracyKind,
  type Coordinate,
  Crs,
  ExclusionZone,
  type Fix,
  GeoError,
  SnapGrid,
  ZoneAction,
  applyExclusionZones,
  coarsen,
  coordinateNew,
  exclusionZoneContains,
  exclusionZoneValidate,
  precisionAtLeast,
  precisionValidate,
} from "../src/generated/web/geocoordinates_ffi";

function at(lat: number, lon: number): Coordinate {
  return coordinateNew(lat, lon, Crs.Wgs84);
}

function bare(coord: Coordinate): Fix {
  return { coord, accuracy: undefined, timestamp: undefined, source: undefined };
}

function thrown(call: () => unknown): unknown {
  try {
    call();
  } catch (e) {
    return e;
  }
  assert.fail("expected the call to throw");
}

before(async () => {
  const wasm = await readFile(
    new URL("../src/generated/web/wasm-bindgen/index_bg.wasm", import.meta.url),
  );
  await initAsync({ module_or_path: wasm });
  geocoordinatesFfi.initialize();
});

describe("Location privacy", () => {
  it("coarsens to a named cell with a bound, idempotently", () => {
    const precision = precisionAtLeast(1_000);
    assert.deepEqual(precision, { grid: SnapGrid.Geohash, level: 5 });
    const shared = coarsen(at(57.64911, 10.40744), precision);
    assert.equal(shared.source?.raw, "privacy:geohash:u4pru");
    assert.equal(shared.accuracy?.kind, AccuracyKind.Bound);
    assert.deepEqual(coarsen(shared.coord, precision), shared);
    const bad = thrown(() => precisionValidate({ grid: SnapGrid.S2, level: 31 }));
    assert.ok(GeoError.InvalidValue.instanceOf(bad), String(bad));
    assert.equal(bad.inner.field, "S2 level");
  });

  it("strips or coarsens a fix inside a zone across the antimeridian", () => {
    const zone = ExclusionZone.Circle.new({ center: at(-17, 179.995), radiusM: 2_000 });
    assert.ok(exclusionZoneContains(zone, at(-17, -179.995)));
    const zones = [zone];
    const inside = bare(at(-17, -179.995));
    assert.equal(applyExclusionZones(inside, zones, ZoneAction.Strip.new()), undefined);
    const precision = { grid: SnapGrid.Geohash, level: 4 };
    const coarse = applyExclusionZones(inside, zones, ZoneAction.Coarsen.new({ precision }));
    assert.ok(coarse?.source?.raw.startsWith("privacy:geohash:"), String(coarse?.source?.raw));
    const away = bare(at(0, 0));
    assert.deepEqual(applyExclusionZones(away, zones, ZoneAction.Strip.new()), away);
    const negative = thrown(() =>
      exclusionZoneValidate(ExclusionZone.Circle.new({ center: at(0, 0), radiusM: -1 })),
    );
    assert.ok(GeoError.InvalidValue.instanceOf(negative), String(negative));
    assert.equal(negative.inner.field, "radius");
  });
});
