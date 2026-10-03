package org.hackeros.blueconnect.core

import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.NetworkInterface
import java.net.SocketTimeoutException

/**
 * UDP side of Blue Connect.
 *
 *  * The listener (bound to the discovery port) records every identity it
 *    hears and answers with our own identity, unicast to the sender's source
 *    port — exactly what the desktop's `bc_start_discovery` waits for (it
 *    broadcasts from an ephemeral port and reads the replies).
 *  * A scan broadcasts our identity from an ephemeral port and collects the
 *    replies of other Blue Connect clients.
 *
 * Reply storms are avoided by never answering a datagram whose source port is
 * the discovery port itself (that is another listener's reply, not a scan).
 */
internal class Discovery(
    private val opt: ConnectOptions,
    private val ownId: () -> String,
    private val ownIdentity: () -> ByteArray,
    private val onSeen: (Protocol.Identity, String) -> Unit,
    private val log: (String) -> Unit,
) {
    @Volatile private var listener: DatagramSocket? = null
    @Volatile private var running = false

    fun startListener() {
        val socket = try {
            DatagramSocket(null).apply {
                reuseAddress = true
                broadcast = true
                bind(InetSocketAddress(opt.bindAddress, opt.discoveryPort))
            }
        } catch (e: Exception) {
            log("Discovery listener could not start on UDP ${opt.discoveryPort}: ${e.message} " +
                "(another KDE Connect / Blue Connect instance?). Scanning still works.")
            return
        }
        listener = socket
        running = true
        Thread({
            val buf = ByteArray(4096)
            while (running) {
                try {
                    val p = DatagramPacket(buf, buf.size)
                    socket.receive(p)
                    val id = Protocol.parseIdentity(buf, p.length) ?: continue
                    if (id.deviceId == ownId()) continue
                    onSeen(id, p.address.hostAddress ?: continue)
                    if (p.port != opt.discoveryPort) {
                        val me = ownIdentity()
                        socket.send(DatagramPacket(me, me.size, p.address, p.port))
                    }
                } catch (e: Exception) {
                    if (!running) break
                }
            }
        }, "blue-connect-udp").apply { isDaemon = true }.start()
    }

    private fun targets(): List<InetAddress> {
        opt.discoveryTargets?.let { if (it.isNotEmpty()) return it }
        val list = linkedSetOf<InetAddress>(InetAddress.getByName("255.255.255.255"))
        try {
            for (nic in NetworkInterface.getNetworkInterfaces()) {
                if (!nic.isUp || nic.isLoopback) continue
                for (ia in nic.interfaceAddresses) {
                    val bc = ia.broadcast
                    if (bc is Inet4Address) list.add(bc)
                }
            }
        } catch (e: Exception) {
            // limited broadcast alone still works on most LANs
        }
        return list.toList()
    }

    /** Broadcast our identity and gather replies for [durationMs]. Blocking. */
    fun scan(durationMs: Long) {
        DatagramSocket(null).use { udp ->
            udp.broadcast = true
            udp.bind(InetSocketAddress(opt.bindAddress, 0))
            val me = ownIdentity()
            for (t in targets()) {
                try {
                    udp.send(DatagramPacket(me, me.size, t, opt.scanTargetPort ?: opt.discoveryPort))
                } catch (e: Exception) {
                    log("Broadcast to ${t.hostAddress} failed: ${e.message}")
                }
            }
            val deadline = System.currentTimeMillis() + durationMs
            val buf = ByteArray(4096)
            while (true) {
                val left = deadline - System.currentTimeMillis()
                if (left <= 0) break
                try {
                    udp.soTimeout = left.toInt().coerceAtLeast(1)
                    val p = DatagramPacket(buf, buf.size)
                    udp.receive(p)
                    val id = Protocol.parseIdentity(buf, p.length)
                    if (id != null && id.deviceId != ownId()) onSeen(id, p.address.hostAddress ?: continue)
                } catch (e: SocketTimeoutException) {
                    break
                } catch (e: Exception) {
                    // keep listening until the window ends
                }
            }
        }
    }

    fun stop() {
        running = false
        listener?.close()
    }
}
