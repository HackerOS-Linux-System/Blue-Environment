import Foundation
import Network
import Security

public struct ConnectOptions {
    /// Folder for device_id and devices.json.
    public var dataDirectory: URL
    public var deviceName: String
    /// phone | tablet | desktop | laptop | tv
    public var deviceType: String
    public var discoveryPort: UInt16
    public var tcpPort: UInt16
    /// UDP port scans send to. nil = `discoveryPort`. Only tests need to differ.
    public var scanTargetPort: UInt16?
    /// Where scans send the identity datagram. nil = broadcast + every interface's directed broadcast.
    public var scanTargets: [String]?

    public init(dataDirectory: URL, deviceName: String, deviceType: String = "laptop",
                discoveryPort: UInt16 = Proto.discoveryPort, tcpPort: UInt16 = Proto.pairingTCPPort,
                scanTargetPort: UInt16? = nil, scanTargets: [String]? = nil) {
        self.dataDirectory = dataDirectory; self.deviceName = deviceName; self.deviceType = deviceType
        self.discoveryPort = discoveryPort; self.tcpPort = tcpPort
        self.scanTargetPort = scanTargetPort; self.scanTargets = scanTargets
    }
}

/// A pairing request from another device, waiting for this person's yes/no.
public struct IncomingPairingRequest: Identifiable, Equatable {
    public let deviceId: String
    public let deviceName: String
    public let address: String
    public let sas: String
    public var id: String { deviceId + sas }
}

/// Shown while WE are the one asking: the code to compare with the other screen.
public struct OutgoingPairingSAS: Equatable {
    public let deviceId: String
    public let deviceName: String
    public let sas: String
}

public struct SMSMessage: Identifiable, Equatable {
    public let id = UUID()
    public let address: String
    public let body: String
}

/// UI hooks. Methods may be called on any thread.
public protocol ConnectDelegate: AnyObject {
    func connectDevicesChanged()
    /// Show the SAS and ask the person. Never auto-accept. The service stops waiting after 2 minutes.
    func connectIncomingPairing(_ request: IncomingPairingRequest) async -> Bool
    func connectOutgoingSAS(_ info: OutgoingPairingSAS)
    func connectPing(from device: DeviceInfo, message: String?)
    func connectLog(_ message: String)
}

public extension ConnectDelegate {
    func connectDevicesChanged() {}
    func connectIncomingPairing(_ request: IncomingPairingRequest) async -> Bool { false }
    func connectOutgoingSAS(_ info: OutgoingPairingSAS) {}
    func connectPing(from device: DeviceInfo, message: String?) {}
    func connectLog(_ message: String) {}
}

/// One-at-a-time async mutex (only one pairing dialog may be on screen).
private actor AsyncGate {
    private var busy = false
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func acquire() async {
        if !busy { busy = true; return }
        await withCheckedContinuation { waiters.append($0) }
    }

    func release() {
        if waiters.isEmpty { busy = false } else { waiters.removeFirst().resume() }
    }
}

/// The whole Blue Connect client: identity, device registry, UDP discovery,
/// the mutual-TLS listener (incoming pairing + packets from paired devices)
/// and the client side (pair with a device, ping, SMS relay through a phone).
/// UI-free.
public final class ConnectService: @unchecked Sendable {
    private let options: ConnectOptions
    private let identity: TLSIdentity
    private let store: DeviceStore
    private var discovery: UDPDiscovery!
    private var listener: NWListener?
    private let queue = DispatchQueue(label: "blue-connect.service")
    private let gate = AsyncGate()

    public let deviceId: String
    public var deviceName: String { options.deviceName }
    public var fingerprint: String { identity.fingerprint }
    public weak var delegate: ConnectDelegate?

    public init(options: ConnectOptions, identity: TLSIdentity) {
        self.options = options
        self.identity = identity
        self.store = DeviceStore(directory: options.dataDirectory)
        let ownDeviceId = ConnectService.loadOrCreateDeviceId(options.dataDirectory)
        self.deviceId = ownDeviceId
        self.discovery = UDPDiscovery(
            discoveryPort: options.discoveryPort,
            scanTargetPort: options.scanTargetPort,
            targets: options.scanTargets,
            ownId: { ownDeviceId },
            ownIdentity: { [weak self] in
                guard let self else { return Data() }
                return Proto.buildIdentity(deviceId: self.deviceId, deviceName: self.options.deviceName,
                                           deviceType: self.options.deviceType, tcpPort: Int(self.options.tcpPort))
            },
            onSeen: { [weak self] id, ip in
                guard let self else { return }
                self.store.seen(id: id.deviceId, name: id.deviceName, type: id.deviceType, address: ip, tcpPort: id.tcpPort)
                self.delegate?.connectDevicesChanged()
            },
            log: { [weak self] in self?.delegate?.connectLog($0) }
        )
    }

    private static func loadOrCreateDeviceId(_ dir: URL) -> String {
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let file = dir.appendingPathComponent("device_id")
        if let existing = try? String(contentsOf: file, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines), !existing.isEmpty {
            return existing
        }
        var bytes = [UInt8](repeating: 0, count: 10)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        let id = "blue-connect-" + bytes.map { String(format: "%02x", $0) }.joined()
        try? id.write(to: file, atomically: true, encoding: .utf8)
        return id
    }

    public var devices: [DeviceInfo] { store.all() }

    // MARK: lifecycle

    /// Starts the TLS listener and the discovery listener. Throws if the TCP port cannot be bound.
    public func start() async throws {
        guard listener == nil else { return }
        let params = TLSConnection.parameters(identity: identity)
        params.allowLocalEndpointReuse = true
        guard let port = NWEndpoint.Port(rawValue: options.tcpPort) else { throw ConnectError.message("Invalid TCP port") }
        let l = try NWListener(using: params, on: port)
        l.newConnectionHandler = { [weak self] conn in
            guard let self else { conn.cancel(); return }
            Task { await self.handleIncoming(conn) }
        }

        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            let done = Locked(false)
            l.stateUpdateHandler = { [weak self] state in
                switch state {
                case .ready:
                    if !done.get() { done.set(true); cont.resume() }
                case .failed(let e):
                    if !done.get() { done.set(true); cont.resume(throwing: ConnectError.message("TCP port \(self?.options.tcpPort ?? 0) is not available: \(e.localizedDescription)")) }
                    else { self?.delegate?.connectLog("Listener failed: \(e.localizedDescription)") }
                default:
                    break
                }
            }
            l.start(queue: queue)
        }
        listener = l
        discovery.startListener()
        delegate?.connectLog("Blue Connect listening: TCP \(options.tcpPort), UDP \(options.discoveryPort) as \"\(options.deviceName)\" (\(deviceId))")
    }

    public func stop() {
        listener?.cancel()
        listener = nil
        discovery.stop()
    }

    // MARK: discovery

    /// Broadcasts and listens for replies for `duration`, then returns the known devices.
    public func scan(duration: TimeInterval = 3) async -> [DeviceInfo] {
        await withCheckedContinuation { (cont: CheckedContinuation<Void, Never>) in
            DispatchQueue.global(qos: .userInitiated).async { [self] in
                discovery.scanBlocking(duration: duration)
                cont.resume()
            }
        }
        return store.all()
    }

    public func forget(_ id: String) {
        if store.forget(id) { delegate?.connectDevicesChanged() }
    }

    // MARK: incoming

    private func handleIncoming(_ conn: NWConnection) async {
        let tls: TLSConnection
        do { tls = try await TLSConnection.accepted(conn, timeout: 20) } catch { conn.cancel(); return }
        defer { tls.close() }
        guard let peerFp = tls.peerFingerprint else { return }
        let ip = tls.remoteAddress ?? "unknown"

        let reader = tls.makeReader()
        tls.readTimeout = 30
        guard let line = try? await reader.readLine(), let first = Proto.parse(line) else { return }

        if first.type == Proto.typePair {
            await handlePairPacket(tls, first, peerFp: peerFp, ip: ip)
            return
        }

        // Any other packet: only from a device we paired (certificate pinned).
        guard let dev = store.findPaired(byFingerprint: peerFp) else {
            delegate?.connectLog("Ignored a '\(first.type)' packet from \(ip): not a paired device")
            return
        }
        store.updateAddress(id: dev.id, address: ip)

        var packet: Proto.Packet? = first
        while let p = packet {
            if p.type == Proto.typePing {
                delegate?.connectPing(from: dev, message: p.body["message"] as? String)
            }
            // Other plugins (SMS gateway etc.) only exist on the phone; a desktop ignores them.
            tls.readTimeout = 10
            guard let next = try? await reader.readLine() else { break }
            packet = Proto.parse(next)
        }
    }

    private func handlePairPacket(_ tls: TLSConnection, _ pkt: Proto.Packet, peerFp: String, ip: String) async {
        let wantsPair = (pkt.body["pair"] as? Bool) ?? false
        if !wantsPair {
            // The other side unpaired us: drop the pin so it can no longer talk to us.
            if let dev = store.findPaired(byFingerprint: peerFp) {
                store.forget(dev.id)
                delegate?.connectLog("\(dev.name) unpaired this device")
                delegate?.connectDevicesChanged()
            }
            return
        }
        guard let deviceId = pkt.body["deviceId"] as? String, !deviceId.isEmpty else {
            delegate?.connectLog("Pairing packet from \(ip) is missing deviceId")
            return
        }

        let existing = store.get(deviceId)
        if let e = existing, e.paired, let pinned = e.pinnedCertSha256, pinned != peerFp {
            // Same stance as the desktop: never silently re-pin a different key.
            delegate?.connectLog("Refused pairing from \(e.name): certificate differs from the one pinned earlier. Forget the device first if this is intentional (e.g. it was reset).")
            try? await tls.send(Proto.pairResponse(accepted: false))
            return
        }

        let name = existing?.name ?? "Device @ \(ip)"
        let sas = Sas.compute(fingerprint, peerFp)
        let accepted = await ask(IncomingPairingRequest(deviceId: deviceId, deviceName: name, address: ip, sas: sas))

        try? await tls.send(Proto.pairResponse(accepted: accepted))
        guard accepted else {
            delegate?.connectLog("Pairing request from \(name) was declined or timed out")
            return
        }
        store.markPaired(id: deviceId, fallbackName: name, address: ip, tcpPort: existing?.tcpPort ?? Int(Proto.pairingTCPPort), fingerprint: peerFp)
        store.updateAddress(id: deviceId, address: ip)
        delegate?.connectLog("Paired with \(name)")
        delegate?.connectDevicesChanged()
    }

    private func ask(_ request: IncomingPairingRequest) async -> Bool {
        guard let delegate else { return false }
        await gate.acquire() // one dialog at a time
        let answer = await withTaskGroup(of: Bool.self) { group -> Bool in
            group.addTask { await delegate.connectIncomingPairing(request) }
            group.addTask {
                try? await Task.sleep(nanoseconds: UInt64(Proto.pairingDecisionTimeout * 1_000_000_000))
                return false
            }
            let first = await group.next() ?? false
            group.cancelAll()
            return first
        }
        await gate.release()
        return answer
    }

    // MARK: outgoing

    private func connectTLS(_ dev: DeviceInfo) async throws -> (TLSConnection, String) {
        do {
            let tls = try await TLSConnection.connect(host: dev.address, port: dev.tcpPort, identity: identity, timeout: 10)
            guard let fp = tls.peerFingerprint else {
                tls.close()
                throw ConnectError.message("TLS handshake completed without a peer certificate")
            }
            return (tls, fp)
        } catch let e as ConnectError {
            if case .message(let m) = e, m == "Timed out" { throw ConnectError.message("Timed out connecting to \(dev.name) (\(dev.address):\(dev.tcpPort))") }
            throw ConnectError.message("Could not connect to \(dev.name) (\(dev.address):\(dev.tcpPort)): \(e.localizedDescription)")
        }
    }

    /// Pairs with a discovered device (another Blue Connect client, or the
    /// desktop while its "Listen" button is active). Reports the SAS through
    /// the delegate, then waits up to two minutes for the other person to accept.
    public func pair(deviceId id: String) async throws {
        guard let dev = store.get(id) else { throw ConnectError.message("Unknown device — scan first") }
        let (tls, peerFp) = try await connectTLS(dev)
        defer { tls.close() }

        if dev.paired, let pinned = dev.pinnedCertSha256, pinned != peerFp {
            throw ConnectError.message("Refusing to pair: \(dev.name) presented a different certificate than the one pinned when it was last paired. This can mean the device was reset, or that something is impersonating it — Forget the device and re-pair only if you are sure.")
        }

        delegate?.connectOutgoingSAS(OutgoingPairingSAS(deviceId: dev.id, deviceName: dev.name, sas: Sas.compute(fingerprint, peerFp)))
        try await tls.send(Proto.pairRequest(deviceId: deviceId)) // single send = single TLS record

        tls.readTimeout = Proto.pairingDecisionTimeout
        let line: Data?
        do { line = try await tls.makeReader().readLine() }
        catch { throw ConnectError.message("\(dev.name) did not respond to the pairing request in time") }
        guard let line else { throw ConnectError.message("\(dev.name) closed the connection without responding") }

        let accepted = (Proto.parse(line)?.body["pair"] as? Bool) ?? false
        guard accepted else { throw ConnectError.message("Pairing was declined on \(dev.name)") }

        store.markPaired(id: dev.id, fallbackName: dev.name, address: dev.address, tcpPort: dev.tcpPort, fingerprint: peerFp)
        delegate?.connectLog("Paired with \(dev.name)")
        delegate?.connectDevicesChanged()
    }

    /// Opens a connection to a PAIRED device and verifies the pinned certificate.
    private func openAuthenticated(_ id: String) async throws -> (TLSConnection, DeviceInfo) {
        guard let dev = store.get(id), dev.paired, let pinned = dev.pinnedCertSha256 else {
            throw ConnectError.message("That device is not paired — pair it first")
        }
        let (tls, fp) = try await connectTLS(dev)
        guard fp == pinned else {
            tls.close()
            throw ConnectError.message("Refusing to talk to \(dev.name): it presented a different certificate than the one pinned when it was paired. Forget the device and re-pair only if you are sure it is really that device.")
        }
        return (tls, dev)
    }

    public func ping(deviceId id: String, message: String? = nil) async throws {
        let (tls, _) = try await openAuthenticated(id)
        defer { tls.close() }
        var body: [String: Any] = [:]
        if let message, !message.isEmpty { body["message"] = message }
        try await tls.send(Proto.frame(type: Proto.typePing, body: body))
    }

    /// Asks a paired phone to send an SMS (phone must run Blue Connect for Android).
    public func sendSMS(deviceId id: String, phoneNumber: String, body: String) async throws {
        let (tls, _) = try await openAuthenticated(id)
        defer { tls.close() }
        try await tls.send(Proto.frame(type: Proto.typeSMSRequest, body: ["sendSms": true, "phoneNumber": phoneNumber, "messageBody": body]))
    }

    /// Requests the phone's SMS history (address + body pairs).
    public func requestSMS(deviceId id: String, timeout: TimeInterval = 10) async throws -> [SMSMessage] {
        let (tls, dev) = try await openAuthenticated(id)
        defer { tls.close() }
        try await tls.send(Proto.frame(type: Proto.typeSMSRequest, body: ["requestAllConversations": true]))
        tls.readTimeout = timeout
        let line: Data?
        do { line = try await tls.makeReader().readLine() }
        catch { throw ConnectError.message("\(dev.name) did not reply with SMS history in time") }
        guard let line else { throw ConnectError.message("\(dev.name) closed the connection without replying") }
        let rows = (Proto.parse(line)?.body["messages"] as? [[String: Any]]) ?? []
        return rows.compactMap { row in
            guard let body = row["body"] as? String else { return nil }
            return SMSMessage(address: row["address"] as? String ?? "", body: body)
        }
    }
}
