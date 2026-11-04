package com.nts.viewer.gateway

import com.nts.viewer.protocol.ChunkLifecycle
import com.nts.viewer.protocol.ChunkState
import com.nts.viewer.protocol.Dimension
import com.nts.viewer.protocol.Hello
import com.nts.viewer.protocol.Message
import com.nts.viewer.protocol.ProtocolCodec
import com.nts.viewer.protocol.ProtocolException
import com.nts.viewer.protocol.ResyncRequest
import com.nts.viewer.protocol.SessionBounds
import com.nts.viewer.protocol.Subscribe
import io.ktor.server.websocket.DefaultWebSocketServerSession
import io.ktor.websocket.CloseReason
import io.ktor.websocket.Frame
import io.ktor.websocket.close
import io.ktor.websocket.readBytes
import kotlin.random.Random

/** The largest number of chunk columns one subscription may cover. */
const val MAX_SUBSCRIBED_COLUMNS = 33 * 33

/**
 * One browser connection: sends `Hello`, then answers each `Subscribe` with the lifecycle and every Section
 * snapshot of the requested columns, and each `ResyncRequest` with fresh snapshots. Malformed or out-of-session
 * messages close the connection; a client must reconnect and start from a new `Hello`.
 */
class ViewerSession(private val socket: DefaultWebSocketServerSession, private val config: GatewayConfig) {
    private val hello = Hello(
        protocolVersion = ProtocolCodec.PROTOCOL_VERSION,
        sessionId = Random.nextLong(),
        gameVersion = config.content.gameVersion,
        dictionaryHash = config.content.dictionary.sha256.hexToByteArray(),
        assetsHash = config.content.assets.sha256.hexToByteArray(),
        blockStateCount = config.content.blockStateCount.toLong(),
        biomeCount = config.content.biomeIds.size.toLong(),
        dimensions = listOf(Dimension("minecraft:overworld", config.world.minY, config.world.height.toLong())),
    )
    private val sessionBounds = SessionBounds(hello)
    private var subscription: Subscribe? = null

    private suspend fun send(message: Message) = socket.send(Frame.Binary(true, ProtocolCodec.encode(message)))

    suspend fun run() {
        send(hello)
        for (frame in socket.incoming) {
            if (frame !is Frame.Binary) {
                return socket.close(CloseReason(CloseReason.Codes.CANNOT_ACCEPT, "display protocol frames are binary"))
            }
            val message = try {
                ProtocolCodec.decode(frame.readBytes()).also(sessionBounds::check)
            } catch (exception: ProtocolException) {
                return socket.close(CloseReason(CloseReason.Codes.PROTOCOL_ERROR, exception.message.orEmpty().take(120)))
            }
            when (message) {
                is Subscribe -> subscribe(message)
                is ResyncRequest.Subscription -> subscription?.takeIf { it.generation == message.generation }?.let { sendColumns(it) }
                is ResyncRequest.Section -> send(config.world.snapshot(message.pos.x, message.pos.y, message.pos.z))
                else -> return socket.close(CloseReason(CloseReason.Codes.CANNOT_ACCEPT, "a browser sends only Subscribe and ResyncRequest"))
            }
        }
    }

    /**
     * Replaces the interest. Columns that leave it get no message: the browser owns eviction of what it no longer
     * asks for. A request covering more than [MAX_SUBSCRIBED_COLUMNS] closes the connection.
     */
    private suspend fun subscribe(subscribe: Subscribe) {
        val columns = subscribe.ranges.sumOf { (it.maxX - it.minX + 1).toLong() * (it.maxZ - it.minZ + 1) }
        if (columns > MAX_SUBSCRIBED_COLUMNS) {
            return socket.close(CloseReason(CloseReason.Codes.CANNOT_ACCEPT, "subscription of $columns columns exceeds $MAX_SUBSCRIBED_COLUMNS"))
        }
        val previous = subscription
        if (previous != null && subscribe.generation.toUInt() <= previous.generation.toUInt()) return
        subscription = subscribe
        sendColumns(subscribe)
    }

    /** Every column of [subscribe], nearest to the range centres first, so the view fills from the middle. */
    private suspend fun sendColumns(subscribe: Subscribe) {
        val columns = subscribe.ranges.flatMap { range ->
            val centreX = (range.minX + range.maxX) / 2
            val centreZ = (range.minZ + range.maxZ) / 2
            (range.minX..range.maxX).flatMap { x -> (range.minZ..range.maxZ).map { z -> Triple(x, z, maxOf(kotlin.math.abs(x - centreX), kotlin.math.abs(z - centreZ))) } }
        }.distinctBy { it.first to it.second }.sortedBy { it.third }
        for ((x, z) in columns) {
            if (subscription !== subscribe) return
            send(ChunkLifecycle(subscribe.dimension, x, z, ChunkState.HOT))
            for (sectionY in config.world.sectionYs) send(config.world.snapshot(x, sectionY, z))
        }
    }
}
