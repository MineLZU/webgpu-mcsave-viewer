package com.nts.viewer.gateway

import io.ktor.server.cio.CIO
import io.ktor.server.engine.embeddedServer
import kotlinx.io.files.Path

/**
 * `viewer-gateway --content <export dir> [--web <built web-client dir>] [--host 127.0.0.1] [--port 8765]`.
 * Binds to the loopback address unless told otherwise: this gateway has no authentication yet.
 */
fun main(args: Array<String>) {
    val options = args.toList().chunked(2).associate { pair ->
        require(pair.size == 2 && pair[0].startsWith("--")) { "Arguments are --name value pairs; got ${pair.joinToString(" ")}" }
        pair[0] to pair[1]
    }
    val content = ReleaseContent.load(Path(options["--content"] ?: error("--content <export dir> is required")))
    val config = GatewayConfig(content, SyntheticWorld(content), options["--web"]?.let(::Path))
    val host = options["--host"] ?: "127.0.0.1"
    val port = options["--port"]?.toInt() ?: 8765
    println("viewer-gateway: ${content.gameVersion}, ${content.blockStateCount} block states, ${content.biomeIds.size} biomes; http://$host:$port/")
    embeddedServer(CIO, port = port, host = host) { gateway(config) }.start(wait = true)
}
