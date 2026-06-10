# Publishing the language bindings

The `geocoordinates-ffi` UniFFI bindings are published to each ecosystem's registry
by per-language GitHub Actions workflows, using that ecosystem's native tooling. The
core Rust crate continues to publish to crates.io via release-plz, untouched.

## Trigger & versioning

Every binding workflow runs on the **`v*` GitHub Release** that release-plz creates
when a release PR merges. Each derives the version from the release tag, so the Rust
crate version is the single source of truth and all packages release in lockstep.
Each workflow also has a `workflow_dispatch` entry that **builds/packages but does not
publish** — use it to dry-run a pipeline.

| Registry | Package | Workflow | Tooling | Auth |
|---|---|---|---|---|
| PyPI | `geocoordinates-rs` | `release-python.yml` | maturin (`bindings=uniffi`) + maturin-action matrix | OIDC trusted publishing (no secret) |
| npm | `geocoordinates` | `release-web.yml` | `ubrn` + wasm-pack (WASM) | `NPM_TOKEN` secret (+ provenance) |
| Maven Central | `io.github.justin13888:geocoordinates` | `release-jvm.yml` | Gradle + JNA fat-JAR + vanniktech plugin | Central Portal token + GPG (secrets) |
| SwiftPM | `GeoCoordinates` | `release-swift.yml` | XCFramework on the GitHub Release + root `Package.swift` | none (uses `GITHUB_TOKEN`) |

## Account / secret setup

### PyPI (OIDC, no secret)
The bare `geocoordinates` is taken on PyPI by an unrelated project, so the
distribution is **`geocoordinates-rs`** (the import module stays `geocoordinates_ffi`).
Configure a **Trusted Publisher** on PyPI for project `geocoordinates-rs`:
repo `justin13888/geocoordinates-rs`, workflow `release-python.yml`, no environment.
A "pending publisher" creates the project on the first CI publish — no token, no
local bootstrap needed.

### npm — `NPM_TOKEN` secret
npm cannot pre-create an empty package, so the first publish creates it with a token
(OIDC trusted publishing can be adopted afterwards). Create an **automation** access
token on npmjs.com (with publish rights), then add it as the repo secret `NPM_TOKEN`.
`--provenance` still attaches a build attestation via GitHub OIDC.

### Maven Central — Central Portal + GPG (not yet set up)
1. Create a **Sonatype Central Portal** account (central.sonatype.com).
2. Verify the **namespace** `io.github.justin13888` (the portal walks you through a
   GitHub-based verification — it has you create a throwaway public repo whose name it
   specifies).
3. Generate a **publishing (user) token** in the portal → two values.
4. Create a **GPG key**, publish its public key to a keyserver
   (`keys.openpgp.org` / `keyserver.ubuntu.com`), and export the **ASCII-armored
   private key**.
5. Add these GitHub repo secrets:
   - `MAVEN_CENTRAL_USERNAME` / `MAVEN_CENTRAL_PASSWORD` — the portal token pair.
   - `GPG_SIGNING_KEY` — the ASCII-armored private key (`-----BEGIN PGP PRIVATE KEY…`).
   - `GPG_SIGNING_PASSWORD` — the key passphrase.

Until these exist, the `release-jvm.yml` publish step **skips with a warning** and the
other registries are unaffected. (Desktop JVM only for now; Android AAR is future work.)

### SwiftPM — nothing to configure
The release workflow builds the XCFramework, uploads it to the GitHub Release, writes
the URL + checksum into `Package.swift`, and re-points the tag so SwiftPM resolves a
matching manifest. Optionally list the package on the Swift Package Index.

## Local builds

- Python wheel: `cd crates/geocoordinates-ffi && maturin build --release`
  (temporarily set `default-members = ["crates/geocoordinates-ffi"]` so maturin's
  `cargo run --bin uniffi-bindgen` resolves; the workflow does this automatically).
- WASM/npm: `cd crates/geocoordinates-ffi/web && npm ci && npm run build`
  (needs the `wasm32-unknown-unknown` target + `wasm-pack`).
- Swift XCFramework: `crates/geocoordinates-ffi/swift/build-xcframework.sh`, then build
  against it with `GEOCOORDINATES_LOCAL_XCFRAMEWORK=1`.
- JVM JAR: stage the cdylib + generated Kotlin, then `gradle build` in
  `crates/geocoordinates-ffi/jvm` (see `release-jvm.yml`).
