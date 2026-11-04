package com.nts.viewer.assets

import com.hiczp.minecraft.distribution.metadata.MinecraftDistributionMetadataApiClient
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.serialization.kotlinx.json.json
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.deleteRecursively

/**
 * Exports the dictionary and block-model asset pack of one release for the gateway to serve:
 * `--version <id> --out <dir> [--cache <dir>]`. Writes `dictionary.json`, `assets.zip` and `manifest.json`,
 * which names both files by SHA-256 together with the official JAR hashes they came from.
 */
@OptIn(kotlin.io.path.ExperimentalPathApi::class)
fun main(args: Array<String>) {
    val options = args.toList().chunked(2).associate { (name, value) -> name to value }
    val gameVersion = options["--version"] ?: error("--version is required, e.g. --version 1.21.11")
    val outputDirectory = Path.of(options["--out"] ?: "generated/$gameVersion")
    val cacheDirectory = Path.of(options["--cache"] ?: "build/official")
    val json = Json { ignoreUnknownKeys = true }
    val officialFiles = HttpClient(CIO) { install(ContentNegotiation) { json(json) } }.use { httpClient ->
        runBlocking { MinecraftDistributionMetadataApiClient(httpClient).officialFiles(gameVersion, cacheDirectory) }
    }
    Files.createDirectories(outputDirectory)
    val workDirectory = Files.createTempDirectory("viewer-datagen")
    val dictionary = try {
        buildDictionary(gameVersion, generateBlocksReport(officialFiles.serverJar, workDirectory), officialFiles.clientJar)
    } finally {
        workDirectory.deleteRecursively()
    }
    val dictionaryFile = outputDirectory.resolve("dictionary.json")
    Files.writeString(dictionaryFile, Json.encodeToString(JsonObject.serializer(), dictionary.json))
    val assetsFile = outputDirectory.resolve("assets.zip")
    val assetEntries = writeAssetPack(officialFiles.clientJar, assetsFile)
    val manifest = JsonObject(
        linkedMapOf(
            "game_version" to JsonPrimitive(gameVersion),
            "dictionary" to JsonPrimitive(dictionaryFile.fileName.toString()),
            "dictionary_sha256" to JsonPrimitive(dictionaryFile.digest("SHA-256")),
            "block_state_count" to JsonPrimitive(dictionary.blockStateCount),
            "biome_count" to JsonPrimitive(dictionary.biomeCount),
            "assets" to JsonPrimitive(assetsFile.fileName.toString()),
            "assets_sha256" to JsonPrimitive(assetsFile.digest("SHA-256")),
            "asset_entries" to JsonPrimitive(assetEntries),
            "client_jar_sha1" to JsonPrimitive(officialFiles.clientJar.digest("SHA-1")),
            "server_jar_sha1" to JsonPrimitive(officialFiles.serverJar.digest("SHA-1")),
        ),
    )
    Files.writeString(outputDirectory.resolve("manifest.json"), Json { prettyPrint = true }.encodeToString(JsonObject.serializer(), manifest))
    println("Exported $gameVersion: ${dictionary.blockStateCount} block states, ${dictionary.biomeCount} biomes, $assetEntries asset files -> $outputDirectory")
}
