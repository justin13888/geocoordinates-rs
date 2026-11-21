// Behavioural tests of reverse geocoding through the Swift bindings, including a
// Swift `ReverseGeocoder` implementation crossing into Rust (the shape a
// `CLGeocoder` adapter takes).
import Foundation
import GeoCoordinates
import XCTest

/// The core crate's doctest gazetteer, found from this file's path.
private let fixture = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()  // GeoCoordinatesTests
    .deletingLastPathComponent()  // Tests
    .deletingLastPathComponent()  // swift
    .deletingLastPathComponent()  // repository root
    .appendingPathComponent("src/reverse/data/fixture.gaz")

private func at(_ lat: Double, _ lon: Double) throws -> Coordinate {
    try coordinateNew(lat: lat, lon: lon, crs: .wgs84)
}

/// A Swift geocoder: every point is in one fixed place.
private final class Everywhere: ReverseGeocoder, @unchecked Sendable {
    let countryCode: String

    init(countryCode: String) {
        self.countryCode = countryCode
    }

    func reverse(coordinate: Coordinate, options: ReverseOptions) throws -> Place? {
        if coordinate.lat < -80 {
            return nil
        }
        return try placeNew(
            name: "Everywhere", coordinate: at(0, 0), countryCode: countryCode,
            query: coordinate, queryMaxErrorM: 0)
    }
}

/// A Swift geocoder whose provider fails, as a platform adapter's would.
private final class Failing: ReverseGeocoder, @unchecked Sendable {
    let failure: Error

    init(failure: Error) {
        self.failure = failure
    }

    func reverse(coordinate: Coordinate, options: ReverseOptions) throws -> Place? {
        throw failure
    }
}

/// A provider failure that is not a `GeoError`.
private struct ProviderOffline: Error, CustomStringConvertible {
    var description: String { "provider offline" }
}

final class ReverseTests: XCTestCase {
    func testGazetteerLoadsAndResolves() throws {
        let gazetteer = try Gazetteer.fromBytes(bytes: Data(contentsOf: fixture))
        XCTAssertEqual(gazetteer.len(), 4)
        XCTAssertEqual(gazetteer.languages(), ["fr", "ja"])
        XCTAssertEqual(try Gazetteer.open(path: fixture.path).sourceDate(), "2026-11-03")
        var options = reverseOptionsDefault()
        options.languages = ["ja"]
        let paris = try XCTUnwrap(gazetteer.reverse(coordinate: at(48.86, 2.35), options: options))
        XCTAssertEqual(paris.name, "Paris")
        XCTAssertEqual(paris.localizedNames["ja"], "パリ")
        XCTAssertEqual(paris.featureClass, .capital)
        XCTAssertNil(try gazetteer.reverse(coordinate: at(0, -30), options: reverseOptionsDefault()))
        XCTAssertThrowsError(try Gazetteer.fromBytes(bytes: Data("GCGZ".utf8))) { error in
            guard case let .Parse(kind, _)? = error as? GeoError else {
                return XCTFail("expected Parse, got \(error)")
            }
            XCTAssertEqual(kind, .syntax)
        }
    }

    func testASwiftGeocoderCrossesIntoRust() throws {
        let found = try XCTUnwrap(
            reverseGeocode(
                geocoder: Everywhere(countryCode: "XX"), coordinate: at(0, 1),
                options: reverseOptionsDefault()))
        XCTAssertEqual(found.name, "Everywhere")
        XCTAssertGreaterThan(found.distanceM, 100_000)
        XCTAssertNil(
            try reverseGeocode(
                geocoder: Everywhere(countryCode: "XX"), coordinate: at(-85, 0),
                options: reverseOptionsDefault()))
        XCTAssertThrowsError(
            try reverseGeocode(
                geocoder: Everywhere(countryCode: "nowhere"), coordinate: at(0, 1),
                options: reverseOptionsDefault())
        ) { error in
            guard case let .InvalidValue(field, _)? = error as? GeoError else {
                return XCTFail("expected InvalidValue, got \(error)")
            }
            XCTAssertEqual(field, "country_code")
        }
        let gazetteer = gazetteerAsReverseGeocoder(
            gazetteer: try Gazetteer.fromBytes(bytes: Data(contentsOf: fixture)))
        let lyon = try reverseGeocode(
            geocoder: gazetteer, coordinate: at(45.75, 4.85), options: reverseOptionsDefault())
        XCTAssertEqual(lyon?.name, "Lyon")
    }

    func testAThrowingForeignGeocoderSurfacesItsGeoError() throws {
        let failure = GeoError.InvalidValue(field: "language", detail: "provider has no such language")
        XCTAssertThrowsError(
            try reverseGeocode(
                geocoder: Failing(failure: failure), coordinate: at(0, 1),
                options: reverseOptionsDefault())
        ) { error in
            guard case let .InvalidValue(field, detail)? = error as? GeoError else {
                return XCTFail("expected InvalidValue, got \(error)")
            }
            XCTAssertEqual(field, "language")
            XCTAssertEqual(detail, "provider has no such language")
        }
    }

    func testAThrowingForeignGeocodersUnexpectedErrorBecomesOther() throws {
        XCTAssertThrowsError(
            try reverseGeocode(
                geocoder: Failing(failure: ProviderOffline()), coordinate: at(0, 1),
                options: reverseOptionsDefault())
        ) { error in
            guard case let .Other(detail)? = error as? GeoError else {
                return XCTFail("expected Other, got \(error)")
            }
            XCTAssertTrue(detail.contains("provider offline"), detail)
        }
    }
}
