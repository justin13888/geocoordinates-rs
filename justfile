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

# --- Mutation testing (cargo-mutants) ---
# Central flags shared by both recipes: published crate only, all features.
mutants_flags := "--package geocoordinates --all-features"

# Mutation-test only the lines changed vs trunk (the gate run by pre-push + CI).
# Override the base ref with e.g. `just mutants origin/main`.
mutants base="origin/master":
    #!/usr/bin/env bash
    set -euo pipefail
    base="{{ base }}"
    if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
        if git rev-parse --verify --quiet "master^{commit}" >/dev/null; then
            base="master"
        else
            echo "mutants: no '$base' or 'master' ref found; skipping." >&2
            exit 0
        fi
    fi
    diff="$(mktemp)"
    trap 'rm -f "$diff"' EXIT
    git diff "$base"...HEAD -- '*.rs' > "$diff"
    if [ ! -s "$diff" ]; then
        echo "mutants: no Rust changes vs $base; nothing to test."
        exit 0
    fi
    cargo mutants {{ mutants_flags }} --in-diff "$diff"

# Full-tree mutation test of the published crate (manual deep audit; slow, may report gaps).
mutants-all:
    cargo mutants {{ mutants_flags }}

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
