package com.nts.viewer.protocol

import kotlinx.io.buffered
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import kotlinx.io.readByteArray
import kotlinx.io.readString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.fail

/** The environment variable the Gradle test tasks set to `crates/viewer-protocol/samples`. */
internal expect fun environmentVariable(name: String): String?

/**
 * Decodes every vector the Rust codec wrote to `crates/viewer-protocol/samples` and requires the outcome its
 * `manifest.json` states: the same decoded fields, the same re-encoded bytes, or the same error category.
 */
class SharedSamplesTest {
    private val samplesDir = Path(environmentVariable("VIEWER_PROTOCOL_SAMPLES") ?: fail("VIEWER_PROTOCOL_SAMPLES is not set"))

    private fun read(name: String): ByteArray = SystemFileSystem.source(Path(samplesDir, name)).buffered().use { it.readByteArray() }

    private val manifest: JsonObject = Json.parseToJsonElement(
        SystemFileSystem.source(Path(samplesDir, "manifest.json")).buffered().use { it.readString() },
    ).jsonObject

    @Test
    fun everySampleDecodesAsTheManifestSays() {
        assertEquals(ProtocolCodec.PROTOCOL_VERSION, manifest.getValue("protocol_version").jsonPrimitive.content.toInt())
        val hello = ProtocolCodec.decode(read(manifest.getValue("session_hello").jsonPrimitive.content)) as Hello
        val sessionBounds = SessionBounds(hello)
        val samples = manifest.getValue("samples").jsonArray
        assertEquals(40, samples.size, "sample count")
        samples.forEach { sample -> checkSample(sample.jsonObject, sessionBounds) }
    }

    private fun checkSample(sample: JsonObject, sessionBounds: SessionBounds) {
        val name = sample.getValue("name").jsonPrimitive.content
        val bytes = read(sample.getValue("file").jsonPrimitive.content)
        when (val expect = sample.getValue("expect").jsonPrimitive.content) {
            "ok" -> {
                val message = ProtocolCodec.decode(bytes)
                sessionBounds.check(message)
                assertEquals(sample.getValue("message"), describe(message), name)
                assertContentEquals(bytes, ProtocolCodec.encode(message), "$name re-encodes differently")
            }
            "decode_error" -> {
                val exception = assertFailsWith<ProtocolException>(name) { ProtocolCodec.decode(bytes) }
                assertEquals(sample.getValue("error").jsonPrimitive.content, exception.kind, "$name: ${exception.message}")
            }
            "out_of_bounds" -> {
                val message = ProtocolCodec.decode(bytes)
                assertEquals(sample.getValue("message"), describe(message), name)
                val exception = assertFailsWith<ProtocolException>(name) { sessionBounds.check(message) }
                assertEquals("out_of_bounds", exception.kind, name)
            }
            else -> fail("$name: unknown expectation $expect")
        }
    }

    private fun number(value: Number) = JsonPrimitive(value)
    private fun array(vararg elements: JsonElement) = JsonArray(elements.toList())
    private fun pos(sectionPos: SectionPos) = JsonObject(
        mapOf("dimension" to number(sectionPos.dimension), "x" to number(sectionPos.x), "y" to number(sectionPos.y), "z" to number(sectionPos.z)),
    )

    private val probes = intArrayOf(0, 1, cellIndex(3, 2, 5), 2047, 2048, 4095)

    private fun light(lightLayer: LightLayer) = JsonArray(
        probes.map { cell -> array(number(cell), if (lightLayer is LightLayer.Data) number(lightLayer[cell]) else JsonNull) },
    )

    private fun palette(paletted: Paletted) = JsonArray(paletted.values.map(::number))

    /** The same fields, in the same shapes, as `describe` in `crates/viewer-protocol/tests/samples.rs`. */
    private fun describe(message: Message): JsonObject = JsonObject(
        when (message) {
            is Hello -> mapOf(
                "type" to JsonPrimitive("hello"),
                "session_id" to JsonPrimitive(message.sessionId.toULong().toString()),
                "game_version" to JsonPrimitive(message.gameVersion),
                "block_state_count" to number(message.blockStateCount),
                "biome_count" to number(message.biomeCount),
                "dimensions" to JsonArray(message.dimensions.map { array(JsonPrimitive(it.id), number(it.minY), number(it.height)) }),
            )
            is Subscribe -> mapOf(
                "type" to JsonPrimitive("subscribe"),
                "generation" to number(message.generation),
                "dimension" to number(message.dimension),
                "ranges" to JsonArray(message.ranges.map { array(number(it.minX), number(it.minZ), number(it.maxX), number(it.maxZ)) }),
            )
            is SectionSnapshot -> mapOf(
                "type" to JsonPrimitive("section_snapshot"),
                "pos" to pos(message.pos),
                "revision" to number(message.revision),
                "light_revision" to number(message.lightRevision),
                "block_palette" to palette(message.blocks),
                "biome_palette" to palette(message.biomes),
                "cells" to JsonArray(probes.map { cell -> array(number(cell), number(message.blocks[cell])) }),
                "biome_cells" to JsonArray(listOf(0, 1, 63).map { cell -> array(number(cell), number(message.biomes[cell])) }),
                "sky_light" to light(message.skyLight),
                "block_light" to light(message.blockLight),
            )
            is BlockDelta -> mapOf(
                "type" to JsonPrimitive("block_delta"),
                "pos" to pos(message.pos),
                "base_revision" to number(message.baseRevision),
                "revision" to number(message.revision),
                "changes" to JsonArray(message.changes.map { array(number(it.index), number(it.state)) }),
            )
            is LightUpdate -> mapOf(
                "type" to JsonPrimitive("light_update"),
                "pos" to pos(message.pos),
                "light_revision" to number(message.lightRevision),
                "sky_light" to light(message.skyLight),
                "block_light" to light(message.blockLight),
            )
            is ChunkLifecycle -> mapOf(
                "type" to JsonPrimitive("chunk_lifecycle"),
                "dimension" to number(message.dimension),
                "x" to number(message.x),
                "z" to number(message.z),
                "state" to number(message.state.ordinal),
            )
            is ResyncRequest.Subscription -> mapOf(
                "type" to JsonPrimitive("resync_request"), "kind" to JsonPrimitive("subscription"),
                "generation" to number(message.generation),
            )
            is ResyncRequest.Section -> mapOf(
                "type" to JsonPrimitive("resync_request"), "kind" to JsonPrimitive("section"), "pos" to pos(message.pos),
            )
        },
    )
}

