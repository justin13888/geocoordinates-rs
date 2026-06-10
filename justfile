# List available recipes
default:
    @just --list

# Format all code
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt --check

# Lint with clippy (warnings denied)
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Lint and auto-fix where possible
lint-fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features

# Run the test suite
test:
    cargo test --all-features

# Report code coverage (no enforced threshold yet)
coverage:
    cargo llvm-cov --all-features

# Build the crate
build:
    cargo build --all-features

# Build documentation
doc:
    cargo doc --no-deps --all-features

# fmt-check + lint + test (CI parity)
check: fmt-check lint test

# --- FFI bindings (geocoordinates-ffi) ---
# The recipes above are scoped to the published crate via `default-members`;
# the FFI crate is built/linted explicitly here.

# Shared library the bindings are generated from (OS-specific extension).
_ffi_lib := if os() == "macos" {
    "target/release/libgeocoordinates_ffi.dylib"
} else {
    "target/release/libgeocoordinates_ffi.so"
}

# Lint the FFI bindings crate (warnings denied).
ffi-check:
    cargo clippy -p geocoordinates-ffi --all-targets -- -D warnings

# Build the release cdylib the bindings are generated from.
bindings-build:
    cargo build --release -p geocoordinates-ffi

# Generate <lang> bindings from the built cdylib (UniFFI library mode).
_bindings lang:
    cargo run -q -p geocoordinates-ffi --bin uniffi-bindgen -- \
        generate --library {{ _ffi_lib }} --language {{ lang }} --out-dir bindings/{{ lang }}

bindings-python: bindings-build (_bindings "python")
bindings-kotlin: bindings-build (_bindings "kotlin")
bindings-swift: bindings-build (_bindings "swift")

# Generate the TypeScript / WebAssembly bindings (uniffi-bindgen-react-native).
# Not a built-in UniFFI backend: builds the crate to wasm32 via wasm-pack.
# Requires the `wasm32-unknown-unknown` target, `wasm-pack`, and Node.js.
bindings-web:
    cd crates/geocoordinates-ffi/web && npm install && npm run build

# Generate bindings for every target language.
# (Java is served by the Kotlin/JVM artifact, so it has no separate generation.)
bindings: bindings-python bindings-kotlin bindings-swift bindings-web
