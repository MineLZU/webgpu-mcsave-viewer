import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget

plugins {
    alias(libs.plugins.kotlinMultiplatform)
}

kotlin {
    jvmToolchain(25)
    // Native is the deliverable; the JVM target runs the same code for quick local debugging.
    jvm {
        binaries {
            executable {
                mainClass = "com.nts.viewer.gateway.MainKt"
            }
        }
    }
    mingwX64()

    targets.withType<KotlinNativeTarget>().configureEach {
        binaries.executable {
            baseName = "viewer-gateway"
            entryPoint = "com.nts.viewer.gateway.main"
        }
    }

    sourceSets {
        commonMain.dependencies {
            implementation(project(":protocol"))
            implementation(libs.ktor.server.core)
            implementation(libs.ktor.server.cio)
            implementation(libs.ktor.server.websockets)
            implementation(libs.kotlinx.serialization.json)
            implementation(libs.kotlinx.io.core)
            implementation(libs.kotlinx.coroutines.core)
        }
    }
}
