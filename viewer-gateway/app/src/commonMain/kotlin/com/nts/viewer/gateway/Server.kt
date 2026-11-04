package com.nts.viewer.gateway

import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.server.application.Application
import io.ktor.server.application.install
import io.ktor.server.response.header
import io.ktor.server.response.respondBytes
import io.ktor.server.routing.get
import io.ktor.server.routing.routing
import io.ktor.server.websocket.WebSockets
import io.ktor.server.websocket.webSocket
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import kotlin.time.Duration.Companion.seconds

/** What one gateway serves: the release export, the terrain source, and optionally the built web client. */
class GatewayConfig(val content: ReleaseContent, val world: SyntheticWorld, val webDirectory: Path?)

private fun contentTypeOf(name: String): ContentType = when (name.substringAfterLast('.', "").lowercase()) {
    "html" -> ContentType.Text.Html
    "js", "mjs" -> ContentType.Application.JavaScript
    "css" -> ContentType.Text.CSS
    "json" -> ContentType.Application.Json
    "wasm" -> ContentType("application", "wasm")
    "png" -> ContentType.Image.PNG
    "svg" -> ContentType.Image.SVG
    else -> ContentType.Application.OctetStream
}

/**
 * `GET /content/{sha256}` serves the dictionary or asset pack by hash, immutable; `/viewer` is the browser's
 * display-protocol WebSocket; every other `GET` is a file of the web client, `/` being `index.html`.
 */
fun Application.gateway(config: GatewayConfig) {
    install(WebSockets) {
        // `pingPeriod: Duration` is a JVM-only extension in Ktor 3; common code sets the millisecond field.
        pingPeriodMillis = 20.seconds.inWholeMilliseconds
        maxFrameSize = 1L shl 20
    }
    val byHash = listOf(config.content.dictionary, config.content.assets).associateBy { it.sha256 }
    routing {
        get("/content/{hash}") {
            val contentFile = byHash[call.parameters["hash"]?.lowercase()]
                ?: return@get call.respondBytes(ByteArray(0), status = HttpStatusCode.NotFound)
            call.response.header(HttpHeaders.CacheControl, "public, max-age=31536000, immutable")
            call.respondBytes(contentFile.bytes, ContentType.parse(contentFile.contentType))
        }
        webSocket("/viewer") { ViewerSession(this, config).run() }
        get("/{path...}") {
            val segments = call.parameters.getAll("path").orEmpty().filter { it.isNotEmpty() }
            val webDirectory = config.webDirectory
            // Segments arrive percent-decoded, so `..%2F..%2Fx` is one segment holding separators: reject any
            // separator, drive colon or dot segment inside a segment, not only a segment that is exactly `..`.
            val escapes = segments.any { segment ->
                segment == "." || segment == ".." || segment.any { it == '/' || it == '\\' || it == ':' || it == '\u0000' }
            }
            if (webDirectory == null || escapes) {
                return@get call.respondBytes(ByteArray(0), status = HttpStatusCode.NotFound)
            }
            val file = segments.ifEmpty { listOf("index.html") }.fold(webDirectory) { parent, segment -> Path(parent, segment) }
            val metadata = SystemFileSystem.metadataOrNull(file)
            if (metadata == null || !metadata.isRegularFile) {
                return@get call.respondBytes(ByteArray(0), status = HttpStatusCode.NotFound)
            }
            call.respondBytes(file.readBytes(), contentTypeOf(file.name))
        }
    }
}
