import org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest

plugins {
    alias(libs.plugins.kotlinMultiplatform)
}

// The sample vectors the Rust codec writes; both codecs must agree with them byte for byte.
val samplesDir = rootProject.layout.projectDirectory.dir("../crates/viewer-protocol/samples").asFile.canonicalPath

kotlin {
    jvmToolchain(25)
    jvm()
    mingwX64()

    sourceSets {
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(libs.kotlinx.serialization.json)
            implementation(libs.kotlinx.io.core)
        }
    }
}

tasks.withType<Test>().configureEach {
    environment("VIEWER_PROTOCOL_SAMPLES", samplesDir)
    inputs.dir(samplesDir)
}

tasks.withType<KotlinNativeTest>().configureEach {
    environment("VIEWER_PROTOCOL_SAMPLES", samplesDir)
    inputs.dir(samplesDir)
}
