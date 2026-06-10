#!/usr/bin/env bash
# Build the Swift XCFramework + generated Swift sources for `geocoordinates`.
#
# Produces:
#   swift/Sources/GeoCoordinates/geocoordinates_ffi.swift   (committed source)
#   swift/geocoordinates_ffiFFI.xcframework                 (git-ignored, local)
#   swift/geocoordinates_ffiFFI.xcframework.zip             (git-ignored; uploaded
#                                                            to the GitHub Release)
#
# Slices: macOS (arm64+x86_64), iOS device (arm64), iOS simulator (arm64+x86_64).
# Device and simulator are separate xcframework slices — they cannot be lipo'd
# together (both are arm64); lipo only fuses arches *within* a platform.
#
# Requires: macOS with Xcode, a Rust toolchain, and the Apple targets (added below).
set -euo pipefail
cd "$(dirname "$0")/../../.." # repo root

LIB=libgeocoordinates_ffi.a
FFI_MODULE=geocoordinates_ffiFFI
BUILD=target/swift-xcframework
SWIFT_DIR=swift

MACOS_TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
IOS_DEVICE=aarch64-apple-ios
IOS_SIM_TARGETS=(aarch64-apple-ios-sim x86_64-apple-ios)
ALL_TARGETS=("${MACOS_TARGETS[@]}" "$IOS_DEVICE" "${IOS_SIM_TARGETS[@]}")

echo "==> Adding Apple targets"
for t in "${ALL_TARGETS[@]}"; do rustup target add "$t" >/dev/null; done

echo "==> Building staticlib for each Apple target"
for t in "${ALL_TARGETS[@]}"; do
  echo "    $t"
  cargo build --release -p geocoordinates-ffi --target "$t"
done

echo "==> lipo within platform"
rm -rf "$BUILD"
mkdir -p "$BUILD/macos" "$BUILD/ios" "$BUILD/ios-sim"
lipo -create "target/aarch64-apple-darwin/release/$LIB" "target/x86_64-apple-darwin/release/$LIB" \
  -output "$BUILD/macos/$LIB"
cp "target/$IOS_DEVICE/release/$LIB" "$BUILD/ios/$LIB"
lipo -create "target/aarch64-apple-ios-sim/release/$LIB" "target/x86_64-apple-ios/release/$LIB" \
  -output "$BUILD/ios-sim/$LIB"

echo "==> Generating Swift bindings (sources + header + modulemap)"
# A host dylib is enough for UniFFI to extract the interface metadata.
cargo build --release -p geocoordinates-ffi
# Pick the host library by extension (note: a bare `ls a b` exits non-zero when
# one path is missing, which would trip `set -e`).
HOST_LIB=""
for ext in dylib so; do
  cand="target/release/libgeocoordinates_ffi.$ext"
  [ -f "$cand" ] && HOST_LIB="$cand" && break
done
cargo run -q -p geocoordinates-ffi --bin uniffi-bindgen -- generate --no-format \
  --library "$HOST_LIB" --language swift --out-dir "$BUILD/gen"

# XCFramework headers dir: the C header + a modulemap named `module.modulemap`.
HDR="$BUILD/headers"
mkdir -p "$HDR"
cp "$BUILD/gen/$FFI_MODULE.h" "$HDR/"
cp "$BUILD/gen/$FFI_MODULE.modulemap" "$HDR/module.modulemap"

echo "==> Creating XCFramework"
rm -rf "$SWIFT_DIR/$FFI_MODULE.xcframework"
xcodebuild -create-xcframework \
  -library "$BUILD/macos/$LIB" -headers "$HDR" \
  -library "$BUILD/ios/$LIB" -headers "$HDR" \
  -library "$BUILD/ios-sim/$LIB" -headers "$HDR" \
  -output "$SWIFT_DIR/$FFI_MODULE.xcframework"

echo "==> Staging generated Swift source"
mkdir -p "$SWIFT_DIR/Sources/GeoCoordinates"
cp "$BUILD/gen/geocoordinates_ffi.swift" "$SWIFT_DIR/Sources/GeoCoordinates/"

echo "==> Zipping XCFramework"
rm -f "$SWIFT_DIR/$FFI_MODULE.xcframework.zip"
ditto -c -k --sequesterRsrc --keepParent \
  "$SWIFT_DIR/$FFI_MODULE.xcframework" "$SWIFT_DIR/$FFI_MODULE.xcframework.zip"

CHECKSUM=$(swift package compute-checksum "$SWIFT_DIR/$FFI_MODULE.xcframework.zip")
echo "==> Done."
echo "    XCFramework: $SWIFT_DIR/$FFI_MODULE.xcframework"
echo "    Zip:         $SWIFT_DIR/$FFI_MODULE.xcframework.zip"
echo "    Checksum:    $CHECKSUM"
# Emit the checksum for CI consumption.
if [ -n "${GITHUB_OUTPUT:-}" ]; then echo "checksum=$CHECKSUM" >> "$GITHUB_OUTPUT"; fi
