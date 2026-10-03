package org.hackeros.blueconnect.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PacketReaderTest {
    @Test fun readsBackToBackPacketsSkipsBlankLinesAndAcceptsUnterminatedLast() {
        val input = "{\"type\":\"a\",\"body\":{}}\n\n{\"type\":\"b\",\"body\":{}}\n{\"type\":\"c\",\"body\":{}}".byteInputStream()
        val r = PacketReader(input)
        assertTrue(r.readLine()!!.contains("\"a\""))
        assertTrue(r.readLine()!!.contains("\"b\""))
        assertTrue(r.readLine()!!.contains("\"c\""))
        assertNull(r.readLine())
    }

    @Test fun decodesUtf8() {
        val r = PacketReader("{\"body\":\"Zażółć gęślą jaźń 🙂\"}\n".byteInputStream(Charsets.UTF_8))
        assertEquals("{\"body\":\"Zażółć gęślą jaźń 🙂\"}", r.readLine())
    }

    @Test fun emptyStreamEndsImmediately() {
        assertNull(PacketReader("".byteInputStream()).readLine())
    }
}
