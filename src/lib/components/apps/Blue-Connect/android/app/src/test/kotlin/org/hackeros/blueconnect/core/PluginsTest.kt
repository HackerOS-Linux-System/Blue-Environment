package org.hackeros.blueconnect.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PluginsTest {

    private class FakeHost : PluginHost {
        var bat: BatteryStatus? = BatteryStatus(55, false)
        var rings = 0
        var clip: String? = null
        val opened = mutableListOf<String>()
        val texts = mutableListOf<String>()
        override fun getBattery() = bat
        override fun ring() { rings++ }
        override fun setClipboard(text: String) { clip = text }
        override fun getClipboard() = clip
        override fun openUrl(url: String, from: DeviceInfo) { opened += url }
        override fun receiveText(text: String, from: DeviceInfo) { texts += text }
        override fun getNotifications() = listOf(NotificationInfo("1", "Mail", "Hi", "Body", 1L))
    }

    private val dev = DeviceInfo(id = "d", name = "Desktop", deviceType = "desktop", address = "127.0.0.1", tcpPort = 1717)

    private fun run(host: FakeHost, type: String, body: org.json.JSONObject = org.json.JSONObject()): Protocol.Packet? {
        var out: ByteArray? = null
        PluginRouter(host).handle(dev, Protocol.Packet(type, body)) { out = it }
        return out?.let { Protocol.parse(String(it, Charsets.UTF_8).trim()) }
    }

    @Test fun batteryRequestIsAnswered() {
        val host = FakeHost()
        val r = run(host, Protocol.TYPE_BATTERY_REQUEST)!!
        assertEquals(BatteryStatus(55, false), PluginPackets.parseBattery(r))
        host.bat = null
        assertNull(PluginPackets.parseBattery(run(host, Protocol.TYPE_BATTERY_REQUEST)!!))
    }

    @Test fun batteryPercentIsClamped() {
        val p = Protocol.parse(String(PluginPackets.battery(BatteryStatus(250, true)), Charsets.UTF_8).trim())!!
        assertEquals(100, PluginPackets.parseBattery(p)!!.percent)
    }

    @Test fun findMyPhoneRingsAndDoesNotReply() {
        val host = FakeHost()
        assertNull(run(host, Protocol.TYPE_FIND_MY_PHONE))
        assertEquals(1, host.rings)
    }

    @Test fun clipboardRoundTripKeepsUnicode() {
        val host = FakeHost()
        run(host, Protocol.TYPE_CLIPBOARD, org.json.JSONObject().put("content", "zażółć 🙂"))
        assertEquals("zażółć 🙂", host.clip)
        val r = run(host, Protocol.TYPE_CLIPBOARD_REQUEST)!!
        assertEquals(Protocol.TYPE_CLIPBOARD, r.type)
        assertEquals("zażółć 🙂", r.body.getString("content"))
    }

    @Test fun oversizedClipboardIsIgnored() {
        val host = FakeHost()
        run(host, Protocol.TYPE_CLIPBOARD, org.json.JSONObject().put("content", "x".repeat(PluginPackets.MAX_CLIPBOARD_BYTES + 1)))
        assertNull(host.clip)
    }

    @Test fun onlySafeUrlsAreOpened() {
        val host = FakeHost()
        run(host, Protocol.TYPE_SHARE, org.json.JSONObject().put("url", "https://example.org/a?b=c"))
        run(host, Protocol.TYPE_SHARE, org.json.JSONObject().put("url", "javascript:alert(1)"))
        run(host, Protocol.TYPE_SHARE, org.json.JSONObject().put("url", "file:///etc/passwd"))
        run(host, Protocol.TYPE_SHARE, org.json.JSONObject().put("text", "plain note"))
        assertEquals(listOf("https://example.org/a?b=c"), host.opened)
        assertEquals(listOf("plain note"), host.texts)
    }

    @Test fun urlValidation() {
        assertTrue(PluginPackets.isSafeUrl("mailto:a@b.c"))
        assertTrue(PluginPackets.isSafeUrl("tel:+48123456789"))
        assertFalse(PluginPackets.isSafeUrl("https://a b"))
        assertFalse(PluginPackets.isSafeUrl("ms-settings:"))
        assertFalse(PluginPackets.isSafeUrl(""))
        assertFalse(PluginPackets.isSafeUrl(null))
        assertFalse(PluginPackets.isSafeUrl("https://x/" + "a".repeat(3000)))
    }

    @Test fun fitUtf8NeverSplitsCharacters() {
        assertEquals("ą", PluginPackets.fitUtf8("ą" + "x".repeat(10), 2))
        assertEquals("🙂", PluginPackets.fitUtf8("🙂🙂", 5))
        assertEquals("short", PluginPackets.fitUtf8("short", 100))
    }

    @Test fun notificationListFitsOneRecordAndKeepsNewestFirst() {
        val big = (0 until 200).map { NotificationInfo("$it", "App", "t".repeat(200), "b".repeat(400), it.toLong()) }
        val frame = PluginPackets.notificationList(big)
        assertTrue(frame.size <= Protocol.MAX_SINGLE_READ_REPLY_BYTES)
        val parsed = PluginPackets.parseNotifications(Protocol.parse(String(frame, Charsets.UTF_8).trim())!!)
        assertTrue(parsed.isNotEmpty() && parsed.size < 200)
        assertEquals("0", parsed[0].id)
    }

    @Test fun unknownTypesAreLeftToOtherHandlers() {
        val host = FakeHost()
        assertNull(run(host, Protocol.TYPE_SMS_REQUEST))
        var calls = 0
        val other = PacketHandler { _, _, _ -> calls++ }
        CompositeHandler(PluginRouter(host), other).handle(dev, Protocol.Packet(Protocol.TYPE_SMS_REQUEST, org.json.JSONObject())) {}
        assertEquals(1, calls)
    }

    @Test fun identityAdvertisesPluginsAndStillParses() {
        val id = Protocol.buildIdentity("id", "n", "phone", 1717)
        val text = String(id, Charsets.UTF_8)
        assertTrue(text.contains(Protocol.TYPE_CLIPBOARD) && text.contains(Protocol.TYPE_FIND_MY_PHONE))
        assertTrue(Protocol.parseIdentity(id) != null)
    }
}
