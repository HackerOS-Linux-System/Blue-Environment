import Foundation

/// Reads newline-terminated JSON packets from an async byte source. Keeps
/// leftover bytes between calls (a peer may send several packets back to back)
/// and tolerates a final packet that is not newline terminated before EOF.
public final class LineReader {
    private var buffer = Data()
    private var eof = false
    private let readChunk: () async throws -> Data? // nil = end of stream

    public init(readChunk: @escaping () async throws -> Data?) { self.readChunk = readChunk }

    /// The next packet as raw JSON bytes, or nil at end of stream.
    public func readLine() async throws -> Data? {
        while true {
            if let nl = buffer.firstIndex(of: 0x0A) {
                let line = Data(buffer[buffer.startIndex..<nl])
                buffer = Data(buffer[buffer.index(after: nl)...])
                if line.allSatisfy({ $0 == 0x20 || $0 == 0x0D || $0 == 0x09 }) { continue } // blank keep-alive
                return line
            }
            if eof {
                if buffer.isEmpty { return nil }
                let rest = buffer
                buffer = Data()
                return rest.allSatisfy({ $0 == 0x20 || $0 == 0x0D || $0 == 0x09 || $0 == 0x0A }) ? nil : rest
            }
            if buffer.count > Proto.maxPacketBytes { throw ConnectError.message("Packet too large") }
            if let chunk = try await readChunk() { buffer.append(chunk) } else { eof = true }
        }
    }
}
