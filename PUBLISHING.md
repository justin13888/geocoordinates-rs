# Publishing the language bindings

The `geocoordinates-ffi` UniFFI bindings are published to each ecosystem's registry
by per-language GitHub Actions workflows, using that ecosystem's native tooling. The
core Rust crate continues to publish to crates.io via release-plz, untouched.

## Trigger & versioning

When a release PR merges, `release-plz.yml` cuts the **`v*` GitHub Release** and then
**dispatches** the four binding workflows, passing the released tag + version and
`publish=true`. Each tracks the Rust crate version, so it is the single source of truth
and all packages release in lockstep. Each workflow also accepts a plain manual
`workflow_dispatch` (with `publish` defaulting to **false**) that **builds/packages but
does not publish** — use it to dry-run a pipeline.

> **No tokens to manage.** A GitHub Release created with the default `GITHUB_TOKEN` does
> not fire `release: published` (GitHub's recursion guard), so the binding workflows
> can't simply trigger off the release. Rather than introduce a PAT (which GitHub forces
> to expire), `release-plz.yml` **dispatches** them: `workflow_dispatch` is explicitly
> exempt from that guard — it runs even when invoked with the default token — so the
> whole chain needs no stored, expiring secret. (Registry auth is separate and uses
> trusted publishing / OIDC where the ecosystem supports it; see the table below.)
> The binding workflows keep a `release: published` trigger too, so manually publishing
> a GitHub Release still works as an escape hatch.

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
