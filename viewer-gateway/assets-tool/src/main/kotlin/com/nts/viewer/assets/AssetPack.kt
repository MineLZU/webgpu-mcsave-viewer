package com.nts.viewer.assets

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.attribute.FileTime
import java.util.zip.ZipEntry
import java.util.zip.ZipFile
import java.util.zip.ZipOutputStream

/**
 * The client-JAR files the browser renderer reads: block states, block models, block textures with their
 * animation metadata, and the biome colormaps. Paths keep their `assets/minecraft/...` form.
 */
private val PACK_PREFIXES = listOf(
    "assets/minecraft/blockstates/",
    "assets/minecraft/models/block/",
    "assets/minecraft/textures/block/",
    "assets/minecraft/textures/colormap/",
)

/** The entry timestamp of every pack entry, so the same client JAR always yields the same bytes and hash. */
private val FIXED_TIME: FileTime = FileTime.fromMillis(315_532_800_000L)

/**
 * Copies the [PACK_PREFIXES] entries of [clientJar] into [target], sorted by path with fixed timestamps.
 * Returns the number of entries written.
 */
fun writeAssetPack(clientJar: Path, target: Path): Int = ZipFile(clientJar.toFile()).use { zipFile ->
    val entries = zipFile.entries().asSequence()
        .filter { zipEntry -> !zipEntry.isDirectory && PACK_PREFIXES.any { zipEntry.name.startsWith(it) } }
        .sortedBy { it.name }
        .toList()
    check(entries.isNotEmpty()) { "The client JAR has none of $PACK_PREFIXES" }
    ZipOutputStream(Files.newOutputStream(target)).use { output ->
        output.setLevel(9)
        for (zipEntry in entries) {
            val bytes = zipFile.getInputStream(zipEntry).use { it.readBytes() }
            output.putNextEntry(ZipEntry(zipEntry.name).apply {
                lastModifiedTime = FIXED_TIME
                creationTime = FIXED_TIME
                lastAccessTime = FIXED_TIME
            })
            output.write(bytes)
            output.closeEntry()
        }
    }
    entries.size
}
