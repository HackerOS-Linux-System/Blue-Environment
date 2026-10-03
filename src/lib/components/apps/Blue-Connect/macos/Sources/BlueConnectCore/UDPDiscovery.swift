import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// UDP side of Blue Connect (POSIX sockets: Network.framework has no good
/// story for subnet broadcasts).
///
///  * The listener (bound to the discovery port) records every identity it
///    hears and answers with our own identity, unicast to the sender's source
///    port — exactly what the desktop's `bc_start_discovery` waits for (it
///    broadcasts from an ephemeral port and reads the replies).
///  * A scan broadcasts our identity from an ephemeral port and collects the
///    replies of other Blue Connect clients.
///
/// Reply storms are avoided by never answering a datagram whose source port is
/// the discovery port itself (that is another listener's reply, not a scan).
final class UDPDiscovery {
    private let discoveryPort: UInt16
    private let scanTargetPort: UInt16
    private let explicitTargets: [String]?
    private let ownId: () -> String
    private let ownIdentity: () -> Data
    private let onSeen: (Proto.Identity, String) -> Void
    private let log: (String) -> Void

    private var listenerFD: Int32 = -1
    private let running = Locked(false)

    init(discoveryPort: UInt16, scanTargetPort: UInt16?, targets: [String]?, ownId: @escaping () -> String,
         ownIdentity: @escaping () -> Data, onSeen: @escaping (Proto.Identity, String) -> Void, log: @escaping (String) -> Void) {
        self.discoveryPort = discoveryPort
        self.scanTargetPort = scanTargetPort ?? discoveryPort
        self.explicitTargets = targets
        self.ownId = ownId; self.ownIdentity = ownIdentity; self.onSeen = onSeen; self.log = log
    }

    // MARK: sockets

    private static func makeSocket(bindPort: UInt16, reuse: Bool) -> Int32? {
        let fd = socket(AF_INET, SOCK_DGRAM, 0)
        if fd < 0 { return nil }
        var yes: Int32 = 1
        let size = socklen_t(MemoryLayout<Int32>.size)
        setsockopt(fd, SOL_SOCKET, SO_BROADCAST, &yes, size)
        if reuse {
            setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, size)
            setsockopt(fd, SOL_SOCKET, SO_REUSEPORT, &yes, size)
        }
        var tv = timeval(tv_sec: 0, tv_usec: 300_000) // wake up regularly so stop()/deadlines work
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))

        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = bindPort.bigEndian
        addr.sin_addr.s_addr = INADDR_ANY
        let ok = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
        }
        if ok != 0 { close(fd); return nil }
        return fd
    }

    private static func makeAddress(host: String, port: UInt16) -> sockaddr_in? {
        var a = sockaddr_in()
        a.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        a.sin_family = sa_family_t(AF_INET)
        a.sin_port = port.bigEndian
        guard inet_pton(AF_INET, host, &a.sin_addr) == 1 else { return nil }
        return a
    }

    private static func send(_ fd: Int32, _ data: Data, to dest: sockaddr_in) {
        var d = dest
        data.withUnsafeBytes { raw in
            withUnsafePointer(to: &d) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                    _ = sendto(fd, raw.baseAddress, data.count, 0, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
                }
            }
        }
    }

    /// Blocks up to the socket's receive timeout; nil on timeout/error.
    private static func receive(_ fd: Int32) -> (Data, sockaddr_in)? {
        var buf = [UInt8](repeating: 0, count: 4096)
        var from = sockaddr_in()
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        let n = withUnsafeMutablePointer(to: &from) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { recvfrom(fd, &buf, buf.count, 0, $0, &len) }
        }
        if n <= 0 { return nil }
        return (Data(buf[0..<n]), from)
    }

    private static func ipString(_ a: sockaddr_in) -> String {
        var addr = a.sin_addr
        var buf = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
        inet_ntop(AF_INET, &addr, &buf, socklen_t(INET_ADDRSTRLEN))
        return String(cString: buf)
    }

    /// Directed broadcast address of every active IPv4 interface.
    private static func interfaceBroadcasts() -> [String] {
        var result: [String] = []
        var head: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&head) == 0, let first = head else { return result }
        defer { freeifaddrs(head) }
        var p: UnsafeMutablePointer<ifaddrs>? = first
        while let cur = p {
            let flags = Int32(cur.pointee.ifa_flags)
            if (flags & IFF_UP) != 0, (flags & IFF_BROADCAST) != 0, (flags & IFF_LOOPBACK) == 0,
               let sa = cur.pointee.ifa_dstaddr, sa.pointee.sa_family == sa_family_t(AF_INET) {
                let b = sa.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { ipString($0.pointee) }
                if !result.contains(b) { result.append(b) }
            }
            p = cur.pointee.ifa_next
        }
        return result
    }

    // MARK: listener

    func startListener() {
        guard let fd = Self.makeSocket(bindPort: discoveryPort, reuse: true) else {
            log("Discovery listener could not start on UDP \(discoveryPort) (another KDE Connect / Blue Connect instance?). Scanning still works.")
            return
        }
        listenerFD = fd
        running.set(true)
        Thread.detachNewThread { [self] in
            while running.get() {
                guard let (data, from) = Self.receive(fd) else { continue }
                guard let id = Proto.parseIdentity(data), id.deviceId != ownId() else { continue }
                let ip = Self.ipString(from)
                onSeen(id, ip)
                if UInt16(bigEndian: from.sin_port) != discoveryPort {
                    Self.send(fd, ownIdentity(), to: from)
                }
            }
            close(fd)
        }
    }

    func stop() { running.set(false) }

    // MARK: scan

    /// Broadcast our identity and gather replies for `duration`. Blocking — call off the main thread.
    func scanBlocking(duration: TimeInterval) {
        guard let fd = Self.makeSocket(bindPort: 0, reuse: false) else { log("Could not open a UDP socket to scan"); return }
        defer { close(fd) }
        let me = ownIdentity()
        let targets = explicitTargets ?? (["255.255.255.255"] + Self.interfaceBroadcasts())
        for t in targets {
            if let dest = Self.makeAddress(host: t, port: scanTargetPort) { Self.send(fd, me, to: dest) }
        }
        let deadline = Date().addingTimeInterval(duration)
        while Date() < deadline {
            guard let (data, from) = Self.receive(fd) else { continue }
            if let id = Proto.parseIdentity(data), id.deviceId != ownId() { onSeen(id, Self.ipString(from)) }
        }
    }
}
