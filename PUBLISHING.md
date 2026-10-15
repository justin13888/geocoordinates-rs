# Publishing the language bindings

The `geocoordinates-ffi` UniFFI bindings are published to each ecosystem's registry
by per-language GitHub Actions workflows, using that ecosystem's native tooling. The
core Rust crate continues to publish to crates.io via release-plz, untouched.

Android AARs use a separate daily GitHub Release pipeline, described under
[Android Library](#android-library--github-releases) below. The registry trigger
and versioning rules in the next section apply to the four other binding workflows.

## Trigger & versioning

When a release PR merges, `release-plz.yml` cuts the **`v*` GitHub Release** and then
**dispatches** the four binding workflows, passing the released tag + version and
`publish=true`. Each tracks the Rust crate version, so it is the single source of truth
and all packages release in lockstep. Each workflow also accepts a plain manual
`workflow_dispatch` (with `publish` defaulting to **false**) that **builds/packages but
does not publish** — use it to dry-run a pipeline.

The same dry run also runs **automatically**, so a pipeline's scripted steps (version
stamping, native staging, the `Package.swift` checksum rewrite) are exercised between
releases rather than first at one:

- on every **pull request** that touches the workflow itself, the shared
  `.github/actions/release-plan/**` action, `Cargo.toml`, `Cargo.lock`, or
  `crates/geocoordinates-ffi/**` (plus `Package.swift` / `swift/**` for SwiftPM, and
  `.mise.toml` for JVM), and
- **weekly** (Mondays 06:00 UTC), to catch runner, toolchain, and action drift.

Neither event can publish: the publish steps run only on `release: published` or a
dispatch with `publish=true`. Every non-publishing run (these automatic runs and a manual
dry-run dispatch alike) takes the version from the root `Cargo.toml` when it has no tag
or `version` input; a publishing run with neither fails instead. A dry run on SwiftPM
rewrites `Package.swift` in the runner's checkout only; it uploads nothing, commits nothing, and moves no tag.

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
| npm | `geocoordinates-rs` | `release-web.yml` | `ubrn` + wasm-pack (WASM) | OIDC trusted publishing (no secret) |
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

### npm (OIDC, no secret)
The bare `geocoordinates` is blocked by npm's name-similarity filter (too close to
the existing `geo-coordinates`), so the package is **`geocoordinates-rs`**, matching
PyPI. Configure a **Trusted Publisher** on npmjs.com for package `geocoordinates-rs`:
repo `justin13888/geocoordinates-rs`, workflow `release-web.yml`.
OIDC cannot publish a package's *first* version — the package must already exist to
host the trusted-publisher config — so `v0.1.1` was bootstrapped with a one-time
local `npm publish`. Every release since uses OIDC, with `--provenance` attaching a
build attestation.

### Maven Central — Central Portal + GPG (configured; releases not yet visible)
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

These secrets are set, so a publishing run of `release-jvm.yml` runs
`publishToMavenCentral` and uploads the deployment (v0.14.1's run succeeded). No
version is on Maven Central yet, though: the Gradle build calls
`publishToMavenCentral(SonatypeHost.CENTRAL_PORTAL)` without automatic release, so each
uploaded deployment most likely waits in the portal for a manual **Publish**. If the
secrets are ever removed, the publish step skips with a warning instead, and the other
registries are unaffected either way. This artifact targets desktop JVM; Android
uses the separate AAR release pipeline below.

### Android Library — GitHub Releases

`sync-upstream.yml` uses `peter-evans/create-pull-request` daily at 05:00
Asia/Shanghai to merge upstream history into the fork through a PR. It preserves
fork changes and respects merge conflicts and branch protection. After a merge,
it dispatches the Android release check explicitly.

`release-android.yml` also checks daily at 06:00 Asia/Shanghai. The root Cargo
version must match upstream's latest official release; each version is published
once as `android-vX.Y.Z`. It builds an AAR with Kotlin bindings
and four Android ABIs, uploads it to a dedicated GitHub Release, and
keeps only the newest three Android releases and their tags. Other release
families are not pruned. Manual dispatch checks for a new upstream version too; pull requests
build and lint without publishing. Only default-branch runs publish, using the
built-in `GITHUB_TOKEN`; no registry credentials are needed.
The fork skips upstream's `release-plz` jobs, avoiding independent version bumps
and registry publication.

See [Android Library](crates/geocoordinates-ffi/android/README.md) for installation,
JNA dependencies, scheduling, retention, and local build details.

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
