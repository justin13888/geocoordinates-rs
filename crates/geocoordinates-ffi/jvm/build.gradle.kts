import com.vanniktech.maven.publish.SonatypeHost
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

// JVM packaging for the UniFFI Kotlin bindings (also the artifact Java consumes).
//
// The generated Kotlin (src/main/kotlin/uniffi/...) and the per-platform native
// libraries (src/main/resources/<os>-<arch>/) are produced by the release
// workflow before `gradle`; both are git-ignored. The Kotlin bindings load the
// native library through JNA, which auto-extracts it from the JAR using the
// `<os>-<arch>/` resource layout, so the published JAR is self-contained.
plugins {
    kotlin("jvm") version "2.1.0"
    id("com.vanniktech.maven.publish") version "0.29.0"
}

group = "io.github.justin13888"

// The workspace version (`[workspace.package] version` in the repository's root
// Cargo.toml), the one version every artifact of a release ships. Read rather
// than copied, so it cannot drift; release-jvm.yml passes the same value as
// -PreleaseVersion.
val workspaceVersion: String =
    rootDir.resolve("../../../Cargo.toml").readLines()
        .map { it.trim() }
        .dropWhile { it != "[workspace.package]" }
        .drop(1)
        .takeWhile { !it.startsWith("[") }
        .firstOrNull { it.substringBefore("=").trim() == "version" }
        ?.substringAfter("=")?.trim()?.removeSurrounding("\"")
        ?: error("no [workspace.package] version in the root Cargo.toml")

version = (findProperty("releaseVersion") as String?) ?: workspaceVersion

repositories { mavenCentral() }

dependencies {
    // UniFFI's Kotlin backend loads the cdylib via JNA (JDK 8+, no Panama).
    implementation("net.java.dev.jna:jna:5.16.0")
    testImplementation(platform("org.junit:junit-bom:5.11.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
}

kotlin {
    compilerOptions { jvmTarget.set(JvmTarget.JVM_11) }
}
java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
    withSourcesJar()
}

tasks.test {
    useJUnitPlatform()
    // The smoke test loads the host cdylib directly via UniFFI's libraryOverride
    // hook, so it does not depend on the JNA `<os>-<arch>/` resource being staged.
    System.getenv("GEOCOORDINATES_NATIVE_LIB")?.let {
        systemProperty("uniffi.component.geocoordinates_ffi.libraryOverride", it)
    }
}

mavenPublishing {
    // automaticRelease: once the Central Portal validates the uploaded deployment,
    // it publishes to Maven Central on its own; without it, every deployment waits
    // in the portal for a manual Publish.
    publishToMavenCentral(SonatypeHost.CENTRAL_PORTAL, automaticRelease = true)
    signAllPublications()
    coordinates("io.github.justin13888", "geocoordinates", version.toString())
    pom {
        name.set("geocoordinates")
        description.set(
            "Geospatial coordinate library: China datums (GCJ-02/BD-09) and geodetic transforms — JVM/Kotlin bindings, callable from Java.",
        )
        inceptionYear.set("2026")
        url.set("https://github.com/justin13888/geocoordinates-rs")
        licenses {
            license {
                name.set("MIT")
                url.set("https://opensource.org/licenses/MIT")
            }
            license {
                name.set("Apache-2.0")
                url.set("https://www.apache.org/licenses/LICENSE-2.0")
            }
        }
        developers {
            developer {
                id.set("justin13888")
                name.set("Justin Chung")
            }
        }
        scm {
            url.set("https://github.com/justin13888/geocoordinates-rs")
            connection.set("scm:git:https://github.com/justin13888/geocoordinates-rs.git")
            developerConnection.set("scm:git:ssh://git@github.com/justin13888/geocoordinates-rs.git")
        }
    }
}
