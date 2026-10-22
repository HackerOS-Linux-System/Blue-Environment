import Foundation
import Security
import XCTest
@testable import BlueConnectCore

/// The SAS vectors were produced by the desktop's own `tls.rs::compute_sas`.
final class SasTests: XCTestCase {
    func testMatchesTheDesktopImplementation() {
        XCTAssertEqual(Sas.compute("aaaa1111", "bbbb2222"), "790764")
        XCTAssertEqual(Sas.compute("x", "y"), "781997")
        XCTAssertEqual(Sas.compute("0123456789abcdef", "fedcba9876543210"), "433941")
    }

    func testIsOrderIndependent() {
        XCTAssertEqual(Sas.compute("aaaa1111", "bbbb2222"), Sas.compute("bbbb2222", "aaaa1111"))
    }

    func testFingerprintIsLowercaseSha256Hex() {
        let fp = Sas.fingerprint(Data([1, 2, 3, 4]))
        XCTAssertEqual(fp, "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a")
        XCTAssertEqual(fp.count, 64)
        XCTAssertEqual(fp, fp.lowercased())
    }
}

final class ProtocolTests: XCTestCase {
    func testIdentityRoundTrip() throws {
        let bytes = Proto.buildIdentity(deviceId: "blue-connect-abc", deviceName: "Test Mac", deviceType: "laptop", tcpPort: 1717)
        let id = try XCTUnwrap(Proto.parseIdentity(bytes))
        XCTAssertEqual(id.deviceId, "blue-connect-abc")
        XCTAssertEqual(id.deviceName, "Test Mac")
        XCTAssertEqual(id.deviceType, "laptop")
        XCTAssertEqual(id.tcpPort, 1717)
        XCTAssertEqual(id.protocolVersion, 7)
        XCTAssertNotEqual(bytes.last, 0x0A, "identity datagram has no trailing newline")
    }

    func testIncompleteOrForeignDatagramsAreIgnored() {
        XCTAssertNil(Proto.parseIdentity(Data(#"{"type":"kdeconnect.identity","body":{"deviceId":"x"}}"#.utf8)))
        XCTAssertNil(Proto.parseIdentity(Data(#"{"type":"kdeconnect.pair","body":{}}"#.utf8)))
        XCTAssertNil(Proto.parseIdentity(Data("not json".utf8)))
        let badPort = #"{"type":"kdeconnect.identity","body":{"deviceId":"x","deviceName":"n","deviceType":"phone","protocolVersion":7,"tcpPort":70000}}"#
        XCTAssertNil(Proto.parseIdentity(Data(badPort.utf8)))
    }

    func testFramedPairRequest() throws {
        let framed = Proto.pairRequest(deviceId: "dev1")
        XCTAssertEqual(framed.last, 0x0A)
        let p = try XCTUnwrap(Proto.parse(Data(framed.dropLast())))
        XCTAssertEqual(p.type, Proto.typePair)
        XCTAssertEqual(p.body["pair"] as? Bool, true)
        XCTAssertEqual(p.body["deviceId"] as? String, "dev1")
    }

    func testUnknownDeviceType() {
        XCTAssertEqual(Proto.normalizeDeviceType("toaster"), "unknown")
        XCTAssertEqual(Proto.normalizeDeviceType(" Laptop "), "laptop")
    }
}

final class DeviceStoreTests: XCTestCase {
    private func tmp() -> URL {
        let d = FileManager.default.temporaryDirectory.appendingPathComponent("bc-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d
    }

    func testReadsTheDesktopsDevicesJson() throws {
        let dir = tmp()
        let json = #"{"d1":{"id":"d1","name":"Phone","deviceType":"phone","address":"192.168.1.5","tcpPort":1717,"paired":true,"pinnedCertSha256":"abc"},"d2":{"id":"d2","name":"Old","deviceType":"desktop","address":"10.0.0.2","tcpPort":1717,"paired":false,"pinnedCertSha256":null}}"#
        try json.write(to: dir.appendingPathComponent("devices.json"), atomically: true, encoding: .utf8)
        let store = DeviceStore(directory: dir)
        XCTAssertEqual(store.all().map(\.id), ["d1", "d2"]) // paired first
        XCTAssertEqual(store.findPaired(byFingerprint: "abc")?.id, "d1")
        XCTAssertNil(store.get("d2")?.pinnedCertSha256)
        XCTAssertNil(store.findPaired(byFingerprint: "zzz"))
    }

    func testSeenNeverTouchesPairingStateAndChangesPersist() {
        let dir = tmp()
        let store = DeviceStore(directory: dir)
        store.markPaired(id: "d1", fallbackName: "First", address: "1.1.1.1", tcpPort: 1717, fingerprint: "fp1")
        store.seen(id: "d1", name: "Renamed", type: "laptop", address: "2.2.2.2", tcpPort: 1800)
        let again = DeviceStore(directory: dir).get("d1")
        XCTAssertEqual(again?.name, "Renamed")
        XCTAssertEqual(again?.tcpPort, 1800)
        XCTAssertEqual(again?.paired, true)
        XCTAssertEqual(again?.pinnedCertSha256, "fp1")
        XCTAssertTrue(store.forget("d1"))
        XCTAssertFalse(store.forget("d1"))
    }

    func testWritesTheDesktopsFieldNamesAndExplicitNull() throws {
        let dir = tmp()
        DeviceStore(directory: dir).seen(id: "d1", name: "N", type: "phone", address: "1.1.1.1", tcpPort: 1717)
        let text = try String(contentsOf: dir.appendingPathComponent("devices.json"), encoding: .utf8)
        XCTAssertTrue(text.contains("\"pinnedCertSha256\" : null"))
        XCTAssertTrue(text.contains("\"deviceType\"") && text.contains("\"tcpPort\""))
    }
}

final class LineReaderTests: XCTestCase {
    func testBackToBackPacketsBlankLinesAndUnterminatedLast() async throws {
        var chunks: [Data] = [
            Data("{\"type\":\"a\",\"body\":{}}\n\n{\"type\":\"b\"".utf8),
            Data(",\"body\":{}}\n{\"type\":\"c\",\"body\":{}}".utf8),
        ]
        let reader = LineReader { chunks.isEmpty ? nil : chunks.removeFirst() }
        let a = try await reader.readLine(), b = try await reader.readLine(), c = try await reader.readLine(), end = try await reader.readLine()
        XCTAssertTrue(String(decoding: a ?? Data(), as: UTF8.self).contains("\"a\""))
        XCTAssertTrue(String(decoding: b ?? Data(), as: UTF8.self).contains("\"b\"")) // packet split across two reads
        XCTAssertTrue(String(decoding: c ?? Data(), as: UTF8.self).contains("\"c\""))
        XCTAssertNil(end)
    }
}

final class DERTests: XCTestCase {
    func testLengthEncoding() {
        XCTAssertEqual(DER.length(5), [0x05])
        XCTAssertEqual(DER.length(128), [0x81, 0x80])
        XCTAssertEqual(DER.length(300), [0x82, 0x01, 0x2C])
    }

    func testOIDEncoding() {
        XCTAssertEqual(X509.ecdsaWithSHA256, [0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x03, 0x02])
    }

    func testIntegerKeepsItPositive() {
        XCTAssertEqual(DER.integer([0x80]), [0x02, 0x02, 0x00, 0x80])
        XCTAssertEqual(DER.integer([0x00, 0x00, 0x7F]), [0x02, 0x01, 0x7F])
    }

    /// Builds a certificate for a throw-away key (not stored in the keychain)
    /// and checks that the Security framework accepts it and sees our key.
    func testSelfSignedCertificateIsAcceptedBySecurityFramework() throws {
        let attrs: [String: Any] = [
            kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeySizeInBits as String: 256,
            kSecAttrIsPermanent as String: false,
        ]
        var err: Unmanaged<CFError>?
        let key = try XCTUnwrap(SecKeyCreateRandomKey(attrs as CFDictionary, &err))
        let cert = try TLSIdentity.makeCertificate(privateKey: key)
        XCTAssertEqual(SecCertificateCopySubjectSummary(cert) as String?, "blue-connect.local")

        let certKey = try XCTUnwrap(SecCertificateCopyKey(cert))
        let a = SecKeyCopyExternalRepresentation(certKey, nil) as Data?
        let b = SecKeyCopyExternalRepresentation(try XCTUnwrap(SecKeyCopyPublicKey(key)), nil) as Data?
        XCTAssertEqual(a, b)
        XCTAssertEqual(Sas.fingerprint(SecCertificateCopyData(cert) as Data).count, 64)
    }
}


final class PluginTests: XCTestCase {
    private final class FakeHost: PluginHost {
        var battery: BatteryStatus? = BatteryStatus(percent: 55, charging: false)
        var rings = 0
        var clipboard: String?
        var opened: [String] = []
        var texts: [String] = []
        func batteryStatus() -> BatteryStatus? { battery }
        func ring() { rings += 1 }
        func setClipboard(_ text: String) { clipboard = text }
        func currentClipboard() -> String? { clipboard }
        func open(url: String, from: DeviceInfo) { opened.append(url) }
        func receive(text: String, from: DeviceInfo) { texts.append(text) }
        func currentNotifications() -> [NotificationInfo] { [NotificationInfo(id: "1", app: "Mail", title: "Hi", text: "Body", timeMs: 1)] }
    }

    private let dev = DeviceInfo(id: "d", name: "Phone", deviceType: "phone", address: "127.0.0.1", tcpPort: 1717)

    private func run(_ host: FakeHost, _ type: String, _ body: [String: Any] = [:]) async -> Proto.Packet? {
        var out: Data?
        let handled = await PluginRouter(host: host).handle(from: dev, packet: Proto.Packet(type: type, body: body)) { out = $0 }
        _ = handled
        return out.flatMap { Proto.parse(String(decoding: $0, as: UTF8.self).trimmingCharacters(in: .newlines)) }
    }

    func testBatteryRequestIsAnswered() async throws {
        let host = FakeHost()
        let r = try XCTUnwrap(await run(host, Proto.typeBatteryRequest))
        XCTAssertEqual(PluginPackets.parseBattery(r), BatteryStatus(percent: 55, charging: false))
        host.battery = nil
        let none = try XCTUnwrap(await run(host, Proto.typeBatteryRequest))
        XCTAssertNil(PluginPackets.parseBattery(none))
    }

    func testFindMyPhoneRingsWithoutReply() async {
        let host = FakeHost()
        let r = await run(host, Proto.typeFindMyPhone)
        XCTAssertNil(r)
        XCTAssertEqual(host.rings, 1)
    }

    func testClipboardRoundTripKeepsUnicode() async throws {
        let host = FakeHost()
        _ = await run(host, Proto.typeClipboard, ["content": "zażółć 🙂"])
        XCTAssertEqual(host.clipboard, "zażółć 🙂")
        let r = try XCTUnwrap(await run(host, Proto.typeClipboardRequest))
        XCTAssertEqual(r.type, Proto.typeClipboard)
        XCTAssertEqual(r.body["content"] as? String, "zażółć 🙂")
    }

    func testOnlySafeURLsAreOpened() async {
        let host = FakeHost()
        _ = await run(host, Proto.typeShare, ["url": "https://example.org/a?b=c"])
        _ = await run(host, Proto.typeShare, ["url": "javascript:alert(1)"])
        _ = await run(host, Proto.typeShare, ["url": "file:///etc/passwd"])
        _ = await run(host, Proto.typeShare, ["text": "plain note"])
        XCTAssertEqual(host.opened, ["https://example.org/a?b=c"])
        XCTAssertEqual(host.texts, ["plain note"])
    }

    func testURLValidation() {
        XCTAssertTrue(PluginPackets.isSafeURL("mailto:a@b.c"))
        XCTAssertTrue(PluginPackets.isSafeURL("tel:+48123456789"))
        XCTAssertFalse(PluginPackets.isSafeURL("https://a b"))
        XCTAssertFalse(PluginPackets.isSafeURL("x-apple.systempreferences:"))
        XCTAssertFalse(PluginPackets.isSafeURL(""))
        XCTAssertFalse(PluginPackets.isSafeURL(nil))
        XCTAssertFalse(PluginPackets.isSafeURL("https://x/" + String(repeating: "a", count: 3000)))
    }

    func testFitUTF8NeverSplitsCharacters() {
        XCTAssertEqual(PluginPackets.fitUTF8("ą" + String(repeating: "x", count: 10), maxBytes: 2), "ą")
        XCTAssertEqual(PluginPackets.fitUTF8("🙂🙂", maxBytes: 5), "🙂")
        XCTAssertEqual(PluginPackets.fitUTF8("short", maxBytes: 100), "short")
    }

    func testNotificationListFitsOneRecord() throws {
        let big = (0..<200).map { NotificationInfo(id: "\($0)", app: "App", title: String(repeating: "t", count: 200), text: String(repeating: "b", count: 400), timeMs: Int64($0)) }
        let frame = PluginPackets.notificationList(big)
        XCTAssertLessThanOrEqual(frame.count, Proto.maxSingleReadReplyBytes)
        let parsed = PluginPackets.parseNotifications(try XCTUnwrap(Proto.parse(String(decoding: frame, as: UTF8.self).trimmingCharacters(in: .newlines))))
        XCTAssertTrue(parsed.count > 0 && parsed.count < 200)
        XCTAssertEqual(parsed.first?.id, "0")
    }

    func testIdentityAdvertisesPlugins() throws {
        let id = Proto.buildIdentity(deviceId: "id", deviceName: "n", deviceType: "laptop", tcpPort: 1717)
        let text = String(decoding: id, as: UTF8.self)
        XCTAssertTrue(text.contains(Proto.typeClipboard) && text.contains(Proto.typeFindMyPhone))
        XCTAssertNotNil(Proto.parseIdentity(id))
    }
}
