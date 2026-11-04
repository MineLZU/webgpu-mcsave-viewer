package com.nts.viewer.protocol

/**
 * The messages of display protocol v1. Layout and validation rules are in `docs/viewer-protocol.md`; the Rust
 * codec in `crates/viewer-protocol` is the other implementation, and both are tested against its sample vectors.
 * Unsigned 32-bit fields are carried as [Long]; unsigned 64-bit fields keep their bits in a [Long].
 */
sealed interface Message

/** A dimension the session can show: [minY] is a multiple of 16, [height] a positive multiple of 16. */
data class Dimension(val id: String, val minY: Int, val height: Long)

/** Opens or replaces a session; everything received before it no longer applies. */
class Hello(
    val protocolVersion: Int,
    val sessionId: Long,
    val gameVersion: String,
    /** SHA-256 of the dictionary that state and biome IDs index into. */
    val dictionaryHash: ByteArray,
    /** SHA-256 of the block-model asset pack. */
    val assetsHash: ByteArray,
    val blockStateCount: Long,
    val biomeCount: Long,
    /** Messages name a dimension by its index here. */
    val dimensions: List<Dimension>,
) : Message {
    override fun equals(other: Any?): Boolean = other is Hello && protocolVersion == other.protocolVersion &&
        sessionId == other.sessionId && gameVersion == other.gameVersion &&
        dictionaryHash.contentEquals(other.dictionaryHash) && assetsHash.contentEquals(other.assetsHash) &&
        blockStateCount == other.blockStateCount && biomeCount == other.biomeCount && dimensions == other.dimensions

    override fun hashCode(): Int = listOf(
        protocolVersion, sessionId, gameVersion, dictionaryHash.contentHashCode(), assetsHash.contentHashCode(),
        blockStateCount, biomeCount, dimensions,
    ).hashCode()
}

/** An inclusive rectangle of chunk columns. */
data class ChunkRange(val minX: Int, val minZ: Int, val maxX: Int, val maxZ: Int)

/** Replaces the sender's whole interest; an empty list unsubscribes. Only the newest [generation] counts. */
data class Subscribe(val generation: Long, val dimension: Int, val ranges: List<ChunkRange>) : Message

/** A Section's position: dimension index, chunk column, and section Y (block Y divided by 16). */
data class SectionPos(val dimension: Int, val x: Int, val y: Int, val z: Int)

/** Index of block `(x, y, z)` (each `0 until 16`) inside a Section. */
fun cellIndex(x: Int, y: Int, z: Int): Int = (y * 16 + z) * 16 + x

/** One light layer: 4096 nibbles in 2048 bytes, low nibble first, cells ordered as [cellIndex]. */
sealed interface LightLayer {
    data object Unknown : LightLayer

    class Data(val nibbles: ByteArray) : LightLayer {
        init {
            require(nibbles.size == 2048) { "A light layer holds 2048 bytes, not ${nibbles.size}" }
        }

        operator fun get(cell: Int): Int = (nibbles[cell / 2].toInt() ushr (cell % 2 * 4)) and 0xf

        override fun equals(other: Any?): Boolean = other is Data && nibbles.contentEquals(other.nibbles)
        override fun hashCode(): Int = nibbles.contentHashCode()
    }
}

/** The complete state of one Section at [revision]; an all-air snapshot clears earlier content. */
data class SectionSnapshot(
    val pos: SectionPos,
    /** Block and biome revision; a [BlockDelta] applies on top of it. */
    val revision: Long,
    /** Counted separately because light changes without blocks changing. */
    val lightRevision: Long,
    val blocks: Paletted,
    val biomes: Paletted,
    val skyLight: LightLayer,
    val blockLight: LightLayer,
) : Message

data class BlockChange(val index: Int, val state: Long)

/** Changes that apply only on top of [baseRevision] and produce [revision]. */
data class BlockDelta(val pos: SectionPos, val baseRevision: Long, val revision: Long, val changes: List<BlockChange>) :
    Message

data class LightUpdate(val pos: SectionPos, val lightRevision: Long, val skyLight: LightLayer, val blockLight: LightLayer) :
    Message

/** A chunk column's state; [UNLOADED] and [UNAVAILABLE] keep no terrain and are not air. */
enum class ChunkState { HOT, UNLOADED, UNAVAILABLE }

data class ChunkLifecycle(val dimension: Int, val x: Int, val z: Int, val state: ChunkState) : Message

sealed interface ResyncRequest : Message {
    data class Subscription(val generation: Long) : ResyncRequest
    data class Section(val pos: SectionPos) : ResyncRequest
}
