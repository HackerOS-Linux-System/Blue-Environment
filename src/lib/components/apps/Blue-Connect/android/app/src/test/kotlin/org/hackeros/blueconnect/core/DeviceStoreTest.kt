package org.hackeros.blueconnect.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.nio.file.Files

class DeviceStoreTest {
    private fun tmp(): File = Files.createTempDirectory("bc-store").toFile()

    @Test fun readsTheDesktopsDevicesJson() {
        val dir = tmp()
        File(dir, "devices.json").writeText(
            """{"d1":{"id":"d1","name":"Phone","deviceType":"phone","address":"192.168.1.5","tcpPort":1717,"paired":true,"pinnedCertSha256":"abc"},""" +
                """"d2":{"id":"d2","name":"Old","deviceType":"desktop","address":"10.0.0.2","tcpPort":1717,"paired":false,"pinnedCertSha256":null}}""",
        )
        val store = DeviceStore(dir)
        assertEquals(listOf("d1", "d2"), store.all().map { it.id }) // paired first
        assertEquals("d1", store.findPairedByFingerprint("abc")?.id)
        assertNull(store.get("d2")!!.pinnedCertSha256) // JSON null stays null, never the string "null"
        assertNull(store.findPairedByFingerprint("zzz"))
    }

    @Test fun seenUpdatesDiscoveryFieldsButNeverPairingState() {
        val store = DeviceStore(tmp())
        store.markPaired("d1", "First", "1.1.1.1", 1717, "fp1")
        store.seen("d1", "Renamed", "laptop", "2.2.2.2", 1800)
        val d = store.get("d1")!!
        assertEquals("Renamed", d.name)
        assertEquals("2.2.2.2", d.address)
        assertEquals(1800, d.tcpPort)
        assertTrue(d.paired)
        assertEquals("fp1", d.pinnedCertSha256)
    }

    @Test fun changesArePersistedAndForgetRemoves() {
        val dir = tmp()
        DeviceStore(dir).markPaired("d1", "N", "1.1.1.1", 1717, "ff")
        val again = DeviceStore(dir)
        assertEquals("ff", again.get("d1")!!.pinnedCertSha256)
        assertTrue(again.forget("d1"))
        assertFalse(again.forget("d1"))
        assertNull(DeviceStore(dir).get("d1"))
    }

    @Test fun writesTheDesktopsFieldNames() {
        val dir = tmp()
        DeviceStore(dir).markPaired("d1", "N", "1.1.1.1", 1717, "ff")
        val text = File(dir, "devices.json").readText()
        assertTrue(text.contains("\"pinnedCertSha256\"") && text.contains("\"deviceType\"") && text.contains("\"tcpPort\""))
    }
}
