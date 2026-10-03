package org.hackeros.blueconnect.core

import java.io.InputStream

/**
 * Reads newline-terminated JSON packets from a stream. Keeps leftover bytes
 * between calls (a peer may send several packets back to back) and tolerates a
 * final packet that is not newline terminated before EOF. Blocking; read
 * timeouts come from the socket's `soTimeout` (a SocketTimeoutException
 * propagates to the caller).
 */
class PacketReader(private val input: InputStream) {
    private var pending = ByteArray(8192)
    private var len = 0
    private var eof = false
    private val chunk = ByteArray(4096)

    /** @return the next packet as raw JSON text, or null at end of stream. */
    fun readLine(): String? {
        while (true) {
            val nl = indexOfNewline()
            if (nl >= 0) {
                val line = String(pending, 0, nl, Charsets.UTF_8)
                consume(nl + 1)
                if (line.isBlank()) continue // blank keep-alive line
                return line
            }
            if (eof) {
                if (len == 0) return null
                val rest = String(pending, 0, len, Charsets.UTF_8)
                len = 0
                return if (rest.isBlank()) null else rest
            }
            if (len > Protocol.MAX_PACKET_BYTES) throw java.io.IOException("Packet too large")
            val n = input.read(chunk)
            if (n < 0) eof = true else append(n)
        }
    }

    private fun indexOfNewline(): Int {
        for (i in 0 until len) if (pending[i] == '\n'.code.toByte()) return i
        return -1
    }

    private fun append(n: Int) {
        if (len + n > pending.size) pending = pending.copyOf(maxOf(pending.size * 2, len + n))
        System.arraycopy(chunk, 0, pending, len, n)
        len += n
    }

    private fun consume(count: Int) {
        System.arraycopy(pending, count, pending, 0, len - count)
        len -= count
    }
}
