plugins {
    alias(libs.plugins.kotlinJvm) apply false
    alias(libs.plugins.kotlinMultiplatform) apply false
    alias(libs.plugins.kotlinSerialization) apply false
}

group = "com.nts.viewer"
version = "0.1.0"

subprojects {
    group = rootProject.group
    version = rootProject.version
}
