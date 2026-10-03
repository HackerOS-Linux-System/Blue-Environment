package org.hackeros.blueconnect.android

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import org.hackeros.blueconnect.core.Sas
import org.hackeros.blueconnect.core.TlsIdentity
import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import java.util.Date
import javax.net.ssl.KeyManager
import javax.net.ssl.KeyManagerFactory
import javax.security.auth.x500.X500Principal

/**
 * This phone's persistent, self-signed TLS identity (ECDSA P-256).
 *
 * The private key is generated INSIDE the AndroidKeyStore and never leaves it;
 * the platform signs TLS handshakes on our behalf. Android creates the matching
 * self-signed certificate for us, so no X.509 library is needed. Created once
 * and reused forever: replacing it would invalidate every pairing (the peers'
 * pins would no longer match) — same recovery story as the desktop: forget the
 * device and pair again.
 */
class AndroidKeystoreIdentity private constructor(
    override val keyManagers: Array<KeyManager>,
    override val fingerprint: String,
) : TlsIdentity {

    companion object {
        private const val PROVIDER = "AndroidKeyStore"
        private const val ALIAS = "blue_connect_identity"
        private const val TWENTY_YEARS_MS = 20L * 365 * 24 * 3600 * 1000
        private const val ONE_DAY_MS = 24L * 3600 * 1000

        fun loadOrCreate(): AndroidKeystoreIdentity {
            val ks = KeyStore.getInstance(PROVIDER).apply { load(null) }

            var cert = (if (ks.containsAlias(ALIAS)) ks.getCertificate(ALIAS) else null) as? X509Certificate
            val usable = cert != null && cert.notAfter.time > System.currentTimeMillis() + 30 * ONE_DAY_MS
            if (!usable) {
                if (ks.containsAlias(ALIAS)) ks.deleteEntry(ALIAS)
                generate()
                cert = ks.getCertificate(ALIAS) as X509Certificate
            }

            val kmf = KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm())
            kmf.init(ks, null) // AndroidKeyStore keys need no password
            return AndroidKeystoreIdentity(kmf.keyManagers, Sas.fingerprint(cert!!.encoded))
        }

        private fun generate() {
            val now = System.currentTimeMillis()
            val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY)
                .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
                // TLS 1.2 and 1.3 sign with different digests (Conscrypt also uses
                // NONE for pre-hashed input) — allow them all or some peers fail the handshake.
                .setDigests(
                    KeyProperties.DIGEST_NONE,
                    KeyProperties.DIGEST_SHA256,
                    KeyProperties.DIGEST_SHA384,
                    KeyProperties.DIGEST_SHA512,
                )
                .setCertificateSubject(X500Principal("CN=blue-connect.local"))
                .setCertificateSerialNumber(BigInteger.valueOf(now))
                .setCertificateNotBefore(Date(now - ONE_DAY_MS))
                .setCertificateNotAfter(Date(now + TWENTY_YEARS_MS))
                .build()
            KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, PROVIDER).apply { initialize(spec) }.generateKeyPair()
        }
    }
}
