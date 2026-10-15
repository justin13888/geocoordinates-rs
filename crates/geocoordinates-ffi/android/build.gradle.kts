plugins {
    id("com.android.library") version "8.9.2"
    kotlin("android") version "2.1.20"
}

group = "io.github.justin13888"
version = providers.gradleProperty("releaseVersion").getOrElse("0.0.0-local")

android {
    namespace = "io.github.justin13888.geocoordinates.android"
    compileSdk = 35
    ndkVersion = "28.2.13676358"

    defaultConfig {
        minSdk = 23
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        // UniFFI timestamps use java.time, including on API 23–25 devices.
        isCoreLibraryDesugaringEnabled = true
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions { jvmTarget = "11" }
}

dependencies {
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.1.5")
    // The Android AAR carries libjnidispatch.so; the desktop JAR does not.
    api("net.java.dev.jna:jna:5.18.1@aar")
    implementation("androidx.annotation:annotation:1.9.1")
}
