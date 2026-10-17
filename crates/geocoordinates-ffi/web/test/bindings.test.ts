// Behavioural tests for the TypeScript/WebAssembly bindings, run under Node.
//
// Loads the wasm-bindgen output built by `npm run build` and exercises the
// generated API: exact and approximate conversions (with their stated error
// bounds) and each typed `GeoError` variant a caller can match on. The cases
// mirror `crates/geocoordinates-ffi/tests/smoke.py`, which pins the reference
// values against `src/test_support.rs`.
//
// `src/index.web.ts` imports the `.wasm` file as a module, which only a
// bundler resolves, so this test initialises the wasm from its bytes instead.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { before, describe, it } from "node:test";

import initAsync from "../src/generated/web/wasm-bindgen/index.js";
import geocoordinatesFfi, {
  AccuracyKind,
  Crs,
  GeoError,
  MgrsPrecision,
  ParseErrorKind,
  UtmHemisphere,
  accuracyNew,
  baiduMercatorFromBd09,
  baiduMercatorFromCoordinate,
  bd09ToWgs84Fast,
  confidenceNew,
  convert,
  coordinateNew,
  datumTransformCatalogToWgs84,
  fixFromCoord,
  formatFix,
  formatOptionsDefault,
  fromNmeaSentence,
  gcj02IsOutsideChina,
  gcj02ToBd09,
  gcj02ToWgs84Fast,
  gcj02ToWgs84Refined,
  geodesicDistanceM,
  h3Decode,
  h3Encode,
  mgrsFromCoordinate,
  mgrsToCoordinate,
  parseCoordinate,
  utmCentralMeridianDeg,
  utmFromCoordinate,
  utmToCoordinate,
  wgs84IsOutsideChina,
  wgs84ToBd09,
  wgs84ToGcj02,
} from "../src/generated/web/geocoordinates_ffi";

// Reference triple from src/test_support.rs (coordtransform-rs), lat first.
const WGS = { lat: 39.915, lon: 116.404 };
const GCJ = { lat: 39.91640428150164, lon: 116.41024449916938 };
const BD = { lat: 39.922699552216216, lon: 116.41662724378733 };

function near(actual: number, expected: number, tol: number, what: string): void {
  assert.ok(
    Math.abs(actual - expected) <= tol,
    `${what}: ${actual} is not within ${tol} of ${expected}`,
  );
}

// Runs `call`, asserts it throws, and returns the thrown value for the caller
// to narrow with a variant's `instanceOf` guard.
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

describe("China datum conversions", () => {
  it("converts WGS-84 forward exactly", () => {
    const gcj = wgs84ToGcj02(WGS);
    near(gcj.lat, GCJ.lat, 1e-9, "GCJ-02 lat");
    near(gcj.lon, GCJ.lon, 1e-9, "GCJ-02 lon");
    const bd = wgs84ToBd09(WGS);
    near(bd.lat, BD.lat, 1e-9, "BD-09 lat");
    near(bd.lon, BD.lon, 1e-9, "BD-09 lon");
  });

  it("reports the refined inverse as approximate with its bound", () => {
    const back = gcj02ToWgs84Refined(GCJ);
    near(back.lat, WGS.lat, 1e-5, "refined lat");
    near(back.lon, WGS.lon, 1e-5, "refined lon");
    assert.ok(back.maxErrorM > 0 && back.maxErrorM <= 0.5, `bound ${back.maxErrorM}`);
  });

  it("measures the fast inverses' bounds", () => {
    const fast = bd09ToWgs84Fast(BD);
    near(fast.lat, WGS.lat, 1e-4, "fast lat");
    near(fast.lon, WGS.lon, 1e-4, "fast lon");
    assert.equal(fast.maxErrorM, 6.5);
    const edge = gcj02ToWgs84Fast({ lat: 40.0, lon: 72.006 });
    assert.ok(edge.maxErrorM > 300, `bound ${edge.maxErrorM}`);
  });

  it("gates on the China bounding area", () => {
    assert.equal(wgs84IsOutsideChina({ lat: 51.5074, lon: -0.1278 }), true);
    assert.equal(wgs84IsOutsideChina(WGS), false);
    assert.equal(gcj02IsOutsideChina(GCJ), false);
  });
});

describe("runtime conversion dispatch", () => {
  it("marks exact legs with a zero bound and approximate legs with theirs", () => {
    const fwd = convert(coordinateNew(WGS.lat, WGS.lon, Crs.Wgs84), Crs.Gcj02);
    assert.equal(fwd.maxErrorM, 0);
    assert.equal(fwd.coord.crs, Crs.Gcj02);
    near(fwd.coord.lat, GCJ.lat, 1e-9, "forward lat");

    const inv = convert(coordinateNew(GCJ.lat, GCJ.lon, Crs.Gcj02), Crs.Wgs84);
    assert.equal(inv.maxErrorM, 0.5);
    assert.equal(inv.coord.crs, Crs.Wgs84);
    near(inv.coord.lat, WGS.lat, 1e-9, "inverse lat");
  });

  it("reports a Helmert leg's published accuracy", () => {
    const nad27 = coordinateNew(40.0, -100.0, Crs.Nad27);
    near(convert(nad27, Crs.Wgs84).maxErrorM, Math.sqrt(86), 1e-12, "NAD27 bound");
    assert.equal(datumTransformCatalogToWgs84(Crs.Pulkovo42)?.accuracyM, undefined);
  });
});

describe("grids and geodesics", () => {
  it("round-trips MGRS with the half-diagonal bound", () => {
    const code = mgrsFromCoordinate(coordinateNew(40.0, -75.0, Crs.Wgs84), MgrsPrecision.M1);
    assert.equal(code, "18TWK0000027757");
    const decoded = mgrsToCoordinate(code);
    near(decoded.maxErrorM, Math.sqrt(0.5) / 0.9996, 1e-12, "MGRS bound");
    near(decoded.coord.lat, 40.0, 1e-3, "MGRS lat");
  });

  it("projects to UTM", () => {
    const utm = utmFromCoordinate(coordinateNew(40.0, -75.0, Crs.Wgs84));
    assert.equal(utm.zone, 18);
    near(utm.easting, 500_000.0, 1e-3, "easting");
    near(utm.northing, 4_427_757.2188, 1e-3, "northing");
  });

  it("measures Karney geodesics on WGS-84", () => {
    const d = geodesicDistanceM(
      coordinateNew(40.7128, -74.006, Crs.Wgs84),
      coordinateNew(51.5074, -0.1278, Crs.Wgs84),
    );
    near(d, 5_585_233.578_931, 1e-3, "NY-London");
  });

  it("carries an RMC fix's UTC timestamp as a Date", () => {
    const rmc = fromNmeaSentence(
      "$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10",
    );
    assert.equal(rmc.timestamp?.toISOString(), "1994-03-23T12:35:19.000Z");
  });

  it("tags each accuracy with its kind and types the geoid separation", () => {
    const gga = fromNmeaSentence(
      "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47",
    );
    assert.equal(gga.accuracy?.kind, AccuracyKind.OneSigma);
    assert.equal(gga.source?.geoidSeparationM, 46.9);
    assert.equal(parseCoordinate("18TWK0000027757").accuracy?.kind, AccuracyKind.Bound);
    const cep = accuracyNew(AccuracyKind.Cep50, 3.0, undefined);
    assert.equal(cep.kind, AccuracyKind.Cep50);
    assert.equal(cep.horizontalM, 3.0);
  });
});

describe("typed GeoError variants", () => {
  it("OutOfRange carries the rejected position", () => {
    const e = thrown(() => coordinateNew(91.0, 0.0, Crs.Wgs84));
    assert.ok(GeoError.OutOfRange.instanceOf(e), String(e));
    assert.deepEqual({ ...e.inner }, { lat: 91.0, lon: 0.0 });
  });

  it("CrsMismatch carries the expected and found systems as Crs", () => {
    const e = thrown(() => baiduMercatorFromCoordinate(coordinateNew(WGS.lat, WGS.lon, Crs.Wgs84)));
    assert.ok(GeoError.CrsMismatch.instanceOf(e), String(e));
    assert.equal(e.inner.expected, Crs.Bd09);
    assert.equal(e.inner.found, Crs.Wgs84);
  });

  it("OutsideDomain names the operation", () => {
    const pole = thrown(() => gcj02ToBd09({ lat: 90.0, lon: 0.0 }));
    assert.ok(GeoError.OutsideDomain.instanceOf(pole), String(pole));
    assert.equal(pole.inner.operation, "BD-09");
    const polar = thrown(() => baiduMercatorFromBd09({ lat: 80.0, lon: 0.0 }));
    assert.ok(GeoError.OutsideDomain.instanceOf(polar), String(polar));
    assert.equal(polar.inner.operation, "Baidu Mercator");
  });

  it("InvalidValue names the field", () => {
    const e = thrown(() => utmCentralMeridianDeg(0));
    assert.ok(GeoError.InvalidValue.instanceOf(e), String(e));
    assert.equal(e.inner.field, "zone");
    // Only the forward image decodes: zone 1 west of the antimeridian is
    // zone 60's longitude.
    const west = thrown(() =>
      utmToCoordinate({ zone: 1, hemisphere: UtmHemisphere.North, easting: 150_000.0, northing: 0.0 }),
    );
    assert.ok(GeoError.InvalidValue.instanceOf(west), String(west));
    assert.equal(west.inner.field, "easting");
  });

  it("InvalidValue rejects an invalid Accuracy or Confidence, also inside a Fix", () => {
    const negative = thrown(() => accuracyNew(AccuracyKind.Bound, -1.0, undefined));
    assert.ok(GeoError.InvalidValue.instanceOf(negative), String(negative));
    assert.equal(negative.inner.field, "horizontal");
    const unsure = thrown(() => confidenceNew(1.5));
    assert.ok(GeoError.InvalidValue.instanceOf(unsure), String(unsure));
    assert.equal(unsure.inner.field, "confidence");
    const handBuilt = {
      ...fixFromCoord(coordinateNew(40.7128, -74.006, Crs.Wgs84)),
      accuracy: { kind: AccuracyKind.OneSigma, horizontalM: -1.0, verticalM: undefined },
    };
    const crossing = thrown(() => formatFix(handBuilt, formatOptionsDefault()));
    assert.ok(GeoError.InvalidValue.instanceOf(crossing), String(crossing));
  });

  it("InvalidCellId names the grid and value", () => {
    const e = thrown(() => h3Decode({ value: 0n }));
    assert.ok(GeoError.InvalidCellId.instanceOf(e), String(e));
    assert.equal(e.inner.grid, "H3");
    assert.equal(e.inner.value, 0n);
    // A valid cell still decodes, with its radius as the bound.
    const cell = h3Decode(h3Encode(coordinateNew(40.7128, -74.006, Crs.Wgs84), 9));
    near(cell.maxErrorM, 205.872_541, 1e-5, "H3 bound");
  });

  it("InvalidGridRef carries the rejected reference", () => {
    const e = thrown(() => mgrsToCoordinate("not-an-mgrs"));
    assert.ok(GeoError.InvalidGridRef.instanceOf(e), String(e));
    assert.equal(e.inner.detail, "not-an-mgrs");
    // Well-formed, but wholly equatorward of the 80°S cap.
    const offCap = thrown(() => mgrsToCoordinate("AJA"));
    assert.ok(GeoError.InvalidGridRef.instanceOf(offCap), String(offCap));
    assert.equal(offCap.inner.detail, "AJA");
  });

  it("Parse rejects ambiguous text and no-fix NMEA", () => {
    const cases: [string, ParseErrorKind][] = [
      ["1234.5, 10", ParseErrorKind.Ambiguous],
      ["ezs42", ParseErrorKind.Syntax],
    ];
    for (const [input, kind] of cases) {
      const e = thrown(() => parseCoordinate(input));
      assert.ok(GeoError.Parse.instanceOf(e), `${input}: ${String(e)}`);
      assert.equal(e.inner.kind, kind, input);
    }
    const voidFix = thrown(() =>
      fromNmeaSentence("$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07"),
    );
    assert.ok(GeoError.Parse.instanceOf(voidFix), String(voidFix));
    assert.equal(voidFix.inner.kind, ParseErrorKind.NoFix);
    assert.match(voidFix.inner.detail, /void/);
    const corrupt = thrown(() => fromNmeaSentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E"));
    assert.ok(GeoError.Parse.instanceOf(corrupt), String(corrupt));
    assert.equal(corrupt.inner.kind, ParseErrorKind.Checksum);
  });
});
