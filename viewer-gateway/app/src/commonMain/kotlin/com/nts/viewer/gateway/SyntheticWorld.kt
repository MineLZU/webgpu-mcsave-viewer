package com.nts.viewer.gateway

import com.nts.viewer.protocol.LightLayer
import com.nts.viewer.protocol.Paletted
import com.nts.viewer.protocol.SectionPos
import com.nts.viewer.protocol.SectionSnapshot
import com.nts.viewer.protocol.cellIndex
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.sin

/**
 * Deterministic terrain for exercising the gateway and browser before a capture mod exists: rolling grass hills
 * over dirt, stone and bedrock, with oak trees, in a plains/forest overworld. Every block and biome is looked up
 * by name in the release dictionary, so the IDs are the real ones of that release. It is not a world save.
 */
class SyntheticWorld(private val content: ReleaseContent, val minY: Int = -64, val height: Int = 384) {
    private val air = content.state("minecraft:air").toLong()
    private val bedrock = content.state("minecraft:bedrock").toLong()
    private val stone = content.state("minecraft:stone").toLong()
    private val dirt = content.state("minecraft:dirt").toLong()
    private val grass = content.state("minecraft:grass_block", mapOf("snowy" to "false")).toLong()
    private val log = content.state("minecraft:oak_log", mapOf("axis" to "y")).toLong()
    private val leaves = content.state("minecraft:oak_leaves", mapOf("persistent" to "true", "distance" to "1")).toLong()
    private val plains = content.biome("minecraft:plains").toLong()
    private val forest = content.biome("minecraft:forest").toLong()

    val sectionYs: IntRange = minY / 16 until (minY + height) / 16

    /** Surface block Y of column `(x, z)`. */
    fun surface(x: Int, z: Int): Int =
        (68 + 6 * sin(x / 23.0) + 5 * cos(z / 17.0) + 3 * sin((x + z) / 9.0)).let(::floor).toInt()

    private fun hash(a: Int, b: Int): Int {
        var h = a * 0x27d4eb2d xor b * 0x165667b1
        h = (h xor (h ushr 15)) * 0x2c1b3c6d
        return h xor (h ushr 12)
    }

    /** The tree block at `(x, y, z)`, if any: trees stand on a 7-block grid, about half of the cells used. */
    private fun treeBlock(x: Int, y: Int, z: Int): Long? {
        val cellX = floor(x / 7.0).toInt()
        val cellZ = floor(z / 7.0).toInt()
        for (gridX in cellX - 1..cellX + 1) for (gridZ in cellZ - 1..cellZ + 1) {
            val h = hash(gridX, gridZ)
            if (h and 1 == 0) continue
            val trunkX = gridX * 7 + 2 + (h ushr 1 and 3)
            val trunkZ = gridZ * 7 + 2 + (h ushr 3 and 3)
            val base = surface(trunkX, trunkZ) + 1
            val top = base + 4 + (h ushr 5 and 1)
            if (x == trunkX && z == trunkZ && y in base until top) return log
            val dx = x - trunkX
            val dz = z - trunkZ
            val radius = if (y >= top) 1 else 2
            if (y in top - 2..top && dx * dx + dz * dz <= radius * radius + 1 && !(dx == 0 && dz == 0 && y < top)) return leaves
        }
        return null
    }

    fun block(x: Int, y: Int, z: Int): Long {
        val surface = surface(x, z)
        return when {
            y == minY -> bedrock
            y < surface - 3 -> stone
            y < surface -> dirt
            y == surface -> grass
            else -> treeBlock(x, y, z) ?: air
        }
    }

    /** The snapshot of Section `(sectionX, sectionY, sectionZ)` in dimension index 0, at revision 1. */
    fun snapshot(sectionX: Int, sectionY: Int, sectionZ: Int): SectionSnapshot {
        val cells = LongArray(4096)
        val sky = ByteArray(2048)
        for (localY in 0 until 16) for (localZ in 0 until 16) for (localX in 0 until 16) {
            val x = sectionX * 16 + localX
            val y = sectionY * 16 + localY
            val z = sectionZ * 16 + localZ
            val cell = cellIndex(localX, localY, localZ)
            cells[cell] = block(x, y, z)
            // Open sky above the ground and the canopy; dark below. Not vanilla light propagation.
            val skyLevel = if (y > surface(x, z) + 7 || (y > surface(x, z) && cells[cell] == air)) 15 else 0
            sky[cell / 2] = (sky[cell / 2].toInt() or (skyLevel shl (cell % 2 * 4))).toByte()
        }
        val biomes = LongArray(64) { biomeCell ->
            val x = sectionX * 16 + (biomeCell % 4) * 4
            if (floor(x / 48.0).toInt() % 2 == 0) plains else forest
        }
        return SectionSnapshot(
            pos = SectionPos(0, sectionX, sectionY, sectionZ),
            revision = 1,
            lightRevision = 1,
            blocks = Paletted.fromCells(cells),
            biomes = Paletted.fromCells(biomes),
            skyLight = LightLayer.Data(sky),
            blockLight = LightLayer.Data(ByteArray(2048)),
        )
    }
}
