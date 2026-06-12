// swift-tools-version:5.9
import PackageDescription
import Foundation

// Swift Package for `geocoordinates` (the UniFFI bindings).
//
// The native code ships as a prebuilt XCFramework attached to the matching
// GitHub Release; this manifest references it as a binaryTarget by URL + checksum,
// which the release workflow rewrites on each release.
//
// For local development against a freshly built XCFramework, set
// GEOCOORDINATES_LOCAL_XCFRAMEWORK=1 and run `crates/geocoordinates-ffi/swift/build-xcframework.sh`.
let useLocalFramework = ProcessInfo.processInfo.environment["GEOCOORDINATES_LOCAL_XCFRAMEWORK"] != nil

let ffiTarget: Target = useLocalFramework
    ? .binaryTarget(
        name: "geocoordinates_ffiFFI",
        path: "swift/geocoordinates_ffiFFI.xcframework"
    )
    // BINARY_TARGET_REMOTE — updated by .github/workflows/release-swift.yml
    : .binaryTarget(
        name: "geocoordinates_ffiFFI",
        url: "https://github.com/justin13888/geocoordinates-rs/releases/download/v0.1.1/geocoordinates_ffiFFI.xcframework.zip",
        checksum: "3a5122dc4a53c5dfdbbe210beb0dc40d64f15165c147c19c372301df65cadb95"
    )

let package = Package(
    name: "GeoCoordinates",
    platforms: [.iOS(.v13), .macOS(.v11)],
    products: [
        .library(name: "GeoCoordinates", targets: ["GeoCoordinates"])
    ],
    targets: [
        ffiTarget,
        .target(
            name: "GeoCoordinates",
            dependencies: [.target(name: "geocoordinates_ffiFFI")],
            path: "swift/Sources/GeoCoordinates"
        ),
    ]
)
