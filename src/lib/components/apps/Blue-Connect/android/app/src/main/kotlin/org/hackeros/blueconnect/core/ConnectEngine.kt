package org.hackeros.blueconnect.core

import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.OutputStream
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.security.SecureRandom
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLServerSocket
import javax.net.ssl.SSLSocket

class ConnectException(message: String, cause: Throwable? = null) : Exception(message, cause)

/** A pairing request from another device, waiting for this person's yes/no. */
data class IncomingPairingRequest(val deviceId: String, val deviceName: String, val address: String, val sas: String)

/** Shown while WE are the one asking: the code to compare with the other screen. */
data class OutgoingPairingSas(val deviceId: String, val deviceName: String, val sas: String)

data class SmsMessage(val address: String, val body: String)

/** The person's answer to an incoming pairing request. Call exactly one of accept()/reject(). */
class PairingDecision {
    private val latch = CountDownLatch(1)
    @Volatile private var result = false

    fun accept() { result = true; latch.countDown() }
    fun reject() { result = false; latch.countDown() }

    /** Waits for the answer; no answer in time counts as "no" (never auto-accept). */
    internal fun await(timeoutMs: Long): Boolean =
        try { latch.await(timeoutMs, TimeUnit.MILLISECONDS) && result } catch (e: InterruptedException) { false }
}

/** UI hooks. Every method may be called on a background thread. */
interface ConnectListener {
    fun onDevicesChanged() {}
    /** Show the SAS and ask the person; the engine waits for [decision] (max 2 minutes). */
    fun onIncomingPairing(request: IncomingPairingRequest, decision: PairingDecision) { decision.reject() }
    fun onOutgoingPairingSas(info: OutgoingPairingSas) {}
    fun onPing(device: DeviceInfo, message: String?) {}
    fun onLog(message: String) {}
}

/** Plugin hook for packets (other than pairing/ping) from a PAIRED device. [reply] writes one framed packet. */
fun interface PacketHandler {
    fun handle(device: DeviceInfo, packet: Protocol.Packet, reply: (ByteArray) -> Unit)
}

/**
 * The whole Blue Connect client: device registry, UDP discovery, the
 * mutual-TLS listener (incoming pairing + packets from paired devices) and the
 * client side (pair with a device, ping, SMS relay). Platform-neutral — the
 * Android layer only supplies the TLS identity and the UI.
 */
class ConnectEngine(
    private val opt: ConnectOptions,
    private val identity: TlsIdentity,
    private val listener: ConnectListener,
) {
    private val store = DeviceStore(opt.dataDir)
    private val sslContext = TlsSupport.context(identity)
    private val pool = Executors.newCachedThreadPool { r -> Thread(r, "blue-connect-io").apply { isDaemon = true } }
    private val confirmGate = Any()
    private val discovery: Discovery
    @Volatile private var server: SSLServerSocket? = null
    @Volatile private var running = false

    val deviceId: String = loadOrCreateDeviceId(opt.dataDir)
    val deviceName: String get() = opt.deviceName
    val fingerprint: String get() = identity.fingerprint
    val tcpPort: Int get() = opt.tcpPort

    @Volatile var packetHandler: PacketHandler? = null

    init {
        discovery = Discovery(
            opt, { deviceId }, { Protocol.buildIdentity(deviceId, opt.deviceName, opt.deviceType, opt.tcpPort) },
            { id, from -> store.seen(id.deviceId, id.deviceName, id.deviceType, from, id.tcpPort); listener.onDevicesChanged() },
            { listener.onLog(it) },
        )
    }

    private fun loadOrCreateDeviceId(dir: File): String {
        dir.mkdirs()
        val f = File(dir, "device_id")
        try {
            if (f.exists()) f.readText().trim().takeIf { it.isNotEmpty() }?.let { return it }
        } catch (e: Exception) { /* regenerate */ }
        val bytes = ByteArray(10).also { SecureRandom().nextBytes(it) }
        val id = "blue-connect-" + bytes.joinToString("") { "%02x".format(it.toInt() and 0xFF) }
        f.writeText(id)
        return id
    }

    fun devices(): List<DeviceInfo> = store.all()

    // ── Lifecycle ────────────────────────────────────────────────────────

    /** Starts the TLS listener and the discovery listener. Throws if the TCP port is taken. */
    @Synchronized
    fun start() {
        if (running) return
        val s = sslContext.serverSocketFactory.createServerSocket(opt.tcpPort, 50, opt.bindAddress) as SSLServerSocket
        s.needClientAuth = true // mutual TLS: no client certificate, no connection
        s.reuseAddress = true
        server = s
        running = true
        pool.execute { acceptLoop(s) }
        discovery.startListener()
        listener.onLog("Blue Connect listening: TCP ${opt.tcpPort}, UDP ${opt.discoveryPort} as \"${opt.deviceName}\" ($deviceId)")
    }

    @Synchronized
    fun stop() {
        running = false
        try { server?.close() } catch (e: Exception) { /* shutting down */ }
        discovery.stop()
        pool.shutdownNow()
    }

    val isRunning: Boolean get() = running

    // ── Discovery ────────────────────────────────────────────────────────

    /** Broadcasts and listens for replies for [durationMs], then returns the known devices. Blocking. */
    fun scan(durationMs: Long = 3000): List<DeviceInfo> {
        discovery.scan(durationMs)
        return store.all()
    }

    fun forget(deviceId: String) {
        if (store.forget(deviceId)) listener.onDevicesChanged()
    }

    // ── Incoming connections ─────────────────────────────────────────────

    private fun acceptLoop(s: SSLServerSocket) {
        while (running) {
            val sock = try { s.accept() as SSLSocket } catch (e: Exception) { if (!running) return else continue }
            pool.execute { handleConnection(sock) }
        }
    }

    private fun handleConnection(sock: SSLSocket) {
        val ip = (sock.remoteSocketAddress as? InetSocketAddress)?.address?.hostAddress ?: "unknown"
        try {
            sock.use {
                sock.soTimeout = HANDSHAKE_TIMEOUT_MS
                sock.startHandshake()
                val peerFp = TlsSupport.peerFingerprint(sock.session) ?: return

                val reader = PacketReader(sock.inputStream)
                sock.soTimeout = FIRST_PACKET_TIMEOUT_MS
                val first = reader.readLine()?.let { Protocol.parse(it) } ?: return
                val out = sock.outputStream

                if (first.type == Protocol.TYPE_PAIR) {
                    handlePairPacket(out, first, peerFp, ip)
                    return
                }

                // Any other packet: only from a device we paired (certificate pinned).
                val dev = store.findPairedByFingerprint(peerFp)
                if (dev == null) {
                    listener.onLog("Ignored a '${first.type}' packet from $ip: not a paired device")
                    return
                }
                store.updateAddress(dev.id, ip)

                var pkt: Protocol.Packet? = first
                while (pkt != null) {
                    dispatch(dev, pkt, out)
                    sock.soTimeout = FOLLOWUP_TIMEOUT_MS
                    pkt = try { reader.readLine()?.let { Protocol.parse(it) } } catch (e: SocketTimeoutException) { null }
                }
            }
        } catch (e: Exception) {
            if (running) listener.onLog("Connection from $ip failed: ${e.message}")
        }
    }

    private fun dispatch(dev: DeviceInfo, pkt: Protocol.Packet, out: OutputStream) {
        if (pkt.type == Protocol.TYPE_PING) {
            listener.onPing(dev, if (pkt.body.has("message")) pkt.body.optString("message") else null)
            return
        }
        packetHandler?.handle(dev, pkt) { bytes -> out.write(bytes); out.flush() }
    }

    private fun handlePairPacket(out: OutputStream, pkt: Protocol.Packet, peerFp: String, ip: String) {
        val wantsPair = pkt.body.optBoolean("pair", false)
        if (!wantsPair) {
            // The other side unpaired us: drop the pin so it can no longer talk to us.
            store.findPairedByFingerprint(peerFp)?.let {
                store.forget(it.id)
                listener.onLog("${it.name} unpaired this device")
                listener.onDevicesChanged()
            }
            return
        }

        val deviceId = pkt.body.optString("deviceId", "")
        if (deviceId.isEmpty()) {
            listener.onLog("Pairing packet from $ip is missing deviceId")
            return
        }

        val existing = store.get(deviceId)
        if (existing != null && existing.paired && existing.pinnedCertSha256 != null && existing.pinnedCertSha256 != peerFp) {
            // Same stance as the desktop: never silently re-pin a different key.
            listener.onLog("Refused pairing from ${existing.name}: certificate differs from the one pinned earlier. " +
                "Forget the device first if this is intentional (e.g. it was reset).")
            out.write(Protocol.pairResponse(false)); out.flush()
            return
        }

        val name = existing?.name ?: "Device @ $ip"
        val sas = Sas.compute(identity.fingerprint, peerFp)
        val accepted = synchronized(confirmGate) { // one dialog at a time
            val decision = PairingDecision()
            listener.onIncomingPairing(IncomingPairingRequest(deviceId, name, ip, sas), decision)
            decision.await(Protocol.PAIRING_DECISION_TIMEOUT_MS)
        }

        out.write(Protocol.pairResponse(accepted)); out.flush()
        if (!accepted) {
            listener.onLog("Pairing request from $name was declined or timed out")
            return
        }
        store.markPaired(deviceId, name, ip, existing?.tcpPort ?: Protocol.PAIRING_TCP_PORT, peerFp)
        listener.onLog("Paired with $name")
        listener.onDevicesChanged()
    }

    // ── Outgoing connections ─────────────────────────────────────────────

    private class Conn(val sock: SSLSocket, val peerFp: String) : AutoCloseable {
        override fun close() { try { sock.close() } catch (e: Exception) { /* closing */ } }
    }

    private fun connectTls(dev: DeviceInfo): Conn {
        val addr = try { InetAddress.getByName(dev.address) } catch (e: Exception) {
            throw ConnectException("Invalid address for ${dev.name}: \"${dev.address}\"")
        }
        val sock = sslContext.socketFactory.createSocket() as SSLSocket
        try {
            sock.connect(InetSocketAddress(addr, dev.tcpPort), CONNECT_TIMEOUT_MS)
            sock.soTimeout = HANDSHAKE_TIMEOUT_MS
            sock.startHandshake()
            val fp = TlsSupport.peerFingerprint(sock.session)
                ?: throw ConnectException("TLS handshake completed without a peer certificate")
            return Conn(sock, fp)
        } catch (e: ConnectException) {
            sock.close(); throw e
        } catch (e: SocketTimeoutException) {
            sock.close(); throw ConnectException("Timed out connecting to ${dev.name} (${dev.address}:${dev.tcpPort})", e)
        } catch (e: Exception) {
            sock.close(); throw ConnectException("Could not connect to ${dev.name} (${dev.address}:${dev.tcpPort}): ${e.message}", e)
        }
    }

    /**
     * Pairs with a discovered device (another Blue Connect client, or the
     * desktop while its "Listen" button is active). Reports the SAS through
     * [ConnectListener.onOutgoingPairingSas], then waits up to two minutes for
     * the other person to accept. Blocking.
     */
    fun pair(deviceId: String) {
        val dev = store.get(deviceId) ?: throw ConnectException("Unknown device — scan first")
        connectTls(dev).use { c ->
            if (dev.paired && dev.pinnedCertSha256 != null && dev.pinnedCertSha256 != c.peerFp) {
                throw ConnectException(
                    "Refusing to pair: ${dev.name} presented a different certificate than the one pinned when it was last paired. " +
                        "This can mean the device was reset, or that something is impersonating it — Forget the device and re-pair only if you are sure.",
                )
            }
            val sas = Sas.compute(identity.fingerprint, c.peerFp)
            listener.onOutgoingPairingSas(OutgoingPairingSas(dev.id, dev.name, sas))

            val out = c.sock.outputStream
            out.write(Protocol.pairRequest(this.deviceId)); out.flush() // single write = single TLS record

            c.sock.soTimeout = Protocol.PAIRING_DECISION_TIMEOUT_MS.toInt()
            val line = try { PacketReader(c.sock.inputStream).readLine() } catch (e: SocketTimeoutException) {
                throw ConnectException("${dev.name} did not respond to the pairing request in time")
            } ?: throw ConnectException("${dev.name} closed the connection without responding")

            val accepted = Protocol.parse(line)?.body?.optBoolean("pair", false) ?: false
            if (!accepted) throw ConnectException("Pairing was declined on ${dev.name}")

            store.markPaired(dev.id, dev.name, dev.address, dev.tcpPort, c.peerFp)
            listener.onLog("Paired with ${dev.name}")
            listener.onDevicesChanged()
        }
    }

    /** Opens a connection to a PAIRED device and verifies the pinned certificate. */
    private fun openAuthenticated(deviceId: String): Pair<Conn, DeviceInfo> {
        val dev = store.get(deviceId)
        val pinned = dev?.pinnedCertSha256
        if (dev == null || !dev.paired || pinned == null) throw ConnectException("That device is not paired — pair it first")
        val c = connectTls(dev)
        if (c.peerFp != pinned) {
            c.close()
            throw ConnectException(
                "Refusing to talk to ${dev.name}: it presented a different certificate than the one pinned when it was paired. " +
                    "Forget the device and re-pair only if you are sure it is really that device.",
            )
        }
        return c to dev
    }

    fun ping(deviceId: String, message: String? = null) {
        val (c, _) = openAuthenticated(deviceId)
        c.use {
            val body = JSONObject()
            if (!message.isNullOrEmpty()) body.put("message", message)
            c.sock.outputStream.apply { write(Protocol.frame(Protocol.TYPE_PING, body)); flush() }
        }
    }

    /** Asks a paired phone to send an SMS (phone must run Blue Connect for Android). */
    fun sendSms(deviceId: String, phoneNumber: String, messageBody: String) {
        val (c, _) = openAuthenticated(deviceId)
        c.use {
            val body = JSONObject().put("sendSms", true).put("phoneNumber", phoneNumber).put("messageBody", messageBody)
            c.sock.outputStream.apply { write(Protocol.frame(Protocol.TYPE_SMS_REQUEST, body)); flush() }
        }
    }

    /** Requests the phone's SMS history (address + body pairs). */
    fun requestSms(deviceId: String, timeoutMs: Int = 10_000): List<SmsMessage> {
        val (c, dev) = openAuthenticated(deviceId)
        c.use {
            val body = JSONObject().put("requestAllConversations", true)
            c.sock.outputStream.apply { write(Protocol.frame(Protocol.TYPE_SMS_REQUEST, body)); flush() }
            c.sock.soTimeout = timeoutMs
            val line = try { PacketReader(c.sock.inputStream).readLine() } catch (e: SocketTimeoutException) {
                throw ConnectException("${dev.name} did not reply with SMS history in time")
            } ?: throw ConnectException("${dev.name} closed the connection without replying")
            val arr: JSONArray = Protocol.parse(line)?.body?.optJSONArray("messages") ?: JSONArray()
            return (0 until arr.length()).mapNotNull { i ->
                val m = arr.optJSONObject(i) ?: return@mapNotNull null
                if (!m.has("body")) return@mapNotNull null
                SmsMessage(m.optString("address", ""), m.optString("body", ""))
            }
        }
    }

    private companion object {
        const val CONNECT_TIMEOUT_MS = 10_000
        const val HANDSHAKE_TIMEOUT_MS = 20_000
        const val FIRST_PACKET_TIMEOUT_MS = 30_000
        const val FOLLOWUP_TIMEOUT_MS = 10_000
    }
}
