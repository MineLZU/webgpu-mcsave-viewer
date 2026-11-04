package com.nts.viewer.protocol

/**
 * The facts of one session a well-formed message is checked against, taken from its [Hello]: dimension index,
 * section Y inside that dimension, and state/biome IDs inside the dictionary. Mirrors the Rust `SessionBounds`.
 */
class SessionBounds(val blockStateCount: Long, val biomeCount: Long, val sectionYs: List<IntRange>) {
    constructor(hello: Hello) : this(
        hello.blockStateCount,
        hello.biomeCount,
        hello.dimensions.map { dimension -> dimension.minY / 16 until (dimension.minY + dimension.height.toInt()) / 16 },
    )

    /** Throws [ProtocolException.OutOfBounds] when [message] refers to something this session lacks. */
    fun check(message: Message) {
        when (message) {
            is Hello -> Unit
            is Subscribe -> dimension(message.dimension)
            is SectionSnapshot -> {
                pos(message.pos)
                ids(message.blocks.values, blockStateCount, "block state")
                ids(message.biomes.values, biomeCount, "biome")
            }
            is BlockDelta -> {
                pos(message.pos)
                ids(LongArray(message.changes.size) { message.changes[it].state }, blockStateCount, "block state")
            }
            is LightUpdate -> pos(message.pos)
            is ChunkLifecycle -> dimension(message.dimension)
            is ResyncRequest.Section -> pos(message.pos)
            is ResyncRequest.Subscription -> Unit
        }
    }

    private fun dimension(dimension: Int): IntRange = sectionYs.getOrNull(dimension)
        ?: throw ProtocolException.OutOfBounds("dimension index $dimension is not one of ${sectionYs.size}")

    private fun pos(sectionPos: SectionPos) {
        val range = dimension(sectionPos.dimension)
        if (sectionPos.y !in range) throw ProtocolException.OutOfBounds("section Y ${sectionPos.y} is outside $range")
    }

    private fun ids(ids: LongArray, count: Long, what: String) {
        ids.firstOrNull { it >= count }?.let { throw ProtocolException.OutOfBounds("$what ID $it is not below $count") }
    }
}
