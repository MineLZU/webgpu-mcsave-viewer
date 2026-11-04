package com.nts.viewer.assets

import com.hiczp.minecraft.distribution.metadata.MinecraftDistributionMetadataApi
import com.hiczp.minecraft.distribution.metadata.MinecraftDownload
import com.hiczp.minecraft.distribution.metadata.download
import io.ktor.client.statement.bodyAsChannel
import io.ktor.client.statement.bodyAsText
import io.ktor.utils.io.jvm.javaio.toInputStream
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.security.MessageDigest

/** The official client and server JARs of one explicitly named release, downloaded and SHA-1 verified. */
class OfficialFiles(val gameVersion: String, val clientJar: Path, val serverJar: Path)

/** Lowercase hex of [digest] over [path]'s bytes. */
fun Path.digest(algorithm: String): String {
    val messageDigest = MessageDigest.getInstance(algorithm)
    Files.newInputStream(this).use { input ->
        val buffer = ByteArray(1 shl 16)
        while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            messageDigest.update(buffer, 0, read)
        }
    }
    return messageDigest.digest().toHexString()
}

/**
 * Resolves [gameVersion] in Version Manifest V2 — never the "latest" release — and downloads its client and
 * server JARs into [cacheDirectory]. A cached file is reused only when its size and SHA-1 match the metadata.
 *
 * The version metadata is read raw rather than through `versionMetadata`: the fork models only the current
 * schema, whose `arguments` requires fields an older release's metadata lacks. Only `downloads.client` and
 * `downloads.server` are read, with the fork's own [MinecraftDownload] serializer.
 */
suspend fun MinecraftDistributionMetadataApi.officialFiles(gameVersion: String, cacheDirectory: Path): OfficialFiles {
    val minecraftVersionReference = versionManifest().versions.singleOrNull { it.id == gameVersion }
        ?: error("Version Manifest V2 has no release $gameVersion")
    val metadata = download(minecraftVersionReference.url).execute { httpResponse -> httpResponse.bodyAsText() }
    val downloads = Json.parseToJsonElement(metadata).jsonObject["downloads"]?.jsonObject
        ?: error("Version metadata of $gameVersion has no downloads")
    fun officialDownload(side: String): MinecraftDownload = Json { ignoreUnknownKeys = true }.decodeFromJsonElement(
        MinecraftDownload.serializer(),
        downloads[side] ?: error("Version metadata of $gameVersion has no $side download"),
    )
    val versionDirectory = cacheDirectory.resolve(gameVersion)
    Files.createDirectories(versionDirectory)
    return OfficialFiles(
        gameVersion,
        verifiedDownload(officialDownload("client"), versionDirectory.resolve("client.jar")),
        verifiedDownload(officialDownload("server"), versionDirectory.resolve("server.jar")),
    )
}

private fun Path.matches(minecraftDownload: MinecraftDownload): Boolean =
    Files.isRegularFile(this) && Files.size(this) == minecraftDownload.size && digest("SHA-1") == minecraftDownload.sha1.lowercase()

private suspend fun MinecraftDistributionMetadataApi.verifiedDownload(minecraftDownload: MinecraftDownload, target: Path): Path {
    if (target.matches(minecraftDownload)) return target
    val partial = target.resolveSibling("${target.fileName}.part")
    download(minecraftDownload).execute { httpResponse ->
        httpResponse.bodyAsChannel().toInputStream().use { input -> Files.copy(input, partial, StandardCopyOption.REPLACE_EXISTING) }
    }
    check(partial.matches(minecraftDownload)) { "Download of ${minecraftDownload.url} does not match its size and SHA-1" }
    Files.move(partial, target, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
    return target
}
