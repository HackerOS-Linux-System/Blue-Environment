using System.Net.Security;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

namespace BlueConnect.Core;

/// <summary>
/// This install's persistent, self-signed TLS identity (ECDSA P-256, like the
/// desktop's rcgen default). Generated once, stored next to the other Blue
/// Connect data and reused forever; losing it invalidates every pairing
/// (the peers' pins no longer match) — same recovery story as the desktop:
/// forget + re-pair.
/// </summary>
public sealed class TlsIdentity : IDisposable
{
    // PFX needs *a* password to round-trip on every .NET platform. It is not a
    // secret: the file lives in the user's profile, exactly like the desktop's
    // identity_key.pem.
    private const string PfxPassword = "blue-connect";

    public X509Certificate2 Certificate { get; }
    public string Fingerprint { get; }

    private TlsIdentity(X509Certificate2 cert)
    {
        Certificate = cert;
        Fingerprint = Sas.Fingerprint(cert.RawData);
    }

    public static TlsIdentity LoadOrCreate(string dataDir)
    {
        Directory.CreateDirectory(dataDir);
        var path = Path.Combine(dataDir, "identity.pfx");

        if (File.Exists(path))
        {
            try
            {
                var existing = new X509Certificate2(File.ReadAllBytes(path), PfxPassword, X509KeyStorageFlags.Exportable);
                if (existing.HasPrivateKey && existing.NotAfter > DateTime.Now.AddDays(30))
                    return new TlsIdentity(existing);
                existing.Dispose();
            }
            catch
            {
                // Corrupt file — fall through and create a fresh identity.
            }
        }

        using var key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        var req = new CertificateRequest("CN=blue-connect.local", key, HashAlgorithmName.SHA256);
        req.CertificateExtensions.Add(new X509BasicConstraintsExtension(false, false, 0, false));
        req.CertificateExtensions.Add(new X509KeyUsageExtension(X509KeyUsageFlags.DigitalSignature, true));
        req.CertificateExtensions.Add(new X509EnhancedKeyUsageExtension(
            new OidCollection { new Oid("1.3.6.1.5.5.7.3.1"), new Oid("1.3.6.1.5.5.7.3.2") }, false));
        var san = new SubjectAlternativeNameBuilder();
        san.AddDnsName("blue-connect.local");
        req.CertificateExtensions.Add(san.Build());

        using var ephemeral = req.CreateSelfSigned(DateTimeOffset.UtcNow.AddDays(-1), DateTimeOffset.UtcNow.AddYears(20));
        var pfx = ephemeral.Export(X509ContentType.Pfx, PfxPassword);

        // Write atomically, then load from the bytes: a cert created purely in
        // memory has an ephemeral key that Windows' Schannel refuses to use for
        // SslStream server authentication; the PFX round-trip gives it a real
        // key container.
        var tmp = path + ".tmp";
        File.WriteAllBytes(tmp, pfx);
        File.Move(tmp, path, overwrite: true);
        return new TlsIdentity(new X509Certificate2(pfx, PfxPassword, X509KeyStorageFlags.Exportable));
    }

    /// <summary>
    /// Client-side TLS options: present our certificate (mutual TLS) and accept
    /// ANY server certificate at the handshake layer — the real trust decision
    /// (fingerprint pin + SAS) is made by the caller from <paramref name="captured"/>.
    /// The callback only reports identity; it never decides trust.
    /// </summary>
    public SslClientAuthenticationOptions ClientOptions(string targetHost, Action<string> captured) => new()
    {
        TargetHost = targetHost,
        ClientCertificates = new X509CertificateCollection { Certificate },
        LocalCertificateSelectionCallback = (_, _, _, _, _) => Certificate,
        RemoteCertificateValidationCallback = (_, cert, _, _) =>
        {
            if (cert is null) return false;
            captured(Sas.Fingerprint(cert.GetRawCertData()));
            return true;
        },
        CertificateRevocationCheckMode = X509RevocationMode.NoCheck,
    };

    /// <summary>Server-side options: require a client certificate, accept any, report its fingerprint.</summary>
    public SslServerAuthenticationOptions ServerOptions(Action<string> captured) => new()
    {
        ServerCertificate = Certificate,
        ClientCertificateRequired = true,
        RemoteCertificateValidationCallback = (_, cert, _, _) =>
        {
            if (cert is null) return false;
            captured(Sas.Fingerprint(cert.GetRawCertData()));
            return true;
        },
        CertificateRevocationCheckMode = X509RevocationMode.NoCheck,
    };

    public void Dispose() => Certificate.Dispose();
}
