package org.hackeros.blueconnect.core

import org.json.JSONArray
import org.json.JSONObject

/**
 * Wire constants + packet helpers. Mirrors the desktop implementation in
 * `src-tauri/src/BlueConnect/mod.rs` exactly — if you change anything here,
 * that file (and the Windows/macOS clients) must change with it.
 *
 * Transport summary:
 *  * Discovery: one UDP datagram holding a `kdeconnect.identity` JSON document
 *    (no trailing newline), port 1716.
 *  * Everything else: mutual TLS over TCP (port from the identity's tcpPort,
 *    1717 by default), newline-terminated JSON packets. Both sides present a
 *    self-signed certificate; trust is decided by pinning the certificate's
 *    SHA-256 fingerprint after a Short-Authentication-String confirmation.
 */
object Protocol {
    const val DISCOVERY_PORT = 1716
    const val PAIRING_TCP_PORT = 1717
    const val PROTOCOL_VERSION = 7

    const val TYPE_IDENTITY = "kdeconnect.identity"
    const val TYPE_PAIR = "kdeconnect.pair"
    const val TYPE_PING = "kdeconnect.ping"
    const val TYPE_SMS_REQUEST = "kdeconnect.sms.request"
    const val TYPE_SMS_MESSAGES = "kdeconnect.sms.messages"

    /** How long either side waits for the other person to answer a pairing request. */
    const val PAIRING_DECISION_TIMEOUT_MS = 120_000L

    /** Upper bound for one packet we are willing to buffer. */
    const val MAX_PACKET_BYTES = 1024 * 1024

    /**
     * Replies sent to the DESKTOP must fit into one TLS record: the desktop
     * reads the answer with a single `read()` into a 64 KiB buffer and parses
     * whatever that call returned as one JSON document, so a reply spanning
     * several TLS records (16 KiB each) could arrive truncated.
     */
    const val MAX_SINGLE_READ_REPLY_BYTES = 14_000

    val DEVICE_TYPES = listOf("phone", "tablet", "desktop", "laptop", "tv", "unknown")

    fun normalizeDeviceType(t: String?): String {
        val v = (t ?: "").trim().lowercase()
        return if (v in DEVICE_TYPES) v else "unknown"
    }

    /** Builds the discovery datagram (no trailing newline — matches the desktop). */
    fun buildIdentity(deviceId: String, deviceName: String, deviceType: String, tcpPort: Int): ByteArray {
        val body = JSONObject()
            .put("deviceId", deviceId)
            .put("deviceName", deviceName)
            .put("deviceType", normalizeDeviceType(deviceType))
            .put("protocolVersion", PROTOCOL_VERSION)
            .put("tcpPort", tcpPort)
            // Extra fields: ignored by the desktop (serde skips unknown keys),
            // but let real KDE Connect negotiate plugins.
            .put("incomingCapabilities", JSONArray(listOf(TYPE_PAIR, TYPE_PING, TYPE_SMS_REQUEST)))
            .put("outgoingCapabilities", JSONArray(listOf(TYPE_PAIR, TYPE_PING, TYPE_SMS_MESSAGES)))
        val packet = JSONObject().put("type", TYPE_IDENTITY).put("body", body)
        return packet.toString().toByteArray(Charsets.UTF_8)
    }

    /** One framed packet: compact JSON + '\n'. Always written with a single write call. */
    fun frame(type: String, body: JSONObject): ByteArray =
        (JSONObject().put("type", type).put("body", body).toString() + "\n").toByteArray(Charsets.UTF_8)

    fun pairRequest(deviceId: String): ByteArray =
        frame(TYPE_PAIR, JSONObject().put("pair", true).put("deviceId", deviceId))

    fun pairResponse(accepted: Boolean): ByteArray =
        frame(TYPE_PAIR, JSONObject().put("pair", accepted))

    data class Packet(val type: String, val body: JSONObject)

    /** Parses one JSON packet; returns null for anything that is not a packet object. */
    fun parse(json: String): Packet? = try {
        val root = JSONObject(json)
        val type = root.optString("type", "")
        if (type.isEmpty()) null else Packet(type, root.optJSONObject("body") ?: JSONObject())
    } catch (e: Exception) {
        null
    }

    data class Identity(
        val deviceId: String,
        val deviceName: String,
        val deviceType: String,
        val protocolVersion: Int,
        val tcpPort: Int,
    )

    /**
     * Parses a discovery datagram. Same strictness as the desktop's serde
     * struct: every field is mandatory, otherwise the datagram is ignored.
     */
    fun parseIdentity(datagram: ByteArray, length: Int = datagram.size): Identity? {
        val p = parse(String(datagram, 0, length, Charsets.UTF_8)) ?: return null
        if (p.type != TYPE_IDENTITY) return null
        val b = p.body
        val id = b.optString("deviceId", "")
        if (id.isBlank() || !b.has("deviceName") || !b.has("deviceType") || !b.has("protocolVersion") || !b.has("tcpPort")) return null
        val port = b.optInt("tcpPort", -1)
        if (port < 1 || port > 65535) return null
        return Identity(
            deviceId = id,
            deviceName = b.optString("deviceName", ""),
            deviceType = normalizeDeviceType(b.optString("deviceType", "")),
            protocolVersion = b.optInt("protocolVersion", PROTOCOL_VERSION),
            tcpPort = port,
        )
    }
}
