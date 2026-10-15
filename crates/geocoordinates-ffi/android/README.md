# Android Library

The Android release contains `geocoordinates-android.aar`, built with UniFFI's
Android Kotlin bindings and native libraries for **arm64-v8a**, **armeabi-v7a**,
**x86_64**, and **x86**. Requires **Android API 23+**. The native libraries use
16 KB ELF page alignment. Kotlin and Java can call the generated API.

## Add to an app

Download the AAR from an `Android Library` GitHub Release and copy it into
`app/libs/`. In the application's `build.gradle.kts`:

```kotlin
android {
    compileOptions {
        isCoreLibraryDesugaringEnabled = true
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

dependencies {
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.1.5")
    implementation(files("libs/geocoordinates-android.aar"))
    implementation("net.java.dev.jna:jna:5.18.1@aar")
    implementation("androidx.annotation:annotation:1.9.1")
}
```

Use `google()` and `mavenCentral()` repositories. A locally imported AAR has no
transitive dependency metadata, so the JNA **AAR** and AndroidX annotation
dependency must be declared explicitly. Do not also include the desktop JVM
`geocoordinates` JAR: both artifacts define the same binding classes.
Core library desugaring provides the `java.time` APIs used by UniFFI timestamps
on Android API 23–25. Use Android Gradle Plugin 8.9.2+ and compile SDK 35+.

```kotlin
import uniffi.geocoordinates_ffi.Wgs84
import uniffi.geocoordinates_ffi.wgs84ToGcj02

val gcj = wgs84ToGcj02(Wgs84(lat = 39.915, lon = 116.404))
```

Consumer R8/ProGuard rules are embedded in the AAR for UniFFI and JNA reflection.
No API key or network access is needed for coordinate calculations.

## Automated releases

`.github/workflows/release-android.yml` checks the default branch daily at
**06:00 Asia/Shanghai (22:00 UTC the previous day)**. GitHub may delay scheduled
runs. The root Cargo version must equal upstream's latest official GitHub
Release version. Each version is published once as `android-vX.Y.Z`, such as
`android-v0.14.2`; there is no independent Android version bump or date suffix.
Unreleased development versions wait until upstream publishes them. Build
failures leave the version unpublished, so the next run retries.
Manual dispatch performs the same check; only default-branch runs can publish.
Relevant pull requests build and lint the AAR without publishing or deleting.

The workflow first creates a draft, uploads the AAR, checksum and documentation,
then publishes it without changing GitHub's latest release designation.
After successful publication, it retains the newest **three published Android
Library releases**, deleting older releases and their tags. Other release
families are untouched. An ownership marker in the release notes identifies
this workflow's releases; keep that marker when editing release notes.
Unpublished drafts from failed uploads do not count as successful releases and
are removed during the next successful cleanup. Cleanup also runs on unchanged
daily checks, allowing it to retry a previous retention failure.

Only the built-in `GITHUB_TOKEN` with `contents: write` in the publishing job is
required; no Maven Central account, PAT, or signing secret is needed. Enable
Actions and scheduled workflows in forks. The workflow must be on the default
branch for the schedule to run.

## Daily upstream synchronization

`.github/workflows/sync-upstream.yml` runs at **05:00 Asia/Shanghai**. It merges
`justin13888/geocoordinates-rs:master` into the fork's current default-branch
history, then uses `peter-evans/create-pull-request@v7` to create/update the
`sync/upstream` PR. It merges that PR while respecting branch protection and
dispatches the Android release check explicitly (bot-created merges do not
trigger other workflows through ordinary push events).

Fork-specific Android files and commits are preserved. Merge conflicts stop the
workflow for manual resolution; required reviews/checks can also block merging.
The built-in token needs `contents: write`, `pull-requests: write`, and
`actions: write`. Repository settings must allow Actions to create pull requests.
Forks skip the upstream `release-plz` jobs, so only upstream controls crate
versions and official registry publication.

## Local build

Use JDK 17, Gradle 8.11.1, Android SDK 35, NDK 28.2.13676358, Rust stable and
`cargo-ndk` 4.1.2. From the repository root, run the generation, native build,
and Gradle commands in `release-android.yml`. The generated Kotlin and `jniLibs`
trees are build outputs and must come from the same checkout. The resulting AAR
is under `crates/geocoordinates-ffi/android/build/outputs/aar/`.
