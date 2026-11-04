pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

rootProject.name = "viewer-gateway"

dependencyResolutionManagement {
    @Suppress("UnstableApiUsage")
    repositories {
        mavenCentral()
    }
    // The fork's catalog, so Kotlin, Ktor and kotlinx versions match the modules this build includes.
    versionCatalogs {
        create("libs") {
            from(files("../minecraft-protocol/gradle/libs.versions.toml"))
        }
    }
}

// The minecraft-protocol fork, consumed from source: `com.hiczp:<module>` resolves to its projects.
includeBuild("../minecraft-protocol")

include(
    ":protocol",
    ":assets-tool",
    ":app",
)
