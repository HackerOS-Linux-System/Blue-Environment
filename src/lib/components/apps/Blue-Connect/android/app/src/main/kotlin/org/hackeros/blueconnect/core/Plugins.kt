package org.hackeros.blueconnect.core

import org.json.JSONArray
import org.json.JSONObject
import java.net.URI

data class BatteryStatus(val percent: Int, val charging: Boolean)
data class NotificationInfo(val id: String, val app: String, val title: String, val text: String, val timeMs: Long)

/**
 * What a platform provides so a paired device can use it: battery, "find my phone" ringing,
 * clipboard, opening a shared link / receiving shared text, and the current notifications.
 * Everything is optional — return null / do nothing when the platform cannot do it.
 */
interface PluginHost {
    fun getBattery(): BatteryStatus?
    /** Attract attention (alarm sound + vibration), like "Find my phone". */
    fun ring()
    fun setClipboard(text: String)
    fun getClipboard(): String?
    /** Open an already validated http/https/mailto/tel URL. */
    fun openUrl(url: String, from: DeviceInfo)
    /** Text pushed from another device ("Share → Blue Connect"). */
    fun receiveText(text: String, from: DeviceInfo)
    fun getNotifications(): List<NotificationInfo>
}

/** Packet builders / parsers / limits of the plugin protocol. Pure functions — unit-tested. */
object PluginPackets {
    const val MAX_CLIPBOARD_BYTES = 256 * 1024
    const val MAX_SHARED_TEXT_BYTES = 64 * 1024
    const val MAX_URL_LENGTH = 2048

    fun batteryRequest(): ByteArray = Protocol.frame(Protocol.TYPE_BATTERY_REQUEST, JSONObject().put("request", true))
    fun findMyPhone(): ByteArray = Protocol.frame(Protocol.TYPE_FIND_MY_PHONE, JSONObject())
    fun clipboardRequest(): ByteArray = Protocol.frame(Protocol.TYPE_CLIPBOARD_REQUEST, JSONObject())
    fun notificationRequest(): ByteArray = Protocol.frame(Protocol.TYPE_NOTIFICATION_REQUEST, JSONObject().put("request", true))

    fun clipboard(content: String): ByteArray =
        Protocol.frame(Protocol.TYPE_CLIPBOARD, JSONObject().put("content", fitUtf8(content, MAX_CLIPBOARD_BYTES)))

    fun shareText(text: String): ByteArray =
        Protocol.frame(Protocol.TYPE_SHARE, JSONObject().put("text", fitUtf8(text, MAX_SHARED_TEXT_BYTES)))

    fun shareUrl(url: String): ByteArray = Protocol.frame(Protocol.TYPE_SHARE, JSONObject().put("url", url))

    /** -1 means "this device has no battery". */
    fun battery(b: BatteryStatus?): ByteArray = Protocol.frame(
        Protocol.TYPE_BATTERY,
        JSONObject()
            .put("currentCharge", if (b == null) -1 else b.percent.coerceIn(0, 100))
            .put("isCharging", b?.charging ?: false)
            .put("thresholdEvent", 0),
    )

    /** Truncates to at most [maxBytes] of UTF-8 without cutting a character in half. */
    fun fitUtf8(s: String, maxBytes: Int): String {
        val bytes = s.toByteArray(Charsets.UTF_8)
        if (bytes.size <= maxBytes) return s
        var len = maxBytes
        while (len > 0 && (bytes[len].toInt() and 0xC0) == 0x80) len-- // back to a character boundary
        return String(bytes, 0, len, Charsets.UTF_8)
    }

    /** Only plain web / mail / phone links may be opened on request of another device. */
    fun isSafeUrl(url: String?): Boolean {
        if (url.isNullOrBlank() || url.length > MAX_URL_LENGTH) return false
        if (url.any { it.isISOControl() || it.isWhitespace() }) return false
        val scheme = try { URI(url).scheme?.lowercase() } catch (e: Exception) { return false }
        return scheme == "http" || scheme == "https" || scheme == "mailto" || scheme == "tel"
    }

    /** Notification list packet, dropping entries (oldest last) until it fits one TLS record. */
    fun notificationList(items: List<NotificationInfo>): ByteArray {
        for (n in items.size downTo 0) {
            val arr = JSONArray()
            for (i in items.take(n)) {
                arr.put(
                    JSONObject().put("id", i.id).put("appName", fitUtf8(i.app, 80)).put("title", fitUtf8(i.title, 200))
                        .put("text", fitUtf8(i.text, 400)).put("time", i.timeMs),
                )
            }
            val frame = Protocol.frame(Protocol.TYPE_NOTIFICATION_LIST, JSONObject().put("notifications", arr))
            if (frame.size <= Protocol.MAX_SINGLE_READ_REPLY_BYTES) return frame
        }
        return Protocol.frame(Protocol.TYPE_NOTIFICATION_LIST, JSONObject().put("notifications", JSONArray()))
    }

    fun parseBattery(p: Protocol.Packet): BatteryStatus? {
        if (p.type != Protocol.TYPE_BATTERY) return null
        val charge = p.body.optInt("currentCharge", -1)
        if (charge < 0) return null
        return BatteryStatus(minOf(charge, 100), p.body.optBoolean("isCharging", false))
    }

    fun parseNotifications(p: Protocol.Packet): List<NotificationInfo> {
        if (p.type != Protocol.TYPE_NOTIFICATION_LIST) return emptyList()
        val arr = p.body.optJSONArray("notifications") ?: return emptyList()
        return (0 until arr.length()).mapNotNull { i ->
            val o = arr.optJSONObject(i) ?: return@mapNotNull null
            NotificationInfo(o.optString("id"), o.optString("appName"), o.optString("title"), o.optString("text"), o.optLong("time"))
        }
    }
}

/** Routes plugin packets from a PAIRED device to a [PluginHost]; ignores everything else. */
class PluginRouter(private val host: PluginHost) : PacketHandler {
    override fun handle(device: DeviceInfo, packet: Protocol.Packet, reply: (ByteArray) -> Unit) {
        val body = packet.body
        when (packet.type) {
            Protocol.TYPE_BATTERY_REQUEST -> reply(PluginPackets.battery(host.getBattery()))
            Protocol.TYPE_FIND_MY_PHONE -> host.ring()
            Protocol.TYPE_CLIPBOARD -> {
                val text = if (body.has("content")) body.optString("content") else null
                if (text != null && text.toByteArray(Charsets.UTF_8).size <= PluginPackets.MAX_CLIPBOARD_BYTES) host.setClipboard(text)
            }
            Protocol.TYPE_CLIPBOARD_REQUEST -> {
                val text = PluginPackets.fitUtf8(host.getClipboard() ?: "", Protocol.MAX_SINGLE_READ_REPLY_BYTES - 200)
                reply(Protocol.frame(Protocol.TYPE_CLIPBOARD, JSONObject().put("content", text)))
            }
            Protocol.TYPE_SHARE -> {
                val url = if (body.has("url")) body.optString("url") else null
                val text = if (body.has("text")) body.optString("text") else null
                if (url != null) {
                    if (PluginPackets.isSafeUrl(url)) host.openUrl(url, device)
                } else if (!text.isNullOrEmpty() && text.toByteArray(Charsets.UTF_8).size <= PluginPackets.MAX_SHARED_TEXT_BYTES) {
                    host.receiveText(text, device)
                }
            }
            Protocol.TYPE_NOTIFICATION_REQUEST -> reply(PluginPackets.notificationList(host.getNotifications()))
            else -> Unit // not ours (e.g. SMS) — another handler may take it
        }
    }
}

/** Calls every handler for every packet; each one ignores the types that are not its own. */
class CompositeHandler(private vararg val handlers: PacketHandler) : PacketHandler {
    override fun handle(device: DeviceInfo, packet: Protocol.Packet, reply: (ByteArray) -> Unit) {
        for (h in handlers) h.handle(device, packet, reply)
    }
}
