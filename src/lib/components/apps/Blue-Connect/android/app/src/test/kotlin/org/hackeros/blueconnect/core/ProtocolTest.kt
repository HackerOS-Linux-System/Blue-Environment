package org.hackeros.blueconnect.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProtocolTest {
    @Test fun identityRoundTrip() {
        val bytes = Protocol.buildIdentity("blue-connect-abc", "Test Phone", "phone", 1717)
        val id = Protocol.parseIdentity(bytes)
        assertNotNull(id)
        assertEquals("blue-connect-abc", id!!.deviceId)
        assertEquals("Test Phone", id.deviceName)
        assertEquals("phone", id.deviceType)
        assertEquals(1717, id.tcpPort)
        assertEquals(7, id.protocolVersion)
    }

    @Test fun identityDatagramHasNoTrailingNewline() {
        assertFalse(String(Protocol.buildIdentity("x", "y", "phone", 1717)).endsWith("\n"))
    }

    @Test fun identityWithMissingFieldsIsIgnored() {
        assertNull(Protocol.parseIdentity("""{"type":"kdeconnect.identity","body":{"deviceId":"x"}}""".toByteArray()))
        assertNull(Protocol.parseIdentity("""{"type":"kdeconnect.pair","body":{}}""".toByteArray()))
        assertNull(Protocol.parseIdentity("not json".toByteArray()))
    }

    @Test fun invalidPortIsIgnored() {
        val bad = """{"type":"kdeconnect.identity","body":{"deviceId":"x","deviceName":"n","deviceType":"phone","protocolVersion":7,"tcpPort":70000}}"""
        assertNull(Protocol.parseIdentity(bad.toByteArray()))
    }

    @Test fun unknownDeviceTypeBecomesUnknown() {
        assertEquals("unknown", Protocol.normalizeDeviceType("toaster"))
        assertEquals("laptop", Protocol.normalizeDeviceType(" Laptop "))
    }

    @Test fun framedPacketEndsWithNewlineAndCarriesPairFlag() {
        val framed = Protocol.pairRequest("dev1")
        assertEquals('\n'.code.toByte(), framed.last())
        val p = Protocol.parse(String(framed).trim())
        assertNotNull(p)
        assertEquals(Protocol.TYPE_PAIR, p!!.type)
        assertTrue(p.body.getBoolean("pair"))
        assertEquals("dev1", p.body.getString("deviceId"))
    }

    @Test fun pairResponseIsASingleJsonDocument() {
        val framed = String(Protocol.pairResponse(true))
        assertEquals(1, framed.count { it == '\n' })
        assertTrue(Protocol.parse(framed.trim())!!.body.getBoolean("pair"))
    }
}
