import Foundation

/// A device we have seen or paired with. Field names/JSON shape are identical
/// to the desktop's `DiscoveredDevice`, so a devices.json is interchangeable
/// between implementations.
public struct DeviceInfo: Codable, Identifiable, Equatable {
    public var id: String
    public var name: String
    public var deviceType: String
    public var address: String
    public var tcpPort: Int
    public var paired: Bool
    public var pinnedCertSha256: String?

    public init(id: String, name: String, deviceType: String = "unknown", address: String = "",
                tcpPort: Int = Int(Proto.pairingTCPPort), paired: Bool = false, pinnedCertSha256: String? = nil) {
        self.id = id; self.name = name; self.deviceType = deviceType; self.address = address
        self.tcpPort = tcpPort; self.paired = paired; self.pinnedCertSha256 = pinnedCertSha256
    }

    enum CodingKeys: String, CodingKey { case id, name, deviceType, address, tcpPort, paired, pinnedCertSha256 }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        name = try c.decodeIfPresent(String.self, forKey: .name) ?? id
        deviceType = Proto.normalizeDeviceType(try c.decodeIfPresent(String.self, forKey: .deviceType))
        address = try c.decodeIfPresent(String.self, forKey: .address) ?? ""
        tcpPort = try c.decodeIfPresent(Int.self, forKey: .tcpPort) ?? Int(Proto.pairingTCPPort)
        paired = try c.decodeIfPresent(Bool.self, forKey: .paired) ?? false
        pinnedCertSha256 = try c.decodeIfPresent(String.self, forKey: .pinnedCertSha256)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(name, forKey: .name)
        try c.encode(deviceType, forKey: .deviceType)
        try c.encode(address, forKey: .address)
        try c.encode(tcpPort, forKey: .tcpPort)
        try c.encode(paired, forKey: .paired)
        // Explicit JSON null (not a missing key) — that is what the desktop writes.
        if let pin = pinnedCertSha256 { try c.encode(pin, forKey: .pinnedCertSha256) } else { try c.encodeNil(forKey: .pinnedCertSha256) }
    }
}

/// Thread-safe, persisted registry of known devices (`devices.json`).
public final class DeviceStore {
    private let lock = NSLock()
    private let url: URL
    private var devices: [String: DeviceInfo] = [:]

    public init(directory: URL) {
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        url = directory.appendingPathComponent("devices.json")
        if let data = try? Data(contentsOf: url),
           let loaded = try? JSONDecoder().decode([String: DeviceInfo].self, from: data) {
            devices = loaded
        }
    }

    private func save() {
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(devices) { try? data.write(to: url, options: .atomic) }
    }

    private func locked<T>(_ body: () -> T) -> T { lock.lock(); defer { lock.unlock() }; return body() }

    public func get(_ id: String) -> DeviceInfo? { locked { devices[id] } }

    public func findPaired(byFingerprint fp: String) -> DeviceInfo? {
        locked { devices.values.first { $0.paired && $0.pinnedCertSha256 == fp } }
    }

    /// Paired devices first, then by name — same order as the desktop's list.
    public func all() -> [DeviceInfo] {
        locked {
            devices.values.sorted { a, b in
                if a.paired != b.paired { return a.paired }
                return a.name < b.name
            }
        }
    }

    /// Records what a discovery datagram told us; never touches pairing state.
    public func seen(id: String, name: String, type: String, address: String, tcpPort: Int) {
        locked {
            if var d = devices[id] {
                d.name = name; d.deviceType = type; d.address = address; d.tcpPort = tcpPort
                devices[id] = d
            } else {
                devices[id] = DeviceInfo(id: id, name: name, deviceType: type, address: address, tcpPort: tcpPort)
            }
            save()
        }
    }

    /// Marks a device paired and pins the certificate it presented.
    public func markPaired(id: String, fallbackName: String, address: String, tcpPort: Int, fingerprint: String) {
        locked {
            var d = devices[id] ?? DeviceInfo(id: id, name: fallbackName, address: address, tcpPort: tcpPort)
            d.paired = true
            d.pinnedCertSha256 = fingerprint
            devices[id] = d
            save()
        }
    }

    public func updateAddress(id: String, address: String) {
        locked {
            if var d = devices[id], d.address != address {
                d.address = address
                devices[id] = d
                save()
            }
        }
    }

    @discardableResult
    public func forget(_ id: String) -> Bool {
        locked {
            let removed = devices.removeValue(forKey: id) != nil
            if removed { save() }
            return removed
        }
    }
}
