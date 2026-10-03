package org.hackeros.blueconnect.core

import java.security.MessageDigest

/**
 * Certificate fingerprint + Short Authentication String, bit-for-bit the same
 * as `src-tauri/src/BlueConnect/tls.rs` (`fingerprint`, `compute_sas`).
 */
object Sas {
    private fun sha256(data: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(data)

    /** SHA-256 of the DER certificate as lowercase hex (64 chars). */
    fun fingerprint(certDer: ByteArray): String =
        sha256(certDer).joinToString("") { "%02x".format(it.toInt() and 0xFF) }

    /**
     * Six-digit code both people compare. Order independent: the two
     * fingerprints are sorted, joined with '|', hashed with SHA-256, and the
     * first four bytes (big endian) are reduced mod 10^6.
     */
    fun compute(fingerprintA: String, fingerprintB: String): String {
        val (first, second) = if (fingerprintA <= fingerprintB) fingerprintA to fingerprintB else fingerprintB to fingerprintA
        val d = sha256((first + "|" + second).toByteArray(Charsets.UTF_8))
        val n = ((d[0].toLong() and 0xFF) shl 24) or ((d[1].toLong() and 0xFF) shl 16) or
            ((d[2].toLong() and 0xFF) shl 8) or (d[3].toLong() and 0xFF)
        return (n % 1_000_000L).toString().padStart(6, '0')
    }
}
