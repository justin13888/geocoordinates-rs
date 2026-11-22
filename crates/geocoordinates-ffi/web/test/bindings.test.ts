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
  AuxiliaryTagPolicy,
  AxisOrder,
  Crs,
  DatumAmbiguity,
  ExifDms,
  ExifGps,
  ExifRational,
  GeoError,
  MgrsPrecision,
  NullIslandPolicy,
  ParseErrorKind,
  Track,
  UtmHemisphere,
  XmpGps,
  accuracyNew,
  baiduMercatorFromBd09,
  baiduMercatorFromCoordinate,
  bd09ToWgs84Fast,
  confidenceNew,
  convert,
  coordinateFromLonLat,
  coordinateNew,
  datumTransformCatalogToWgs84,
  fixFromCoord,
  formatFix,
  formatOptionsDefault,
  gcj02IsOutsideChina,
  gcj02ToBd09,
  gcj02ToWgs84Fast,
  gcj02ToWgs84Refined,
  geotagOptionsDefault,
  geodesicDistanceM,
  h3Decode,
  h3Encode,
  h3Validate,
  initialBearingDeg,
  mgrsFromCoordinate,
  mgrsToCoordinate,
  parseCoordinate,
  parseExif,
  parseExifWith,
  parseGpx,
  parseNmeaSentence,
  parseText,
  parseWkt,
  parseXmp,
  parseXmpWith,
  rhumbBearingDeg,
  s2Decode,
  s2Encode,
  s2Level,
  s2Validate,
  utmCentralMeridianDeg,
  utmFromCoordinate,
  utmToCoordinate,
  utmValidate,
  wgs84IsOutsideChina,
  wgs84ToBd09,
  wgs84ToGcj02,
} from "../src/generated/web/geocoordinates_ffi";
import * as entry from "../src/index";

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
    near(utm.eastingM, 500_000.0, 1e-3, "easting");
    near(utm.northingM, 4_427_757.2188, 1e-3, "northing");
  });

  it("measures Karney geodesics on WGS-84", () => {
    const d = geodesicDistanceM(
      coordinateNew(40.7128, -74.006, Crs.Wgs84),
      coordinateNew(51.5074, -0.1278, Crs.Wgs84),
    );
    near(d, 5_585_233.578_931, 1e-3, "NY-London");
  });

  it("suffixes every returned angle with its unit", () => {
    const equator = coordinateNew(0.0, 0.0, Crs.Wgs84);
    near(initialBearingDeg(equator, coordinateNew(0.0, 1.0, Crs.Wgs84)), 90.0, 1e-6, "initial");
    const parallel = rhumbBearingDeg(
      coordinateNew(60.0, -10.0, Crs.Wgs84),
      coordinateNew(60.0, 30.0, Crs.Wgs84),
    );
    near(parallel, 90.0, 1e-9, "rhumb");
  });

  it("names a longitude-first constructor and a free-text parser", () => {
    assert.deepEqual(
      coordinateFromLonLat(-74.0, 40.0, Crs.Wgs84),
      coordinateNew(40.0, -74.0, Crs.Wgs84),
    );
    const text = parseText("40.7128, -74.006");
    near(text.coord.lat, 40.7128, 1e-9, "text lat");
    near(text.coord.lon, -74.006, 1e-9, "text lon");
    const grid = thrown(() => parseText("87G7PX7V+4J"));
    assert.ok(GeoError.Parse.instanceOf(grid), String(grid));
    assert.equal(grid.inner.kind, ParseErrorKind.Syntax);
  });

  it("carries an RMC fix's UTC timestamp as a Date", () => {
    const rmc = parseNmeaSentence(
      "$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10",
    );
    assert.equal(rmc.timestamp?.toISOString(), "1994-03-23T12:35:19.000Z");
  });

  it("drops a pre-epoch GPX time with a note instead of trapping", () => {
    // wasm32's SystemTime cannot hold an instant before 1970, so the reader
    // leaves the timestamp empty and says why (#372); a later one is kept.
    const [old, recent] = parseGpx(
      '<gpx version="1.1" creator="x" xmlns="http://www.topografix.com/GPX/1/1">' +
        '<wpt lat="1" lon="2"><time>1969-12-31T23:59:58Z</time></wpt>' +
        '<wpt lat="3" lon="4"><time>1970-01-01T00:00:01Z</time></wpt></gpx>',
    );
    assert.equal(old.timestamp, undefined);
    assert.ok(old.source?.notes.some((note) => note.startsWith("GPX time")));
    assert.equal(recent.timestamp?.toISOString(), "1970-01-01T00:00:01.000Z");
    assert.deepEqual(recent.source?.notes, []);
  });

  it("round-trips sub-second timestamps through a track", () => {
    // About 56 m apart, inside the default 50 m/s speed limit.
    const before = new Date("1970-01-01T00:00:01.250Z");
    const after = new Date("1970-01-01T00:00:03.250Z");
    const timed = (lon: number, timestamp: Date) => ({
      coord: coordinateNew(0.0, lon, Crs.Wgs84),
      accuracy: undefined,
      timestamp,
      source: undefined,
    });
    const track = new Track([timed(0.0005, after), timed(0.0, before)]);
    assert.deepEqual(
      track.fixes().map((fix) => fix.timestamp?.toISOString()),
      [before.toISOString(), after.toISOString()],
    );
    const midway = new Date("1970-01-01T00:00:02.250Z");
    const between = track.locate(midway, geotagOptionsDefault());
    assert.ok(between);
    assert.equal(between.timestamp?.toISOString(), midway.toISOString());
    near(between.coord.lon, 0.00025, 1e-12, "interpolated lon");
  });

  it("tags each accuracy with its kind and types the geoid separation", () => {
    const gga = parseNmeaSentence(
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
      utmToCoordinate({ zone: 1, hemisphere: UtmHemisphere.North, eastingM: 150_000.0, northingM: 0.0 }),
    );
    assert.ok(GeoError.InvalidValue.instanceOf(west), String(west));
    assert.equal(west.inner.field, "easting");
    // `utmValidate` applies the same rules without decoding.
    utmValidate(utmFromCoordinate(coordinateNew(40.0, -75.0, Crs.Wgs84)));
    const westRecord = thrown(() =>
      utmValidate({ zone: 1, hemisphere: UtmHemisphere.North, eastingM: 150_000.0, northingM: 0.0 }),
    );
    assert.ok(GeoError.InvalidValue.instanceOf(westRecord), String(westRecord));
    assert.equal(westRecord.inner.field, "easting");
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
    assert.equal(crossing.inner.field, "horizontal");
  });

  it("InvalidCellId names the grid and value", () => {
    const e = thrown(() => h3Decode({ value: 0n }));
    assert.ok(GeoError.InvalidCellId.instanceOf(e), String(e));
    assert.equal(e.inner.grid, "H3");
    assert.equal(e.inner.value, 0n);
    // A valid cell still decodes, with its radius as the bound.
    const cell = h3Decode(h3Encode(coordinateNew(40.7128, -74.006, Crs.Wgs84), 9));
    near(cell.maxErrorM, 205.872_541, 1e-5, "H3 bound");
    h3Validate(h3Encode(coordinateNew(40.7128, -74.006, Crs.Wgs84), 9));
    for (const [validate, grid] of [
      [() => h3Validate({ value: 0n }), "H3"],
      [() => s2Validate({ value: 0n }), "S2"],
    ] as const) {
      const bad = thrown(validate);
      assert.ok(GeoError.InvalidCellId.instanceOf(bad), String(bad));
      assert.equal(bad.inner.grid, grid);
    }
  });

  it("round-trips an S2 cell id above 2^63", () => {
    const cell = s2Encode(coordinateNew(40.7128, -74.006, Crs.Wgs84), 20);
    assert.equal(cell.value, 9_926_595_630_970_437_632n);
    assert.ok(cell.value >= 1n << 63n);
    s2Validate(cell);
    assert.equal(s2Level(cell), 20);
    const back = s2Decode({ value: 9_926_595_630_970_437_632n });
    near(back.coord.lat, 40.712_762_829, 1e-8, "S2 lat");
    near(back.coord.lon, -74.006_004_053, 1e-8, "S2 lon");
    // The id an error carries crosses back just as wide.
    const max = (1n << 64n) - 1n;
    const bad = thrown(() => s2Decode({ value: max }));
    assert.ok(GeoError.InvalidCellId.instanceOf(bad), String(bad));
    assert.equal(bad.inner.value, max);
  });

  it("InvalidGridRef carries the rejected reference", () => {
    const e = thrown(() => mgrsToCoordinate("not-an-mgrs"));
    assert.ok(GeoError.InvalidGridRef.instanceOf(e), String(e));
    assert.equal(e.inner.grid, "MGRS");
    assert.equal(e.inner.detail, '"not-an-mgrs" is not a well-formed reference');
    // Well-formed, but wholly equatorward of the 80°S cap.
    const offCap = thrown(() => mgrsToCoordinate("AJA"));
    assert.ok(GeoError.InvalidGridRef.instanceOf(offCap), String(offCap));
    assert.equal(offCap.inner.grid, "MGRS");
    assert.equal(offCap.inner.detail, '"AJA" holds no point of its zone');
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
      parseNmeaSentence("$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07"),
    );
    assert.ok(GeoError.Parse.instanceOf(voidFix), String(voidFix));
    assert.equal(voidFix.inner.kind, ParseErrorKind.NoFix);
    assert.match(voidFix.inner.detail, /void/);
    const corrupt = thrown(() => parseNmeaSentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E"));
    assert.ok(GeoError.Parse.instanceOf(corrupt), String(corrupt));
    assert.equal(corrupt.inner.kind, ParseErrorKind.Checksum);
    const unsupported = thrown(() => parseNmeaSentence("$GPVTG,054.7,T"));
    assert.ok(GeoError.Parse.instanceOf(unsupported), String(unsupported));
    assert.equal(unsupported.inner.kind, ParseErrorKind.Unsupported);
    const deep = thrown(() => parseWkt("GEOMETRYCOLLECTION(".repeat(200), AxisOrder.LonLat));
    assert.ok(GeoError.Parse.instanceOf(deep), String(deep));
    assert.equal(deep.inner.kind, ParseErrorKind.TooDeep);
  });
});

describe("EXIF and XMP GPS tags", () => {
  const whole = (numerator: number) => new ExifRational.Unsigned({ numerator, denominator: 1 });
  const gps = () =>
    ExifGps.create({
      latitude: ExifDms.create({ degrees: whole(39), minutes: whole(54), seconds: whole(0) }),
      latitudeRef: "N",
      longitude: ExifDms.create({ degrees: whole(116), minutes: whole(24), seconds: whole(0) }),
      longitudeRef: "E",
      altitude: whole(10),
      altitudeRef: 1,
      speed: whole(36),
      speedRef: "K",
    });

  it("reaches the same capture by either path", () => {
    const capture = parseExif(gps());
    assert.equal(capture.fix.coord.lat, 39.9);
    assert.equal(capture.fix.coord.lon, 116.4);
    assert.equal(capture.fix.coord.height?.inner.meters, -10);
    assert.equal(capture.fix.source?.datumAmbiguity, DatumAmbiguity.PossiblyGcj02);
    assert.equal(capture.speedMps, 10);
    const xmp = parseXmp(XmpGps.create({ latitude: "39,54.0N", longitude: "116,24,0E" }));
    assert.equal(xmp.fix.coord.lat, capture.fix.coord.lat);
    assert.equal(xmp.fix.coord.lon, capture.fix.coord.lon);
  });

  it("rejects a bad Ref and, by option, null island", () => {
    const badRef = thrown(() => parseExif({ ...gps(), latitudeRef: "Q" }));
    assert.ok(GeoError.InvalidValue.instanceOf(badRef), String(badRef));
    assert.equal(badRef.inner.field, "latitude_ref");
    const island = thrown(() =>
      parseXmpWith(XmpGps.create({ latitude: "0,0.0N", longitude: "0,0.0E" }), {
        nullIsland: NullIslandPolicy.Reject,
        invalidAuxiliary: AuxiliaryTagPolicy.Reject,
      }),
    );
    assert.ok(GeoError.Parse.instanceOf(island), String(island));
    assert.equal(island.inner.kind, ParseErrorKind.NoFix);
  });

  it("drops an invalid auxiliary tag by option, keeping the position", () => {
    const unset = {
      ...gps(),
      altitude: new ExifRational.Unsigned({ numerator: 0, denominator: 0 }),
    };
    const altitude = thrown(() =>
      parseExifWith(unset, {
        nullIsland: NullIslandPolicy.Accept,
        invalidAuxiliary: AuxiliaryTagPolicy.Reject,
      }),
    );
    assert.ok(GeoError.InvalidValue.instanceOf(altitude), String(altitude));
    assert.equal(altitude.inner.field, "altitude");
    const kept = parseExifWith(unset, {
      nullIsland: NullIslandPolicy.Accept,
      invalidAuxiliary: AuxiliaryTagPolicy.Drop,
    });
    assert.equal(kept.fix.coord.lat, 39.9);
    assert.equal(kept.fix.coord.height, undefined);
    assert.ok(kept.fix.source?.notes.some((note) => note.startsWith("GPSAltitude")));
  });
});

// The package entry (`src/index.ts`) checks every `Date` before it crosses: a
// pre-epoch one would trap the module with `RuntimeError: unreachable` (#346).
describe("pre-epoch timestamps", () => {
  const PRE = new Date("1969-12-31T23:59:58.750Z");
  const fixAt = (timestamp: Date | undefined, lon = 0.0) => ({
    ...fixFromCoord(coordinateNew(0.0, lon, Crs.Wgs84)),
    timestamp,
  });

  // Asserts `call` throws a `PreEpochTimestampError` naming `field` and `PRE`.
  function rejects(call: () => unknown, field: string): void {
    const e = thrown(call);
    assert.ok(e instanceof entry.PreEpochTimestampError, String(e));
    assert.ok(e instanceof Error);
    assert.equal(e.name, "PreEpochTimestampError");
    assert.equal(e.field, field);
    assert.equal(e.timestamp, PRE);
    assert.match(e.message, /1969-12-31T23:59:58\.750Z/);
  }

  const track = () =>
    new entry.Track([
      fixAt(new Date("1970-01-01T00:00:00.000Z")),
      fixAt(new Date("1970-01-01T00:00:02.000Z"), 0.0005),
    ]);

  it("rejects a pre-epoch Fix timestamp at every entry that takes a Fix", () => {
    rejects(() => new entry.Track([fixAt(undefined), fixAt(PRE)]), "timestamp");
    rejects(() => entry.formatFix(fixAt(PRE), formatOptionsDefault()), "timestamp");
    rejects(
      () => entry.applyExclusionZones(fixAt(PRE), [], entry.ZoneAction.Strip.new()),
      "timestamp",
    );
    rejects(() => entry.segmentVisits([fixAt(PRE)], 50.0, 60_000), "timestamp");
    // The default export's namespace holds the checked forms too.
    const ns = entry.default.geocoordinates_ffi;
    assert.equal(ns.wgs84ToGcj02, wgs84ToGcj02);
    rejects(() => new ns.Track([fixAt(PRE)]), "timestamp");
    rejects(() => ns.formatFix(fixAt(PRE), formatOptionsDefault()), "timestamp");
  });

  it("rejects a pre-epoch photo time and locate instant", () => {
    const t = track();
    rejects(() => t.locate(PRE, geotagOptionsDefault()), "at");
    rejects(
      () => entry.estimateOffsetMs(t, [{ photoTime: PRE, location: coordinateNew(0, 0, Crs.Wgs84) }]),
      "photo_time",
    );
  });

  it("rejects a pre-epoch visit arrival or departure", () => {
    const stay = (lon: number, seconds: number) =>
      fixAt(new Date(Date.UTC(1970, 0, 1, 0, 0, seconds)), lon);
    const [visit] = entry.segmentVisits([stay(0.0, 0), stay(0.0, 120)], 50.0, 60_000);
    assert.ok(visit);
    entry.visitValidate(visit);
    assert.equal(entry.segmentTrips([visit], 60_000).length, 1);
    rejects(() => entry.visitValidate({ ...visit, arrival: PRE }), "arrival");
    rejects(() => entry.segmentTrips([visit, { ...visit, departure: PRE }], 60_000), "departure");
  });

  it("passes the epoch itself and later instants through unchanged", () => {
    const t = track();
    assert.equal(t.fixes().length, 2);
    const at = t.locate(new Date("1970-01-01T00:00:01.000Z"), geotagOptionsDefault());
    near(at?.coord.lon ?? NaN, 0.00025, 1e-12, "interpolated lon");
    assert.equal(
      entry.estimateOffsetMs(t, [
        { photoTime: new Date(0), location: coordinateNew(0, 0, Crs.Wgs84) },
      ]),
      0n,
    );
    assert.equal(
      entry.formatFix(fixAt(new Date(0)), formatOptionsDefault()),
      formatFix(fixAt(new Date(0)), formatOptionsDefault()),
    );
    const kept = entry.applyExclusionZones(fixAt(new Date(0)), [], entry.ZoneAction.Strip.new());
    assert.equal(kept?.timestamp?.toISOString(), new Date(0).toISOString());
  });
});
