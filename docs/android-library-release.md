# Android Library release handoff

Updated: 2026-10-15 (Asia/Shanghai).

## Delivered

- `.github/workflows/release-android.yml` checks the default branch daily at
  06:00 Asia/Shanghai and supports manual dispatch. Relevant pull requests build
  without publishing.
- Android uses the root Cargo version only when it matches upstream's latest
  official GitHub Release. Each version is published once as `android-vX.Y.Z`;
  failed builds retry on subsequent checks. Development versions wait for the
  corresponding upstream release.
- `sync-upstream.yml` runs at 05:00 Asia/Shanghai, merges upstream master while
  preserving fork changes, uses `peter-evans/create-pull-request@v7`, merges the
  PR and dispatches an Android release check. Conflicts or branch protection
  stop automatic merging. Forks skip upstream's release-plz jobs to avoid
  independent version bumps and registry publication.
- The Android AAR includes Kotlin bindings and arm64-v8a, armeabi-v7a, x86_64,
  and x86 native libraries. The minimum Android API is 23; timestamps require
  core library desugaring on API 23–25.
- Dedicated `android-*` GitHub Releases carry the AAR, SHA256SUMS, installation
  guide and licenses. Uploads complete in a draft before publication.
- Retention keeps the newest three published releases owned by this pipeline,
  removes older owned releases/tags and abandoned upload drafts, and retries on
  unchanged daily checks. Other release families are protected by an ownership
  marker in the notes.
- Android builds disable the default `bindgen` feature to omit host CLI tooling.
  Existing host-generation commands retain their default feature behavior.

## Verification

- `mise run check`: Rust formatting, Clippy and core tests passed.
- `mise run ffi-check` and `mise run ffi-test`: passed.
- `cargo test --locked -p geocoordinates-ffi --no-default-features`: passed.
- Eleven Node release-planning/retention tests and actionlint: passed.
- Push hooks passed, including 98.85% line coverage (98% floor); no changed Rust
  source lines required mutation testing.
- NDK 28.2.13676358 release builds for all four Android ABIs: passed.
- Gradle 8.11.1/JDK 17 `assembleRelease lintRelease`: passed.
- Final local AAR: 3,078,017 bytes; Kotlin classes, consumer rules and all four
  native libraries verified. All native LOAD segments, and JNA's 64-bit native
  libraries, have 16 KB alignment.
- No Android device/emulator runtime test has been performed.

## Release operation

The first hosted build is requested as part of this handoff. Its result is
recorded below once the default-branch workflow finishes. No additional secrets
are needed beyond the publishing job's built-in `GITHUB_TOKEN`.

The first synchronization run succeeded with no upstream changes (no empty PR).
The first Android attempt exposed a removed Google SDK `tools` package in the
setup action's default inputs; the workflow now requests only `platform-tools`
before installing its pinned SDK and NDK packages.

Install and local-build instructions:
[`crates/geocoordinates-ffi/android/README.md`](../crates/geocoordinates-ffi/android/README.md).

Future changes should preserve the ownership marker in release notes and avoid
deleting releases belonging to the Python, JVM, Swift or web pipelines.
