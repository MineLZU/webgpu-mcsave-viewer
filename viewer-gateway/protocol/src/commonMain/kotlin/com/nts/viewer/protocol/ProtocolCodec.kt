package com.nts.viewer.protocol

/**
 * Encodes and decodes display-protocol v1 messages. Both directions enforce the same layout rules as the Rust
 * codec, so the shared sample vectors decode identically and every decoded value re-encodes to the same bytes.
 * Session-dependent checks are separate: see [SessionBounds].
 */
object ProtocolCodec {
    const val PROTOCOL_VERSION: Int = 1
    const val SECTION_CELLS: Int = 4096
    const val BIOME_CELLS: Int = 64
    const val MAX_DIMENSIONS: Int = 64
    const val MAX_RANGES: Int = 64

    private const val WORLD_MIN_Y = -2032
    private const val WORLD_END_Y = 2032

    private const val HELLO = 0x01
    private const val SUBSCRIBE = 0x02
    private const val SECTION_SNAPSHOT = 0x03
    private const val BLOCK_DELTA = 0x04
    private const val LIGHT_UPDATE = 0x05
    private const val CHUNK_LIFECYCLE = 0x06
    private const val RESYNC_REQUEST = 0x07

    fun encode(message: Message): ByteArray {
        val writer = Writer()
        when (message) {
            is Hello -> encodeHello(writer, message)
            is Subscribe -> encodeSubscribe(writer, message)
            is SectionSnapshot -> encodeSnapshot(writer, message)
            is BlockDelta -> encodeDelta(writer, message)
            is LightUpdate -> {
                writer.u8(LIGHT_UPDATE)
                encodePos(writer, message.pos)
                writer.u64(message.lightRevision)
                encodeLight(writer, message.skyLight)
                encodeLight(writer, message.blockLight)
            }
            is ChunkLifecycle -> {
                writer.u8(CHUNK_LIFECYCLE)
                writer.u16(message.dimension)
                writer.i32(message.x)
                writer.i32(message.z)
                writer.u8(message.state.ordinal)
            }
            is ResyncRequest.Subscription -> { writer.u8(RESYNC_REQUEST); writer.u8(0); writer.u32(message.generation) }
            is ResyncRequest.Section -> { writer.u8(RESYNC_REQUEST); writer.u8(1); encodePos(writer, message.pos) }
        }
        return writer.toByteArray()
    }

    private fun invalid(reason: String): Nothing = throw ProtocolException.Invalid(reason)

    private fun checkDimension(dimension: Dimension) {
        val end = dimension.minY.toLong() + dimension.height
        if (dimension.id.isEmpty() || dimension.minY % 16 != 0 || dimension.height == 0L ||
            dimension.height % 16 != 0L || dimension.minY < WORLD_MIN_Y || end > WORLD_END_Y
        ) {
            invalid("dimension \"${dimension.id}\" has min_y ${dimension.minY} and height ${dimension.height}; both must be multiples of 16 inside $WORLD_MIN_Y..$WORLD_END_Y")
        }
    }

    private fun checkHello(hello: Hello) {
        if (hello.protocolVersion != PROTOCOL_VERSION) invalid("protocol version ${hello.protocolVersion} is not $PROTOCOL_VERSION")
        if (hello.blockStateCount == 0L || hello.biomeCount == 0L) invalid("dictionary has no block states or no biomes")
        if (hello.dimensions.isEmpty() || hello.dimensions.size > MAX_DIMENSIONS) {
            invalid("${hello.dimensions.size} dimensions is outside 1..=$MAX_DIMENSIONS")
        }
        if (hello.dictionaryHash.size != 32 || hello.assetsHash.size != 32) invalid("hashes must be 32 bytes")
        hello.dimensions.forEach(::checkDimension)
    }

    private fun encodeHello(writer: Writer, hello: Hello) {
        checkHello(hello)
        writer.u8(HELLO)
        writer.u16(hello.protocolVersion)
        writer.u64(hello.sessionId)
        writer.string(hello.gameVersion)
        writer.bytes(hello.dictionaryHash)
        writer.bytes(hello.assetsHash)
        writer.u32(hello.blockStateCount)
        writer.u32(hello.biomeCount)
        writer.u8(hello.dimensions.size)
        hello.dimensions.forEach { dimension ->
            writer.string(dimension.id)
            writer.i32(dimension.minY)
            writer.u32(dimension.height)
        }
    }

    private fun checkRanges(ranges: List<ChunkRange>) {
        if (ranges.size > MAX_RANGES) invalid("${ranges.size} ranges is more than $MAX_RANGES")
        ranges.firstOrNull { it.minX > it.maxX || it.minZ > it.maxZ }?.let { invalid("chunk range $it has a minimum above its maximum") }
    }

    private fun encodeSubscribe(writer: Writer, subscribe: Subscribe) {
        checkRanges(subscribe.ranges)
        writer.u8(SUBSCRIBE)
        writer.u32(subscribe.generation)
        writer.u16(subscribe.dimension)
        writer.u8(subscribe.ranges.size)
        subscribe.ranges.forEach { chunkRange ->
            writer.i32(chunkRange.minX); writer.i32(chunkRange.minZ); writer.i32(chunkRange.maxX); writer.i32(chunkRange.maxZ)
        }
    }

    private fun encodePos(writer: Writer, sectionPos: SectionPos) {
        writer.u16(sectionPos.dimension)
        writer.i32(sectionPos.x)
        writer.i32(sectionPos.y)
        writer.i32(sectionPos.z)
    }

    private fun encodeLight(writer: Writer, lightLayer: LightLayer) = when (lightLayer) {
        LightLayer.Unknown -> writer.u8(0)
        is LightLayer.Data -> { writer.u8(1); writer.bytes(lightLayer.nibbles) }
    }

    private fun checkPaletted(paletted: Paletted, cells: Int, what: String) {
        if (paletted !is Paletted.Indexed) return
        val palette = paletted.palette
        if (palette.size !in 2..cells) invalid("$what palette length ${palette.size} is outside 2..=$cells")
        if (palette.toSet().size != palette.size) invalid("$what palette repeats a value")
        if (paletted.indices.size != cells) invalid("$what has ${paletted.indices.size} cells, expected $cells")
        paletted.indices.firstOrNull { it !in palette.indices }?.let { invalid("$what index $it is outside its palette") }
    }

    private fun encodePaletted(writer: Writer, paletted: Paletted, cells: Int, what: String) {
        checkPaletted(paletted, cells, what)
        when (paletted) {
            is Paletted.Single -> { writer.u8(0); writer.u32(paletted.value) }
            is Paletted.Indexed -> {
                writer.u8(1)
                writer.u16(paletted.palette.size)
                paletted.palette.forEach(writer::u32)
                val bits = Paletted.bitsForPalette(paletted.palette.size)
                writer.u8(bits)
                val packed = ByteArray((cells * bits + 7) / 8)
                paletted.indices.forEachIndexed { cell, index ->
                    for (bit in 0 until bits) {
                        if ((index ushr bit) and 1 == 1) {
                            val position = cell * bits + bit
                            packed[position / 8] = (packed[position / 8].toInt() or (1 shl (position % 8))).toByte()
                        }
                    }
                }
                writer.bytes(packed)
            }
        }
    }

    private fun encodeSnapshot(writer: Writer, snapshot: SectionSnapshot) {
        writer.u8(SECTION_SNAPSHOT)
        encodePos(writer, snapshot.pos)
        writer.u64(snapshot.revision)
        writer.u64(snapshot.lightRevision)
        encodePaletted(writer, snapshot.blocks, SECTION_CELLS, "blocks")
        encodePaletted(writer, snapshot.biomes, BIOME_CELLS, "biomes")
        encodeLight(writer, snapshot.skyLight)
        encodeLight(writer, snapshot.blockLight)
    }

    private fun checkDelta(delta: BlockDelta) {
        if (delta.revision.toULong() <= delta.baseRevision.toULong()) {
            invalid("delta revision ${delta.revision.toULong()} does not follow base revision ${delta.baseRevision.toULong()}")
        }
        if (delta.changes.size !in 1..SECTION_CELLS) invalid("${delta.changes.size} changes is outside 1..=$SECTION_CELLS")
        val seen = BooleanArray(SECTION_CELLS)
        delta.changes.forEach { blockChange ->
            if (blockChange.index !in 0 until SECTION_CELLS) invalid("change index ${blockChange.index} is outside the Section")
            if (seen[blockChange.index]) invalid("change index ${blockChange.index} appears twice")
            seen[blockChange.index] = true
        }
    }

    private fun encodeDelta(writer: Writer, delta: BlockDelta) {
        checkDelta(delta)
        writer.u8(BLOCK_DELTA)
        encodePos(writer, delta.pos)
        writer.u64(delta.baseRevision)
        writer.u64(delta.revision)
        writer.u16(delta.changes.size)
        delta.changes.forEach { blockChange -> writer.u16(blockChange.index); writer.u32(blockChange.state) }
    }

    /** Decodes one whole message; [bytes] must hold exactly one message and nothing after it. */
    fun decode(bytes: ByteArray): Message {
        val reader = Reader(bytes)
        val message = when (val tag = reader.u8()) {
            HELLO -> decodeHello(reader)
            SUBSCRIBE -> decodeSubscribe(reader)
            SECTION_SNAPSHOT -> SectionSnapshot(
                pos = decodePos(reader),
                revision = reader.u64(),
                lightRevision = reader.u64(),
                blocks = decodePaletted(reader, SECTION_CELLS, "blocks"),
                biomes = decodePaletted(reader, BIOME_CELLS, "biomes"),
                skyLight = decodeLight(reader),
                blockLight = decodeLight(reader),
            )
            BLOCK_DELTA -> decodeDelta(reader)
            LIGHT_UPDATE -> LightUpdate(decodePos(reader), reader.u64(), decodeLight(reader), decodeLight(reader))
            CHUNK_LIFECYCLE -> ChunkLifecycle(
                dimension = reader.u16(),
                x = reader.i32(),
                z = reader.i32(),
                state = reader.u8().let { state ->
                    ChunkState.entries.getOrNull(state) ?: invalid("chunk state $state is not 0, 1 or 2")
                },
            )
            RESYNC_REQUEST -> when (val kind = reader.u8()) {
                0 -> ResyncRequest.Subscription(reader.u32())
                1 -> ResyncRequest.Section(decodePos(reader))
                else -> invalid("resync kind $kind is neither 0 nor 1")
            }
            else -> throw ProtocolException.UnknownMessageType(tag)
        }
        reader.finish()
        return message
    }

    private fun decodeHello(reader: Reader): Hello {
        val protocolVersion = reader.u16()
        if (protocolVersion != PROTOCOL_VERSION) invalid("protocol version $protocolVersion is not $PROTOCOL_VERSION")
        val sessionId = reader.u64()
        val gameVersion = reader.string()
        val dictionaryHash = reader.take(32)
        val assetsHash = reader.take(32)
        val blockStateCount = reader.u32()
        val biomeCount = reader.u32()
        val dimensions = List(reader.u8()) { Dimension(reader.string(), reader.i32(), reader.u32()) }
        return Hello(protocolVersion, sessionId, gameVersion, dictionaryHash, assetsHash, blockStateCount, biomeCount, dimensions)
            .also(::checkHello)
    }

    private fun decodeSubscribe(reader: Reader): Subscribe {
        val generation = reader.u32()
        val dimension = reader.u16()
        val ranges = List(reader.u8()) { ChunkRange(reader.i32(), reader.i32(), reader.i32(), reader.i32()) }
        checkRanges(ranges)
        return Subscribe(generation, dimension, ranges)
    }

    private fun decodePos(reader: Reader): SectionPos = SectionPos(reader.u16(), reader.i32(), reader.i32(), reader.i32())

    private fun decodeLight(reader: Reader): LightLayer = when (val tag = reader.u8()) {
        0 -> LightLayer.Unknown
        1 -> LightLayer.Data(reader.take(2048))
        else -> invalid("light layer tag $tag is neither 0 nor 1")
    }

    private fun decodePaletted(reader: Reader, cells: Int, what: String): Paletted {
        val paletted = when (val tag = reader.u8()) {
            0 -> Paletted.Single(reader.u32())
            1 -> {
                val paletteSize = reader.u16()
                if (paletteSize !in 2..cells) invalid("$what palette length $paletteSize is outside 2..=$cells")
                val palette = LongArray(paletteSize) { reader.u32() }
                val bits = reader.u8()
                if (bits != Paletted.bitsForPalette(paletteSize)) {
                    invalid("$what index width $bits does not match palette length $paletteSize")
                }
                val packed = reader.take((cells * bits + 7) / 8)
                val indices = IntArray(cells) { cell ->
                    (0 until bits).fold(0) { index, bit ->
                        val position = cell * bits + bit
                        index or (((packed[position / 8].toInt() ushr (position % 8)) and 1) shl bit)
                    }
                }
                Paletted.Indexed(palette, indices)
            }
            else -> invalid("$what container tag $tag is neither 0 nor 1")
        }
        checkPaletted(paletted, cells, what)
        return paletted
    }

    private fun decodeDelta(reader: Reader): BlockDelta {
        val pos = decodePos(reader)
        val baseRevision = reader.u64()
        val revision = reader.u64()
        val changes = List(reader.u16()) { BlockChange(reader.u16(), reader.u32()) }
        return BlockDelta(pos, baseRevision, revision, changes).also(::checkDelta)
    }
}
