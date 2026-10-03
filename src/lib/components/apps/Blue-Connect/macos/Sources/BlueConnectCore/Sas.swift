import CryptoKit
import Foundation

/// Certificate fingerprint + Short Authentication String, bit-for-bit the same
/// as `src-tauri/src/BlueConnect/tls.rs` (`fingerprint`, `compute_sas`).
public enum Sas {
    /// SHA-256 of the DER certificate as lowercase hex (64 chars).
    public static func fingerprint(_ certDER: Data) -> String {
        SHA256.hash(data: certDER).map { String(format: "%02x", $0) }.joined()
    }

    /// Six-digit code both people compare. Order independent: the two
    /// fingerprints are sorted (bytewise), joined with "|", hashed with
    /// SHA-256, and the first four bytes (big endian) are reduced mod 10^6.
    public static func compute(_ a: String, _ b: String) -> String {
        // Fingerprints are lowercase hex, so ordinal == bytewise ordering; compare UTF-8 bytes to be exact.
        let aBytes = Array(a.utf8), bBytes = Array(b.utf8)
        let (first, second) = aBytes.lexicographicallyPrecedes(bBytes) || aBytes == bBytes ? (a, b) : (b, a)
        let d = Array(SHA256.hash(data: Data((first + "|" + second).utf8)))
        let n = (UInt32(d[0]) << 24) | (UInt32(d[1]) << 16) | (UInt32(d[2]) << 8) | UInt32(d[3])
        return String(format: "%06u", n % 1_000_000)
    }
}
