// Reverse geocoding through the TypeScript/WebAssembly bindings, run under Node,
// including a TypeScript `ReverseGeocoder` implementation crossing into Rust.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { before, describe, it } from "node:test";

import initAsync from "../src/generated/web/wasm-bindgen/index.js";
import geocoordinatesFfi, {
  type Coordinate,
  Crs,
  FeatureClass,
  Gazetteer,
  GeoError,
  ParseErrorKind,
  type Place,
  type ReverseGeocoder,
  type ReverseOptions,
  coordinateNew,
  gazetteerAsReverseGeocoder,
  placeNew,
  reverseGeocode,
  reverseOptionsDefault,
} from "../src/generated/web/geocoordinates_ffi";

// The core crate's doctest gazetteer: Paris, Montmartre, Lyon and Savusavu.
const FIXTURE = new URL("../../../../src/reverse/data/fixture.gaz", import.meta.url);
let fixture: ArrayBuffer;

function at(lat: number, lon: number): Coordinate {
  return coordinateNew(lat, lon, Crs.Wgs84);
}

function thrown(call: () => unknown): unknown {
  try {
    call();
  } catch (e) {
    return e;
  }
  assert.fail("expected the call to throw");
}

/** A TypeScript geocoder: every point is in one fixed place. */
class Everywhere implements ReverseGeocoder {
  constructor(private readonly countryCode: string) {}

  reverse(coordinate: Coordinate, _options: ReverseOptions): Place | undefined {
    if (coordinate.lat < -80) {
      return undefined;
    }
    return placeNew("Everywhere", at(0, 0), this.countryCode, coordinate, 0);
  }
}

before(async () => {
  const wasm = await readFile(
    new URL("../src/generated/web/wasm-bindgen/index_bg.wasm", import.meta.url),
  );
  await initAsync({ module_or_path: wasm });
  geocoordinatesFfi.initialize();
  const bytes = await readFile(FIXTURE);
  fixture = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
});

describe("Reverse geocoding", () => {
  it("loads a gazetteer and resolves a place with its localized name", () => {
    const gazetteer = Gazetteer.fromBytes(fixture);
    assert.equal(gazetteer.len(), 4n);
    assert.deepEqual(gazetteer.languages(), ["fr", "ja"]);
    const options = { ...reverseOptionsDefault(), languages: ["ja"] };
    const paris = gazetteer.reverse(at(48.86, 2.35), options);
    assert.ok(paris);
    assert.equal(paris.name, "Paris");
    assert.equal(paris.localizedNames.get("ja"), "パリ");
    assert.equal(paris.featureClass, FeatureClass.Capital);
    assert.equal(gazetteer.reverse(at(0, -30), reverseOptionsDefault()), undefined);
    const truncated = thrown(() => Gazetteer.fromBytes(new TextEncoder().encode("GCGZ").buffer));
    assert.ok(GeoError.Parse.instanceOf(truncated), String(truncated));
    assert.equal(truncated.inner.kind, ParseErrorKind.Syntax);
  });

  it("calls a TypeScript geocoder from Rust and checks its answer", () => {
    const found = reverseGeocode(new Everywhere("XX"), at(0, 1), reverseOptionsDefault());
    assert.ok(found);
    assert.equal(found.name, "Everywhere");
    assert.ok(found.distanceM > 100_000, String(found.distanceM));
    assert.equal(reverseGeocode(new Everywhere("XX"), at(-85, 0), reverseOptionsDefault()), undefined);
    const lying = thrown(() =>
      reverseGeocode(new Everywhere("nowhere"), at(0, 1), reverseOptionsDefault()),
    );
    assert.ok(GeoError.InvalidValue.instanceOf(lying), String(lying));
    assert.equal(lying.inner.field, "country_code");
    const gazetteer = gazetteerAsReverseGeocoder(Gazetteer.fromBytes(fixture));
    assert.equal(reverseGeocode(gazetteer, at(45.75, 4.85), reverseOptionsDefault())?.name, "Lyon");
  });
});
