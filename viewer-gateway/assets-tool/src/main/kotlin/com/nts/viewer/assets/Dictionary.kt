package com.nts.viewer.assets

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.nio.file.Files
import java.nio.file.Path
import java.util.zip.ZipFile

/**
 * The block-state and biome dictionary every display-protocol ID indexes into, built from official data only:
 * state IDs from the server data generator's `reports/blocks.json`, biomes from the vanilla data pack in the
 * client JAR. [json] is the serialized form; its SHA-256 is the `dictionary_hash` of a session's Hello.
 */
class Dictionary(val json: JsonObject, val blockStateCount: Int, val biomeCount: Int)

private const val DICTIONARY_FORMAT = 1
private const val BIOME_PREFIX = "data/minecraft/worldgen/biome/"

/** Runs the official server's data generator in [workDirectory] and returns its `blocks.json` report. */
fun generateBlocksReport(serverJar: Path, workDirectory: Path): JsonObject {
    val output = workDirectory.resolve("generated")
    val process = ProcessBuilder(
        // Bounded heap: the generator's default is a quarter of physical memory, and `--reports` needs far less.
        "java", "-Xmx2g", "--enable-native-access=ALL-UNNAMED", "-DbundlerMainClass=net.minecraft.data.Main",
        "-jar", serverJar.toAbsolutePath().toString(), "--reports", "--output", output.toAbsolutePath().toString(),
    ).directory(workDirectory.toFile()).redirectErrorStream(true).start()
    val log = process.inputStream.bufferedReader().readText()
    val exitCode = process.waitFor()
    check(exitCode == 0) { "Official data generator exited with $exitCode:\n${log.lines().takeLast(40).joinToString("\n")}" }
    val report = output.resolve("reports/blocks.json")
    check(Files.isRegularFile(report)) { "Official data generator did not write reports/blocks.json" }
    return Json.parseToJsonElement(Files.readString(report)).jsonObject
}

/**
 * `block_states[id]` is `[block, {property: value}]`; IDs must be dense from 0, each used once. `blocks` maps a
 * block to its default state ID. `biomes` is sorted by identifier and its index is the biome ID; each entry keeps
 * the climate and visual fields of the official biome file and drops generation-only fields.
 */
fun buildDictionary(gameVersion: String, blocksReport: JsonObject, clientJar: Path): Dictionary {
    val states = HashMap<Int, JsonArray>()
    val defaults = sortedMapOf<String, JsonElement>()
    for ((block, entry) in blocksReport) {
        for (state in entry.jsonObject.getValue("states").jsonArray) {
            val stateObject = state.jsonObject
            val id = stateObject.getValue("id").jsonPrimitive.int
            val properties = stateObject["properties"]?.jsonObject ?: JsonObject(emptyMap())
            check(states.put(id, JsonArray(listOf(JsonPrimitive(block), properties))) == null) { "State ID $id appears twice" }
            if (stateObject["default"]?.jsonPrimitive?.content == "true") defaults[block] = JsonPrimitive(id)
        }
        check(block in defaults) { "Block $block has no default state" }
    }
    val blockStates = List(states.size) { id -> states[id] ?: error("State IDs are not dense: $id is missing") }
    val biomes = readBiomes(clientJar)
    val json = JsonObject(
        linkedMapOf(
            "format" to JsonPrimitive(DICTIONARY_FORMAT),
            "game_version" to JsonPrimitive(gameVersion),
            "block_states" to JsonArray(blockStates),
            "blocks" to JsonObject(defaults),
            "biomes" to JsonArray(biomes),
        ),
    )
    return Dictionary(json, blockStates.size, biomes.size)
}

/** Fields of a biome file that describe world generation, not its appearance. */
private val GENERATION_FIELDS = setOf("carvers", "features", "spawners", "spawn_costs", "creature_spawn_probability")

private fun readBiomes(clientJar: Path): List<JsonObject> = ZipFile(clientJar.toFile()).use { zipFile ->
    zipFile.entries().asSequence()
        .filter { it.name.startsWith(BIOME_PREFIX) && it.name.endsWith(".json") && '/' !in it.name.removePrefix(BIOME_PREFIX) }
        .map { zipEntry ->
            val id = "minecraft:${zipEntry.name.removePrefix(BIOME_PREFIX).removeSuffix(".json")}"
            val fields = Json.parseToJsonElement(zipFile.getInputStream(zipEntry).bufferedReader().readText()).jsonObject
            id to JsonObject(linkedMapOf<String, JsonElement>("id" to JsonPrimitive(id)) + fields.filterKeys { it !in GENERATION_FIELDS }.toSortedMap())
        }
        .sortedBy { it.first }
        .map { it.second }
        .toList()
        .also { check(it.isNotEmpty()) { "The client JAR has no files under $BIOME_PREFIX" } }
}
