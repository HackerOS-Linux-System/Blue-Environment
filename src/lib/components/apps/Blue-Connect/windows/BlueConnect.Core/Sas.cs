using System.Security.Cryptography;
using System.Text;

namespace BlueConnect.Core;

/// <summary>
/// Certificate fingerprint + Short Authentication String, bit-for-bit the same
/// as <c>src-tauri/src/BlueConnect/tls.rs</c> (<c>fingerprint</c>, <c>compute_sas</c>).
/// </summary>
public static class Sas
{
    /// <summary>SHA-256 of the DER certificate as lowercase hex (64 chars).</summary>
    public static string Fingerprint(ReadOnlySpan<byte> certDer) =>
        Convert.ToHexString(SHA256.HashData(certDer)).ToLowerInvariant();

    /// <summary>
    /// Six-digit code both people compare. Order independent: the two
    /// fingerprints are sorted (ordinal / bytewise), joined with '|', hashed
    /// with SHA-256, and the first four bytes (big endian) are reduced mod 10^6.
    /// </summary>
    public static string Compute(string fingerprintA, string fingerprintB)
    {
        var (first, second) = string.CompareOrdinal(fingerprintA, fingerprintB) <= 0
            ? (fingerprintA, fingerprintB)
            : (fingerprintB, fingerprintA);
        var digest = SHA256.HashData(Encoding.UTF8.GetBytes(first + "|" + second));
        uint n = ((uint)digest[0] << 24) | ((uint)digest[1] << 16) | ((uint)digest[2] << 8) | digest[3];
        return (n % 1_000_000u).ToString("D6");
    }
}
