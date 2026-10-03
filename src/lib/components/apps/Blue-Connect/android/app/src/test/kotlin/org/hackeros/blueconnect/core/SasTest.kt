package org.hackeros.blueconnect.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The vectors below were produced by the desktop's own `tls.rs::compute_sas`. */
class SasTest {
    @Test fun matchesTheDesktopImplementation() {
        assertEquals("790764", Sas.compute("aaaa1111", "bbbb2222"))
        assertEquals("781997", Sas.compute("x", "y"))
        assertEquals("433941", Sas.compute("0123456789abcdef", "fedcba9876543210"))
    }

    @Test fun isOrderIndependent() {
        assertEquals(Sas.compute("aaaa1111", "bbbb2222"), Sas.compute("bbbb2222", "aaaa1111"))
    }

    @Test fun fingerprintIsLowercaseSha256Hex() {
        val fp = Sas.fingerprint(byteArrayOf(1, 2, 3, 4))
        assertEquals("9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a", fp)
        assertEquals(64, fp.length)
        assertTrue(fp == fp.lowercase())
    }
}
