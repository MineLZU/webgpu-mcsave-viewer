package com.nts.viewer.protocol

/**
 * Why a display-protocol message could not be decoded, encoded, or accepted by a session. [kind] is the
 * category shared with the Rust codec and the sample manifest.
 */
sealed class ProtocolException(val kind: String, message: String) : Exception(message) {
    class Truncated(offset: Int, needed: Int) :
        ProtocolException("truncated", "message truncated: $needed more bytes needed at offset $offset")

    class TrailingBytes(count: Int) : ProtocolException("trailing_bytes", "$count trailing bytes after the message")

    class UnknownMessageType(tag: Int) : ProtocolException("unknown_type", "unknown message type $tag")

    class Invalid(reason: String) : ProtocolException("invalid", "invalid message: $reason")

    class OutOfBounds(reason: String) : ProtocolException("out_of_bounds", "out of session bounds: $reason")
}

/** A little-endian cursor over one message; [finish] rejects leftover bytes. */
internal class Reader(private val bytes: ByteArray) {
    private var offset = 0

    fun take(count: Int): ByteArray {
        val remaining = bytes.size - offset
        if (count > remaining) throw ProtocolException.Truncated(offset, count - remaining)
        return bytes.copyOfRange(offset, offset + count).also { offset += count }
    }

    private fun little(count: Int): Long {
        val slice = take(count)
        return (0 until count).fold(0L) { value, index -> value or ((slice[index].toLong() and 0xff) shl (8 * index)) }
    }

    fun u8(): Int = little(1).toInt()
    fun u16(): Int = little(2).toInt()
    fun u32(): Long = little(4)
    fun i32(): Int = little(4).toInt()
    fun u64(): Long = little(8)

    fun string(): String {
        val bytes = take(u16())
        return try {
            bytes.decodeToString(throwOnInvalidSequence = true)
        } catch (_: CharacterCodingException) {
            throw ProtocolException.Invalid("string is not UTF-8")
        }
    }

    fun finish() {
        val count = bytes.size - offset
        if (count != 0) throw ProtocolException.TrailingBytes(count)
    }
}

/** The encoding counterpart of [Reader]. */
internal class Writer {
    private var buffer = ByteArray(256)
    private var size = 0

    fun bytes(bytes: ByteArray) {
        if (size + bytes.size > buffer.size) buffer = buffer.copyOf(maxOf(buffer.size * 2, size + bytes.size))
        bytes.copyInto(buffer, size)
        size += bytes.size
    }

    private fun little(value: Long, count: Int) = bytes(ByteArray(count) { index -> (value ushr (8 * index)).toByte() })

    fun u8(value: Int) = little(value.toLong(), 1)
    fun u16(value: Int) = little(value.toLong(), 2)
    fun u32(value: Long) = little(value, 4)
    fun i32(value: Int) = little(value.toLong(), 4)
    fun u64(value: Long) = little(value, 8)

    fun string(value: String) {
        val encoded = value.encodeToByteArray()
        if (encoded.size > 0xffff) throw ProtocolException.Invalid("string longer than 65535 bytes")
        u16(encoded.size)
        bytes(encoded)
    }

    fun toByteArray(): ByteArray = buffer.copyOf(size)
}
