import Foundation

/// Wire constants + packet helpers. Mirrors the desktop implementation in
/// `src-tauri/src/BlueConnect/mod.rs` exactly — if you change anything here,
/// that file (and the Android/Windows clients) must change with it.
///
/// Transport summary:
///  * Discovery: one UDP datagram holding a `kdeconnect.identity` JSON document
///    (no trailing newline), port 1716.
///  * Everything else: mutual TLS over TCP (port from the identity's tcpPort,
///    1717 by default), newline-terminated JSON packets. Both sides present a
///    self-signed certificate; trust is decided by pinning the certificate's
///    SHA-256 fingerprint after a Short-Authentication-String confirmation.
public enum Proto {
    public static let discoveryPort: UInt16 = 1716
    public static let pairingTCPPort: UInt16 = 1717
    public static let protocolVersion = 7

    public static let typeIdentity = "kdeconnect.identity"
    public static let typePair = "kdeconnect.pair"
    public static let typePing = "kdeconnect.ping"
    public static let typeSMSRequest = "kdeconnect.sms.request"
    public static let typeSMSMessages = "kdeconnect.sms.messages"

    /// How long either side waits for the other person to answer a pairing request.
    public static let pairingDecisionTimeout: TimeInterval = 120
    /// Upper bound for one packet we are willing to buffer.
    public static let maxPacketBytes = 1024 * 1024

    public static let deviceTypes = ["phone", "tablet", "desktop", "laptop", "tv", "unknown"]

    public static func normalizeDeviceType(_ t: String?) -> String {
        let v = (t ?? "").trimmingCharacters(in: .whitespaces).lowercased()
        return deviceTypes.contains(v) ? v : "unknown"
    }

    /// Builds the discovery datagram (no trailing newline — matches the desktop).
    public static func buildIdentity(deviceId: String, deviceName: String, deviceType: String, tcpPort: Int) -> Data {
        let body: [String: Any] = [
            "deviceId": deviceId,
            "deviceName": deviceName,
            "deviceType": normalizeDeviceType(deviceType),
            "protocolVersion": protocolVersion,
            "tcpPort": tcpPort,
            // Extra fields: ignored by the desktop (serde skips unknown keys),
            // but let real KDE Connect negotiate plugins.
            "incomingCapabilities": [typePair, typePing, typeSMSMessages],
            "outgoingCapabilities": [typePair, typePing, typeSMSRequest],
        ]
        return encode(["type": typeIdentity, "body": body])
    }

    private static func encode(_ obj: [String: Any]) -> Data {
        (try? JSONSerialization.data(withJSONObject: obj, options: [.withoutEscapingSlashes])) ?? Data()
    }

    /// One framed packet: compact JSON + "\n". Always written with a single send.
    public static func frame(type: String, body: [String: Any]) -> Data {
        var d = encode(["type": type, "body": body])
        d.append(0x0A)
        return d
    }

    public static func pairRequest(deviceId: String) -> Data {
        frame(type: typePair, body: ["pair": true, "deviceId": deviceId])
    }

    public static func pairResponse(accepted: Bool) -> Data {
        frame(type: typePair, body: ["pair": accepted])
    }

    public struct Packet {
        public let type: String
        public let body: [String: Any]
    }

    /// Parses one JSON packet; nil for anything that is not a packet object.
    public static func parse(_ data: Data) -> Packet? {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = obj["type"] as? String, !type.isEmpty else { return nil }
        return Packet(type: type, body: obj["body"] as? [String: Any] ?? [:])
    }

    public static func parse(_ line: String) -> Packet? { parse(Data(line.utf8)) }

    public struct Identity: Equatable {
        public let deviceId: String
        public let deviceName: String
        public let deviceType: String
        public let protocolVersion: Int
        public let tcpPort: Int
    }

    /// Parses a discovery datagram. Same strictness as the desktop's serde
    /// struct: every field is mandatory, otherwise the datagram is ignored.
    public static func parseIdentity(_ data: Data) -> Identity? {
        guard let p = parse(data), p.type == typeIdentity else { return nil }
        let b = p.body
        guard let id = b["deviceId"] as? String, !id.trimmingCharacters(in: .whitespaces).isEmpty,
              let name = b["deviceName"] as? String,
              let type = b["deviceType"] as? String,
              let ver = (b["protocolVersion"] as? NSNumber)?.intValue,
              let port = (b["tcpPort"] as? NSNumber)?.intValue,
              (1...65535).contains(port) else { return nil }
        return Identity(deviceId: id, deviceName: name, deviceType: normalizeDeviceType(type), protocolVersion: ver, tcpPort: port)
    }
}
