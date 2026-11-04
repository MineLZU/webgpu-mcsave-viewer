plugins {
    alias(libs.plugins.kotlinJvm)
    application
}

kotlin {
    jvmToolchain(25)
}

dependencies {
    // Resolved from the included minecraft-protocol build.
    implementation("com.hiczp:distribution-metadata")
    implementation(libs.ktor.client.cio)
    implementation(libs.ktor.client.content.negotiation)
    implementation(libs.ktor.serialization.kotlinx.json)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.core)
    testImplementation(kotlin("test"))
}

application {
    mainClass = "com.nts.viewer.assets.MainKt"
}

tasks.named<JavaExec>("run") {
    // Relative `--out`/`--cache` arguments resolve against the gateway build root.
    workingDir = rootProject.projectDir
    // The tool only streams downloads and zips; the data generator it starts has its own bound.
    maxHeapSize = "1g"
}
