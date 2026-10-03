package org.hackeros.blueconnect.core

import java.net.Socket
import java.security.SecureRandom
import java.security.cert.X509Certificate
import javax.net.ssl.KeyManager
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLEngine
import javax.net.ssl.SSLSession
import javax.net.ssl.X509ExtendedTrustManager

/**
 * This install's persistent self-signed TLS identity. On Android it is backed
 * by the AndroidKeyStore (the private key never leaves secure storage); the
 * JVM tests use a PKCS#12 file. Losing it invalidates every pairing (the
 * peers' pins no longer match) — same recovery story as the desktop: forget
 * the device and pair again.
 */
interface TlsIdentity {
    /** Key managers that present our certificate during the handshake (mutual TLS). */
    val keyManagers: Array<KeyManager>

    /** SHA-256 of our certificate (lowercase hex) — see [Sas.fingerprint]. */
    val fingerprint: String
}

/**
 * Accepts ANY peer certificate at the handshake layer. That is deliberate and
 * is NOT where trust is decided: the real decision (certificate pin + the
 * Short Authentication String the two people compare) is made by
 * [ConnectEngine] from the fingerprint it reads off the finished session —
 * the same design as the desktop's `FingerprintCapturingVerifier` in tls.rs.
 * This class only lets the handshake complete so the identity can be read.
 *
 * Extending [X509ExtendedTrustManager] (instead of plain X509TrustManager)
 * stops the JDK/Conscrypt from wrapping it with additional checks such as
 * hostname/algorithm constraints that make no sense for self-signed peers.
 */
@Suppress("CustomX509TrustManager", "TrustAllX509TrustManager")
internal class IdentityOnlyTrustManager : X509ExtendedTrustManager() {
    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?, socket: Socket?) {}
    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?, socket: Socket?) {}
    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?, engine: SSLEngine?) {}
    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?, engine: SSLEngine?) {}
    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {}
    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {}
    // Empty: send no CA hints in the CertificateRequest, so any client cert is acceptable.
    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
}

object TlsSupport {
    fun context(identity: TlsIdentity): SSLContext =
        SSLContext.getInstance("TLS").apply {
            init(identity.keyManagers, arrayOf(IdentityOnlyTrustManager()), SecureRandom())
        }

    /** Fingerprint of the certificate the other side presented in this (finished) session, or null. */
    fun peerFingerprint(session: SSLSession): String? = try {
        (session.peerCertificates.firstOrNull() as? X509Certificate)?.encoded?.let { Sas.fingerprint(it) }
    } catch (e: Exception) {
        null // SSLPeerUnverifiedException: the peer sent no certificate
    }
}
