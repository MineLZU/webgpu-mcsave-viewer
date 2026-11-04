package com.nts.viewer.protocol

/**
 * Cell values for one Section: state IDs for its 4096 blocks or biome IDs for its 64 samples.
 * [Single] is the only form of a uniform container; [Indexed] has at least two distinct palette values.
 */
sealed interface Paletted {
    /** Every distinct value. */
    val values: LongArray

    operator fun get(cell: Int): Long

    data class Single(val value: Long) : Paletted {
        override val values: LongArray get() = longArrayOf(value)
        override fun get(cell: Int): Long = value
    }

    class Indexed(val palette: LongArray, val indices: IntArray) : Paletted {
        override val values: LongArray get() = palette
        override fun get(cell: Int): Long = palette[indices[cell]]

        override fun equals(other: Any?): Boolean =
            other is Indexed && palette.contentEquals(other.palette) && indices.contentEquals(other.indices)

        override fun hashCode(): Int = 31 * palette.contentHashCode() + indices.contentHashCode()
    }

    companion object {
        /** The canonical container for [cells]: single when uniform, otherwise a first-seen palette. */
        fun fromCells(cells: LongArray): Paletted {
            val positions = HashMap<Long, Int>()
            val palette = ArrayList<Long>()
            val indices = IntArray(cells.size) { cell ->
                positions.getOrPut(cells[cell]) { palette.add(cells[cell]); palette.size - 1 }
            }
            return if (palette.size == 1) Single(palette[0]) else Indexed(palette.toLongArray(), indices)
        }

        /** The narrowest index width that addresses [paletteSize] entries; at least 1. */
        fun bitsForPalette(paletteSize: Int): Int {
            var bits = 1
            while ((1 shl bits) < paletteSize) bits++
            return bits
        }
    }
}
