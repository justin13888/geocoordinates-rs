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

The first hosted release succeeded on 2026-10-15 at 11:15 Asia/Shanghai:

- Release: [Android Library 0.14.2](https://github.com/uJZk/geocoordinates-rs/releases/tag/android-v0.14.2).
- Workflow: [build, publish and cleanup passed](https://github.com/uJZk/geocoordinates-rs/actions/runs/38674781083).
- Source commit: `2945763901c9319df7e036585b8e8604f5c3e7bc`.
- Published AAR: 3,077,904 bytes. Re-downloaded successfully, SHA256SUMS passed,
  and all four native ABIs and consumer rules were confirmed.
- No additional secrets were needed beyond the built-in `GITHUB_TOKEN`.
- A [second check](https://github.com/uJZk/geocoordinates-rs/actions/runs/38675411367)
  passed with build/publication skipped, confirming that the same upstream
  version does not produce duplicate releases; retention still ran successfully.

The first synchronization run succeeded with no upstream changes (no empty PR).
The first Android attempt exposed a removed Google SDK `tools` package in the
setup action's default inputs; the workflow now requests only `platform-tools`
before installing its pinned SDK and NDK packages.

The first sync exercised the no-change path. Actual PR creation/merging awaits
the next upstream change; repository settings must allow Actions to create PRs.
Merge conflicts and protected-branch requirements still require resolution.

Install and local-build instructions:
[`crates/geocoordinates-ffi/android/README.md`](../crates/geocoordinates-ffi/android/README.md).

Future changes should preserve the ownership marker in release notes and avoid
deleting releases belonging to the Python, JVM, Swift or web pipelines.
