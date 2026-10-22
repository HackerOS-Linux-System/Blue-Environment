import Foundation

public struct BatteryStatus: Equatable {
    public let percent: Int
    public let charging: Bool
    public init(percent: Int, charging: Bool) { self.percent = percent; self.charging = charging }
}

public struct NotificationInfo: Equatable {
    public let id: String
    public let app: String
    public let title: String
    public let text: String
    public let timeMs: Int64
    public init(id: String, app: String, title: String, text: String, timeMs: Int64) {
        self.id = id; self.app = app; self.title = title; self.text = text; self.timeMs = timeMs
    }
}

/// What a platform provides so a paired device can use it. Everything is optional:
/// return nil / do nothing when the platform cannot do it.
public protocol PluginHost: AnyObject {
    func batteryStatus() -> BatteryStatus?
    /// Attract attention (sound + Dock bounce), like "Find my phone".
    func ring()
    func setClipboard(_ text: String)
    func currentClipboard() -> String?
    /// Open an already validated http/https/mailto/tel URL.
    func open(url: String, from: DeviceInfo)
    /// Text pushed from another device ("Share → Blue Connect").
    func receive(text: String, from: DeviceInfo)
    func currentNotifications() -> [NotificationInfo]
}

/// Packet builders / parsers / limits of the plugin protocol. Pure functions — unit-tested.
public enum PluginPackets {
    public static let maxClipboardBytes = 256 * 1024
    public static let maxSharedTextBytes = 64 * 1024
    public static let maxURLLength = 2048

    public static func batteryRequest() -> Data { Proto.frame(type: Proto.typeBatteryRequest, body: ["request": true]) }
    public static func findMyPhone() -> Data { Proto.frame(type: Proto.typeFindMyPhone, body: [:]) }
    public static func clipboardRequest() -> Data { Proto.frame(type: Proto.typeClipboardRequest, body: [:]) }
    public static func notificationRequest() -> Data { Proto.frame(type: Proto.typeNotificationRequest, body: ["request": true]) }

    public static func clipboard(_ content: String) -> Data {
        Proto.frame(type: Proto.typeClipboard, body: ["content": fitUTF8(content, maxBytes: maxClipboardBytes)])
    }
    public static func shareText(_ text: String) -> Data {
        Proto.frame(type: Proto.typeShare, body: ["text": fitUTF8(text, maxBytes: maxSharedTextBytes)])
    }
    public static func shareURL(_ url: String) -> Data { Proto.frame(type: Proto.typeShare, body: ["url": url]) }

    /// -1 means "this device has no battery".
    public static func battery(_ b: BatteryStatus?) -> Data {
        Proto.frame(type: Proto.typeBattery, body: [
            "currentCharge": b.map { min(max($0.percent, 0), 100) } ?? -1,
            "isCharging": b?.charging ?? false,
            "thresholdEvent": 0,
        ])
    }

    /// Truncates to at most `maxBytes` of UTF-8 without cutting a character in half.
    public static func fitUTF8(_ s: String, maxBytes: Int) -> String {
        if s.utf8.count <= maxBytes { return s }
        var out = String.UnicodeScalarView()
        var used = 0
        for scalar in s.unicodeScalars {
            let n = String(scalar).utf8.count
            if used + n > maxBytes { break }
            out.append(scalar)
            used += n
        }
        return String(out)
    }

    /// Only plain web / mail / phone links may be opened on request of another device.
    public static func isSafeURL(_ url: String?) -> Bool {
        guard let url, !url.isEmpty, url.count <= maxURLLength else { return false }
        if url.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) || CharacterSet.whitespacesAndNewlines.contains($0) }) {
            return false
        }
        guard let scheme = URL(string: url)?.scheme?.lowercased() else { return false }
        return ["http", "https", "mailto", "tel"].contains(scheme)
    }

    /// Notification list packet, dropping entries (oldest last) until it fits one TLS record.
    public static func notificationList(_ items: [NotificationInfo]) -> Data {
        for n in stride(from: items.count, through: 0, by: -1) {
            let rows: [[String: Any]] = items.prefix(n).map {
                ["id": $0.id, "appName": fitUTF8($0.app, maxBytes: 80), "title": fitUTF8($0.title, maxBytes: 200),
                 "text": fitUTF8($0.text, maxBytes: 400), "time": $0.timeMs]
            }
            let frame = Proto.frame(type: Proto.typeNotificationList, body: ["notifications": rows])
            if frame.count <= Proto.maxSingleReadReplyBytes { return frame }
        }
        return Proto.frame(type: Proto.typeNotificationList, body: ["notifications": [[String: Any]]()])
    }

    public static func parseBattery(_ p: Proto.Packet) -> BatteryStatus? {
        guard p.type == Proto.typeBattery, let charge = (p.body["currentCharge"] as? NSNumber)?.intValue, charge >= 0 else { return nil }
        return BatteryStatus(percent: min(charge, 100), charging: (p.body["isCharging"] as? Bool) ?? false)
    }

    public static func parseNotifications(_ p: Proto.Packet) -> [NotificationInfo] {
        guard p.type == Proto.typeNotificationList, let rows = p.body["notifications"] as? [[String: Any]] else { return [] }
        return rows.map {
            NotificationInfo(id: $0["id"] as? String ?? "", app: $0["appName"] as? String ?? "", title: $0["title"] as? String ?? "",
                             text: $0["text"] as? String ?? "", timeMs: ($0["time"] as? NSNumber)?.int64Value ?? 0)
        }
    }
}

/// Routes plugin packets from a PAIRED device to a `PluginHost`.
public final class PluginRouter {
    private let host: PluginHost
    public init(host: PluginHost) { self.host = host }

    /// - Returns: true when the packet was a plugin packet (handled or deliberately dropped).
    @discardableResult
    public func handle(from dev: DeviceInfo, packet: Proto.Packet, reply: (Data) async throws -> Void) async -> Bool {
        switch packet.type {
        case Proto.typeBatteryRequest:
            try? await reply(PluginPackets.battery(host.batteryStatus()))
            return true

        case Proto.typeFindMyPhone:
            host.ring()
            return true

        case Proto.typeClipboard:
            if let text = packet.body["content"] as? String, text.utf8.count <= PluginPackets.maxClipboardBytes {
                host.setClipboard(text)
            }
            return true

        case Proto.typeClipboardRequest:
            let text = PluginPackets.fitUTF8(host.currentClipboard() ?? "", maxBytes: Proto.maxSingleReadReplyBytes - 200)
            try? await reply(Proto.frame(type: Proto.typeClipboard, body: ["content": text]))
            return true

        case Proto.typeShare:
            if let url = packet.body["url"] as? String {
                if PluginPackets.isSafeURL(url) { host.open(url: url, from: dev) }
            } else if let text = packet.body["text"] as? String, !text.isEmpty, text.utf8.count <= PluginPackets.maxSharedTextBytes {
                host.receive(text: text, from: dev)
            }
            return true

        case Proto.typeNotificationRequest:
            try? await reply(PluginPackets.notificationList(host.currentNotifications()))
            return true

        default:
            return false
        }
    }
}
