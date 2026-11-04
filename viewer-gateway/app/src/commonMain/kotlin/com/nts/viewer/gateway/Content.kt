package com.nts.viewer.gateway

import kotlinx.io.buffered
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import kotlinx.io.readByteArray
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

fun Path.readBytes(): ByteArray = SystemFileSystem.source(this).buffered().use { it.readByteArray() }

/** One file served by content hash: its bytes, lowercase SHA-256 hex and media type. */
class ContentFile(val bytes: ByteArray, val sha256: String, val contentType: String)

/**
 * A release export written by `:assets-tool:run`: its `manifest.json`, the dictionary every state and biome ID
 * indexes into, and the block-model asset pack. Hashes come from the manifest; browsers verify them on download.
 */
class ReleaseContent(val gameVersion: String, val dictionary: ContentFile, val assets: ContentFile) {
    private val dictionaryJson: JsonObject = Json.parseToJsonElement(dictionary.bytes.decodeToString()).jsonObject

    val blockStateCount: Int = dictionaryJson.getValue("block_states").jsonArray.size
    val biomeIds: List<String> = dictionaryJson.getValue("biomes").jsonArray.map { it.jsonObject.getValue("id").jsonPrimitive.content }

    /** `block[k=v,...]` with sorted keys, for every state; and a block's default state. */
    private val stateIds: Map<String, Int> = buildMap {
        dictionaryJson.getValue("block_states").jsonArray.forEachIndexed { id, entry ->
            val (block, properties) = entry.jsonArray
            put(stateKey(block.jsonPrimitive.content, properties.jsonObject.mapValues { it.value.jsonPrimitive.content }), id)
        }
    }
    private val defaultStates: Map<String, Int> =
        dictionaryJson.getValue("blocks").jsonObject.mapValues { it.value.jsonPrimitive.int }

    private fun stateKey(block: String, properties: Map<String, String>): String =
        // `toSortedMap` is JVM-only; sorting the entries works on every target.
        "$block[${properties.entries.sortedBy { it.key }.joinToString(",") { "${it.key}=${it.value}" }}]"

    /** The state ID of [block] with [properties] over its default state; fails for unknown names or values. */
    fun state(block: String, properties: Map<String, String> = emptyMap()): Int {
        val defaultState = defaultStates[block] ?: error("Dictionary has no block $block")
        if (properties.isEmpty()) return defaultState
        val defaultProperties = dictionaryJson.getValue("block_states").jsonArray[defaultState].jsonArray[1].jsonObject
            .mapValues { it.value.jsonPrimitive.content }
        return stateIds[stateKey(block, defaultProperties + properties)] ?: error("Dictionary has no state $block$properties")
    }

    fun biome(id: String): Int = biomeIds.indexOf(id).also { check(it >= 0) { "Dictionary has no biome $id" } }

    companion object {
        /** Loads the export in [directory]; its `manifest.json` names the two files and their hashes. */
        fun load(directory: Path): ReleaseContent {
            val manifest = Json.parseToJsonElement(Path(directory, "manifest.json").readBytes().decodeToString()).jsonObject
            fun file(name: String, hash: String, contentType: String) = ContentFile(
                Path(directory, manifest.getValue(name).jsonPrimitive.content).readBytes(),
                manifest.getValue(hash).jsonPrimitive.content.lowercase(),
                contentType,
            )
            val content = ReleaseContent(
                manifest.getValue("game_version").jsonPrimitive.content,
                file("dictionary", "dictionary_sha256", "application/json"),
                file("assets", "assets_sha256", "application/zip"),
            )
            check(content.blockStateCount == manifest.getValue("block_state_count").jsonPrimitive.int) {
                "dictionary.json does not hold the block_state_count its manifest states"
            }
            return content
        }
    }
}
