// Behavioural tests of the Swift bindings: exact and approximate conversions
// (with their stated error bounds) and each typed `GeoError` case. The cases are
// ported from crates/geocoordinates-ffi/tests/smoke.py, which pins the reference
// values against src/test_support.rs.
import Foundation
import GeoCoordinates
import XCTest

// Reference triple from src/test_support.rs (coordtransform-rs), lat first.
private let wgs = Wgs84(lat: 39.915, lon: 116.404)
private let gcj = Gcj02(lat: 39.91640428150164, lon: 116.41024449916938)
private let bd = Bd09(lat: 39.922699552216216, lon: 116.41662724378733)

private func wgs84(_ lat: Double, _ lon: Double) throws -> Coordinate {
    try coordinateNew(lat: lat, lon: lon, crs: .wgs84)
}

/// Runs `body`, which must throw a `GeoError`, and returns that error for the
/// caller to match against one case and read its typed fields.
private func geoError<T>(
    _ body: () throws -> T, file: StaticString = #filePath, line: UInt = #line
) -> GeoError? {
    do {
        _ = try body()
        XCTFail("expected a GeoError", file: file, line: line)
    } catch let error as GeoError {
        return error
    } catch {
        XCTFail("expected a GeoError, got \(error)", file: file, line: line)
    }
    return nil
}

final class ConversionTests: XCTestCase {
    func testChinaForwardConversionsAreExact() throws {
        let toGcj = try wgs84ToGcj02(p: wgs)
        XCTAssertEqual(toGcj.lat, gcj.lat, accuracy: 1e-9)
        XCTAssertEqual(toGcj.lon, gcj.lon, accuracy: 1e-9)
        let toBd = try wgs84ToBd09(p: wgs)
        XCTAssertEqual(toBd.lat, bd.lat, accuracy: 1e-9)
        XCTAssertEqual(toBd.lon, bd.lon, accuracy: 1e-9)
        XCTAssertTrue(try wgs84IsOutsideChina(p: Wgs84(lat: 51.5074, lon: -0.1278)))
        XCTAssertFalse(try wgs84IsOutsideChina(p: wgs))
        XCTAssertFalse(try gcj02IsOutsideChina(p: gcj))
    }

    func testRefinedInverseCarriesItsBound() throws {
        let back = try gcj02ToWgs84Refined(p: gcj)
        XCTAssertEqual(back.lat, wgs.lat, accuracy: 1e-5)
        XCTAssertEqual(back.lon, wgs.lon, accuracy: 1e-5)
        XCTAssertGreaterThan(back.maxErrorM, 0.0)
        XCTAssertLessThanOrEqual(back.maxErrorM, 0.5)
    }

    func testFastInversesMeasureTheirBounds() throws {
        let fast = try bd09ToWgs84Fast(p: bd)
        XCTAssertEqual(fast.lat, wgs.lat, accuracy: 1e-4)
        XCTAssertEqual(fast.lon, wgs.lon, accuracy: 1e-4)
        XCTAssertEqual(fast.maxErrorM, 6.5)
        let edge = try gcj02ToWgs84Fast(p: Gcj02(lat: 40.0, lon: 72.006))
        XCTAssertGreaterThan(edge.maxErrorM, 300.0)
    }

    func testConvertReportsExactAndApproximateLegs() throws {
        let fwd = try convert(coord: wgs84(wgs.lat, wgs.lon), to: .gcj02)
        XCTAssertEqual(fwd.maxErrorM, 0.0)
        XCTAssertEqual(fwd.coord.crs, .gcj02)
        XCTAssertEqual(fwd.coord.lat, gcj.lat, accuracy: 1e-9)

        let inv = try convert(
            coord: coordinateNew(lat: gcj.lat, lon: gcj.lon, crs: .gcj02), to: .wgs84)
        XCTAssertEqual(inv.maxErrorM, 0.5)
        XCTAssertEqual(inv.coord.crs, .wgs84)
        XCTAssertEqual(inv.coord.lat, wgs.lat, accuracy: 1e-9)

        // A Helmert leg reports its published TR8350.2 accuracy; Pulkovo has none.
        let nad27 = try coordinateNew(lat: 40.0, lon: -100.0, crs: .nad27)
        XCTAssertEqual(try convert(coord: nad27, to: .wgs84).maxErrorM, 86.0.squareRoot(), accuracy: 1e-12)
        XCTAssertNil(datumTransformCatalogToWgs84(datum: .pulkovo42)?.accuracyM)
    }

    func testGridsProjectionsAndGeodesics() throws {
        let code = try mgrsFromCoordinate(coord: wgs84(40.0, -75.0), precision: .m1)
        XCTAssertEqual(code, "18TWK0000027757")
        let decoded = try mgrsToCoordinate(mgrs: code)
        XCTAssertEqual(decoded.maxErrorM, 0.5.squareRoot() / 0.9996, accuracy: 1e-12)
        XCTAssertEqual(decoded.coord.lat, 40.0, accuracy: 1e-3)

        let utm = try utmFromCoordinate(coord: wgs84(40.0, -75.0))
        XCTAssertEqual(utm.zone, 18)
        XCTAssertEqual(utm.hemisphere, .north)
        XCTAssertEqual(utm.northing, 4_427_757.2188, accuracy: 1e-3)

        let cell = try h3Decode(cell: h3Encode(coord: wgs84(40.7128, -74.006), resolution: 9))
        XCTAssertEqual(cell.maxErrorM, 205.872_541, accuracy: 1e-5)

        let d = try geodesicDistanceM(a: wgs84(40.7128, -74.006), b: wgs84(51.5074, -0.1278))
        XCTAssertEqual(d, 5_585_233.578_931, accuracy: 1e-3)
    }

    func testNormalizedNamesCarryTheirUnitAndAxisOrder() throws {
        let east = try initialBearingDeg(a: wgs84(0.0, 0.0), b: wgs84(0.0, 1.0))
        XCTAssertEqual(east, 90.0, accuracy: 1e-6)
        let parallel = try rhumbBearingDeg(a: wgs84(60.0, -10.0), b: wgs84(60.0, 30.0))
        XCTAssertEqual(parallel, 90.0, accuracy: 1e-9)
        let lonFirst = try coordinateFromLonLat(lon: -74.0, lat: 40.0, crs: .wgs84)
        XCTAssertEqual(lonFirst.lat, 40.0)
        XCTAssertEqual(lonFirst.lon, -74.0)
        let text = try parseText(input: "40.7128, -74.006")
        XCTAssertEqual(text.coord.lat, 40.7128, accuracy: 1e-9)
        XCTAssertEqual(text.coord.lon, -74.006, accuracy: 1e-9)
        try utmValidate(utm: utmFromCoordinate(coord: wgs84(40.0, -75.0)))
        try h3Validate(cell: h3Encode(coord: wgs84(40.7128, -74.006), resolution: 9))
    }

    func testNmeaRmcCarriesItsDate() throws {
        let rmc = try parseNmeaSentence(
            sentence: "$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10")
        XCTAssertEqual(rmc.timestamp, Date(timeIntervalSince1970: 764_426_119))
    }
}

final class TypedErrorTests: XCTestCase {
    func testOutOfRangeCarriesThePosition() {
        guard case let .OutOfRange(lat, lon)? = geoError({
            try coordinateNew(lat: 91.0, lon: 0.0, crs: .wgs84)
        }) else { return XCTFail("expected OutOfRange") }
        XCTAssertEqual(lat, 91.0)
        XCTAssertEqual(lon, 0.0)
    }

    func testCrsMismatchCarriesBothSystems() {
        guard case let .CrsMismatch(expected, found)? = geoError({
            try baiduMercatorFromCoordinate(coord: wgs84(wgs.lat, wgs.lon))
        }) else { return XCTFail("expected CrsMismatch") }
        XCTAssertEqual(expected, .bd09)
        XCTAssertEqual(found, .wgs84)
    }

    func testOutsideDomainNamesTheOperation() {
        guard case let .OutsideDomain(pole, _, _)? = geoError({
            try gcj02ToBd09(p: Gcj02(lat: 90.0, lon: 0.0))
        }) else { return XCTFail("expected OutsideDomain past the pole") }
        XCTAssertEqual(pole, "BD-09")
        guard case let .OutsideDomain(polar, _, _)? = geoError({
            try baiduMercatorFromBd09(p: Bd09(lat: 80.0, lon: 0.0))
        }) else { return XCTFail("expected OutsideDomain beyond 74 degrees") }
        XCTAssertEqual(polar, "Baidu Mercator")
    }

    func testInvalidValueNamesTheField() {
        guard case let .InvalidValue(field, _)? = geoError({
            try utmCentralMeridianDeg(zone: 0)
        }) else { return XCTFail("expected InvalidValue") }
        XCTAssertEqual(field, "zone")
        guard case let .InvalidValue(capField, _)? = geoError({
            try upsValidate(ups: Ups(hemisphere: .north, easting: 0.0, northing: 0.0))
        }) else { return XCTFail("expected InvalidValue from upsValidate") }
        XCTAssertEqual(capField, "northing")
    }

    func testFixAccuracyIsTaggedAndValidated() throws {
        let gga = try parseNmeaSentence(
            sentence: "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")
        XCTAssertEqual(gga.accuracy?.kind, .oneSigma)
        XCTAssertEqual(gga.source?.geoidSeparationM, 46.9)
        XCTAssertEqual(try parseCoordinate(input: "18TWK0000027757").accuracy?.kind, .bound)
        XCTAssertEqual(
            try accuracyNew(kind: .cep50, horizontalM: 3.0, verticalM: nil),
            Accuracy(kind: .cep50, horizontalM: 3.0, verticalM: nil))
        guard case let .InvalidValue(horizontal, _)? = geoError({
            try accuracyNew(kind: .bound, horizontalM: -1.0, verticalM: nil)
        }) else { return XCTFail("expected InvalidValue for a negative accuracy") }
        XCTAssertEqual(horizontal, "horizontal")
        guard case let .InvalidValue(confidence, _)? = geoError({
            try confidenceNew(value: 1.5)
        }) else { return XCTFail("expected InvalidValue for confidence 1.5") }
        XCTAssertEqual(confidence, "confidence")
        var handBuilt = try fixFromCoord(coord: wgs84(40.7128, -74.006))
        handBuilt.accuracy = Accuracy(kind: .oneSigma, horizontalM: -1.0, verticalM: nil)
        let fix = handBuilt
        guard case .InvalidValue? = geoError({
            try formatFix(fix: fix, options: formatOptionsDefault())
        }) else { return XCTFail("expected a hand-built Fix to be validated") }
    }

    func testInvalidCellIdNamesTheGridAndValue() {
        guard case let .InvalidCellId(grid, value)? = geoError({
            try h3Decode(cell: H3Cell(value: 0))
        }) else { return XCTFail("expected InvalidCellId") }
        XCTAssertEqual(grid, "H3")
        XCTAssertEqual(value, 0)
        guard case let .InvalidCellId(s2Grid, _)? = geoError({
            try s2Validate(cell: S2CellId(value: 0))
        }) else { return XCTFail("expected InvalidCellId from s2Validate") }
        XCTAssertEqual(s2Grid, "S2")
    }

    func testInvalidGridRefCarriesTheReference() {
        guard case let .InvalidGridRef(detail)? = geoError({
            try mgrsToCoordinate(mgrs: "not-an-mgrs")
        }) else { return XCTFail("expected InvalidGridRef") }
        XCTAssertEqual(detail, "not-an-mgrs")
    }

    func testParseRejectsAmbiguousTextAndNoFixNmea() {
        for (input, expected) in [("1234.5, 10", ParseErrorKind.ambiguous), ("ezs42", .syntax)] {
            guard case let .Parse(kind, _)? = geoError({ try parseCoordinate(input: input) }) else {
                return XCTFail("expected Parse for \(input)")
            }
            XCTAssertEqual(kind, expected, input)
        }
        guard case let .Parse(voidKind, detail)? = geoError({
            try parseNmeaSentence(
                sentence: "$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07")
        }) else { return XCTFail("expected Parse for a void RMC") }
        XCTAssertEqual(voidKind, .noFix)
        XCTAssertTrue(detail.contains("void"), detail)
        guard case let .Parse(checksumKind, _)? = geoError({
            try parseNmeaSentence(sentence: "$GPGLL,4916.45,N,12311.12,W,225444,A,*1E")
        }) else { return XCTFail("expected Parse for a bad checksum") }
        XCTAssertEqual(checksumKind, .checksum)
        let deepWkt = String(repeating: "GEOMETRYCOLLECTION(", count: 200)
        guard case let .Parse(deepKind, _)? = geoError({
            try parseWkt(input: deepWkt, axisOrder: .lonLat)
        }) else { return XCTFail("expected Parse for deep WKT") }
        XCTAssertEqual(deepKind, .tooDeep)
    }
}
