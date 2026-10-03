import Foundation
import Network
import Security

/// Small thread-safe box (Network.framework callbacks run on their own queues).
final class Locked<T> {
    private let lock = NSLock()
    private var value: T
    init(_ v: T) { value = v }
    func get() -> T { lock.lock(); defer { lock.unlock() }; return value }
    func set(_ v: T) { lock.lock(); value = v; lock.unlock() }
}

/// One mutual-TLS connection (either role) on Network.framework, with an
/// async API and per-operation timeouts. The peer's certificate is read from
/// the finished session's metadata (per connection — a listener shares one
/// set of parameters between all of its connections, so nothing can be
/// captured in the verify block). It is only ever *reported*: whether to
/// trust it is decided by the caller (pin + SAS), never here.
public final class TLSConnection {
    public let connection: NWConnection
    private let queue = DispatchQueue(label: "blue-connect.tls")

    /// Per-read timeout used by readers made with [makeReader]; may be changed between reads.
    public var readTimeout: TimeInterval = 30

    init(connection: NWConnection) { self.connection = connection }

    /// DER of the certificate the other side presented (valid once the connection is ready).
    public var peerCertificateDER: Data? {
        guard let meta = connection.metadata(definition: NWProtocolTLS.definition) as? NWProtocolTLS.Metadata else { return nil }
        let first = Locked<Data?>(nil)
        _ = sec_protocol_metadata_access_peer_certificate_chain(meta.securityProtocolMetadata) { cert in
            if first.get() == nil {
                first.set(SecCertificateCopyData(sec_certificate_copy_ref(cert).takeRetainedValue()) as Data)
            }
        }
        return first.get()
    }

    public var peerFingerprint: String? { peerCertificateDER.map { Sas.fingerprint($0) } }

    public var remoteAddress: String? {
        let endpoint = connection.currentPath?.remoteEndpoint ?? connection.endpoint
        if case let .hostPort(host, _) = endpoint {
            switch host {
            case .ipv4(let a): return "\(a)".components(separatedBy: "%").first
            case .ipv6(let a): return "\(a)".components(separatedBy: "%").first
            case .name(let n, _): return n
            @unknown default: return nil
            }
        }
        return nil
    }

    /// TLS parameters presenting our certificate. Peer authentication is
    /// REQUIRED (a server then asks the client for its certificate — that is
    /// what makes the channel mutual) but any self-signed certificate passes
    /// the handshake; trust is decided later by the caller (pin + SAS).
    static func parameters(identity: TLSIdentity) -> NWParameters {
        let tls = NWProtocolTLS.Options()
        let sec = tls.securityProtocolOptions
        if let secIdentity = sec_identity_create(identity.identity) {
            sec_protocol_options_set_local_identity(sec, secIdentity)
        }
        sec_protocol_options_set_min_tls_protocol_version(sec, .TLSv12)
        sec_protocol_options_set_peer_authentication_required(sec, true)
        sec_protocol_options_set_verify_block(sec, { _, _, complete in complete(true) }, DispatchQueue(label: "blue-connect.verify"))
        return NWParameters(tls: tls, tcp: NWProtocolTCP.Options())
    }

    /// Outgoing connection (we are the TLS client).
    public static func connect(host: String, port: Int, identity: TLSIdentity, timeout: TimeInterval) async throws -> TLSConnection {
        guard let p = NWEndpoint.Port(rawValue: UInt16(port)) else { throw ConnectError.message("Invalid port \(port)") }
        let conn = NWConnection(host: NWEndpoint.Host(host), port: p, using: parameters(identity: identity))
        let wrapper = TLSConnection(connection: conn)
        try await wrapper.waitUntilReady(timeout: timeout)
        return wrapper
    }

    /// Wraps a connection accepted by the listener (we are the TLS server).
    static func accepted(_ conn: NWConnection, timeout: TimeInterval) async throws -> TLSConnection {
        let wrapper = TLSConnection(connection: conn)
        try await wrapper.waitUntilReady(timeout: timeout)
        return wrapper
    }

    private func waitUntilReady(timeout: TimeInterval) async throws {
        let timedOut = Locked(false)
        let timer = DispatchWorkItem { [connection] in timedOut.set(true); connection.cancel() }
        queue.asyncAfter(deadline: .now() + timeout, execute: timer)
        defer { timer.cancel() }

        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            let resumed = Locked(false)
            func finish(_ result: Result<Void, Error>) {
                if resumed.get() { return }
                resumed.set(true)
                cont.resume(with: result)
            }
            connection.stateUpdateHandler = { [connection] state in
                switch state {
                case .ready:
                    finish(.success(()))
                case .failed(let e):
                    finish(.failure(ConnectError.message(timedOut.get() ? "Timed out" : e.localizedDescription)))
                case .cancelled:
                    finish(.failure(ConnectError.message(timedOut.get() ? "Timed out" : "Connection closed")))
                case .waiting(let e):
                    // e.g. connection refused — Network.framework would retry forever, we do not.
                    finish(.failure(ConnectError.message(e.localizedDescription)))
                    connection.cancel()
                default:
                    break
                }
            }
            connection.start(queue: queue)
        }
        connection.stateUpdateHandler = nil
    }

    /// One write = one TLS record for packets under 16 KB (the desktop reads replies with a single `read`).
    public func send(_ data: Data) async throws {
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            connection.send(content: data, completion: .contentProcessed { error in
                if let error { cont.resume(throwing: ConnectError.message(error.localizedDescription)) } else { cont.resume() }
            })
        }
    }

    /// Next chunk (nil = peer closed). On timeout the connection is cancelled and an error thrown.
    public func receive(timeout: TimeInterval) async throws -> Data? {
        let timedOut = Locked(false)
        let timer = DispatchWorkItem { [connection] in timedOut.set(true); connection.cancel() }
        queue.asyncAfter(deadline: .now() + timeout, execute: timer)
        defer { timer.cancel() }

        return try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Data?, Error>) in
            connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { data, _, isComplete, error in
                if let data, !data.isEmpty { cont.resume(returning: data) }
                else if let error { cont.resume(throwing: ConnectError.message(timedOut.get() ? "Timed out" : error.localizedDescription)) }
                else if isComplete { cont.resume(returning: nil) }
                else { cont.resume(returning: Data()) }
            }
        }
    }

    public func makeReader() -> LineReader {
        LineReader { [self] in
            while true { // an empty chunk is a spurious wake-up, keep reading
                guard let chunk = try await self.receive(timeout: self.readTimeout) else { return nil }
                if !chunk.isEmpty { return chunk }
            }
        }
    }

    public func close() { connection.cancel() }
}
