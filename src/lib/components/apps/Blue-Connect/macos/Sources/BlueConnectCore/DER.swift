import Foundation

/// Minimal ASN.1 DER writer — just enough to assemble a self-signed X.509
/// certificate (macOS has no public API to create one). The exact layout was
/// cross-checked with OpenSSL (`openssl x509` parses it and `openssl verify
/// -check_ss_sig` accepts the signature).
enum DER {
    static func length(_ n: Int) -> [UInt8] {
        if n < 0x80 { return [UInt8(n)] }
        var bytes: [UInt8] = []
        var v = n
        while v > 0 { bytes.insert(UInt8(v & 0xFF), at: 0); v >>= 8 }
        return [0x80 | UInt8(bytes.count)] + bytes
    }

    static func tlv(_ tag: UInt8, _ content: [UInt8]) -> [UInt8] {
        [tag] + length(content.count) + content
    }

    static func sequence(_ items: [[UInt8]]) -> [UInt8] { tlv(0x30, items.flatMap { $0 }) }
    static func set(_ items: [[UInt8]]) -> [UInt8] { tlv(0x31, items.flatMap { $0 }) }

    /// Positive INTEGER from big-endian magnitude bytes.
    static func integer(_ magnitude: [UInt8]) -> [UInt8] {
        var b = Array(magnitude.drop(while: { $0 == 0 }))
        if b.isEmpty { b = [0] }
        if b[0] & 0x80 != 0 { b.insert(0, at: 0) }
        return tlv(0x02, b)
    }

    static func oid(_ arcs: [UInt64]) -> [UInt8] {
        precondition(arcs.count >= 2)
        var out: [UInt8] = [UInt8(arcs[0] * 40 + arcs[1])]
        for arc in arcs.dropFirst(2) {
            var chunk: [UInt8] = [UInt8(arc & 0x7F)]
            var a = arc >> 7
            while a > 0 { chunk.append(0x80 | UInt8(a & 0x7F)); a >>= 7 }
            out += chunk.reversed()
        }
        return tlv(0x06, out)
    }

    static func utf8String(_ s: String) -> [UInt8] { tlv(0x0C, Array(s.utf8)) }

    /// UTCTime (valid through 2049, which covers the 10 year validity we use).
    static func utcTime(_ date: Date) -> [UInt8] {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        let c = cal.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
        let s = String(format: "%02ld%02ld%02ld%02ld%02ld%02ldZ", (c.year ?? 2000) % 100, c.month ?? 1, c.day ?? 1, c.hour ?? 0, c.minute ?? 0, c.second ?? 0)
        return tlv(0x17, Array(s.utf8))
    }

    static func bitString(_ bytes: [UInt8]) -> [UInt8] { tlv(0x03, [0x00] + bytes) }
    static func explicit(_ n: UInt8, _ content: [UInt8]) -> [UInt8] { tlv(0xA0 | n, content) }
}

enum X509 {
    static let ecdsaWithSHA256 = DER.oid([1, 2, 840, 10045, 4, 3, 2])
    static let ecPublicKey = DER.oid([1, 2, 840, 10045, 2, 1])
    static let prime256v1 = DER.oid([1, 2, 840, 10045, 3, 1, 7])
    static let commonName = DER.oid([2, 5, 4, 3])

    /// The "to be signed" part of a self-signed certificate for a P-256 key.
    /// - Parameter publicKeyPoint: 65-byte ANSI X9.63 point (04 || X || Y),
    ///   exactly what `SecKeyCopyExternalRepresentation` returns for EC keys.
    static func tbsCertificate(commonName cn: String, publicKeyPoint: [UInt8], serial: [UInt8], notBefore: Date, notAfter: Date) -> [UInt8] {
        let name = DER.sequence([DER.set([DER.sequence([commonName, DER.utf8String(cn)])])])
        let spki = DER.sequence([
            DER.sequence([ecPublicKey, prime256v1]),
            DER.bitString(publicKeyPoint),
        ])
        return DER.sequence([
            DER.explicit(0, DER.integer([2])),               // version: v3
            DER.integer(serial),
            DER.sequence([ecdsaWithSHA256]),                 // signature algorithm
            name,                                            // issuer (self-signed)
            DER.sequence([DER.utcTime(notBefore), DER.utcTime(notAfter)]),
            name,                                            // subject
            spki,
        ])
    }

    /// Final certificate. `signatureDER` is the ASN.1 ECDSA-Sig-Value
    /// `SecKeyCreateSignature(.ecdsaSignatureMessageX962SHA256)` produces.
    static func certificate(tbs: [UInt8], signatureDER: [UInt8]) -> [UInt8] {
        DER.sequence([tbs, DER.sequence([ecdsaWithSHA256]), DER.bitString(signatureDER)])
    }
}
