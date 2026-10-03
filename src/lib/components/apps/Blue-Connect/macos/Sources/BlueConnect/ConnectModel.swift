import BlueConnectCore
import Foundation
import SwiftUI

/// Bridges the UI-free [ConnectService] to SwiftUI: owns the service and
/// republishes everything the screens show on the main actor.
@MainActor
final class ConnectModel: ObservableObject, ConnectDelegate {
    @Published var devices: [DeviceInfo] = []
    @Published var incoming: IncomingPairingRequest?
    @Published var outgoing: OutgoingPairingSAS?
    @Published var message: String?
    @Published var running = false
    @Published var scanning = false
    @Published var busyId: String?
    @Published var fingerprint = ""
    @Published var deviceName = Host.current().localizedName ?? "Mac"

    private var service: ConnectService?
    private var pendingDecision: CheckedContinuation<Bool, Never>?
    private var decisionTimeout: Task<Void, Never>?

    func start() async {
        guard service == nil else { return }
        Notifier.requestPermission()
        do {
            let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
                .appendingPathComponent("Blue-Environment/Connect", isDirectory: true)
            let identity = try TLSIdentity.loadOrCreate()
            let svc = ConnectService(
                options: ConnectOptions(dataDirectory: base, deviceName: deviceName, deviceType: "laptop"),
                identity: identity
            )
            svc.delegate = self
            try await svc.start()
            service = svc
            fingerprint = svc.fingerprint
            running = true
            devices = svc.devices
        } catch {
            message = "Could not start Blue Connect: \(error.localizedDescription). Is another app (e.g. KDE Connect) using TCP 1717 / UDP 1716?"
        }
    }

    func stop() {
        service?.stop()
        service = nil
        running = false
    }

    // MARK: actions

    func scan() {
        guard let service, !scanning else { return }
        scanning = true
        Task {
            devices = await service.scan(duration: 3)
            scanning = false
            if devices.isEmpty { message = "No devices found. Make sure Blue Connect is open on the other device and both are on the same network." }
        }
    }

    func pair(_ device: DeviceInfo) {
        guard let service else { return }
        busyId = device.id
        Task {
            do { try await service.pair(deviceId: device.id); message = "Paired with \(device.name)" }
            catch { message = error.localizedDescription }
            outgoing = nil
            busyId = nil
            devices = service.devices
        }
    }

    func ping(_ device: DeviceInfo) {
        guard let service else { return }
        busyId = device.id
        Task {
            do { try await service.ping(deviceId: device.id, message: "Hello from \(deviceName)"); message = "Ping sent to \(device.name)" }
            catch { message = error.localizedDescription }
            busyId = nil
        }
    }

    func forget(_ device: DeviceInfo) {
        service?.forget(device.id)
        devices = service?.devices ?? []
    }

    func smsService() -> ConnectService? { service }

    // MARK: pairing answer

    func answerIncoming(_ accept: Bool) {
        decisionTimeout?.cancel()
        incoming = nil
        pendingDecision?.resume(returning: accept)
        pendingDecision = nil
    }

    // MARK: ConnectDelegate (called from any thread)

    nonisolated func connectDevicesChanged() {
        Task { @MainActor in self.devices = self.service?.devices ?? [] }
    }

    nonisolated func connectIncomingPairing(_ request: IncomingPairingRequest) async -> Bool {
        await withCheckedContinuation { (cont: CheckedContinuation<Bool, Never>) in
            Task { @MainActor in
                self.incoming = request
                self.pendingDecision = cont
                Notifier.post(title: "Pairing request from \(request.deviceName)", body: "Code \(request.sas) — open Blue Connect to compare and accept")
                // Same two-minute limit as the service; also closes the sheet.
                self.decisionTimeout = Task { @MainActor in
                    try? await Task.sleep(nanoseconds: UInt64(Proto.pairingDecisionTimeout * 1_000_000_000))
                    if !Task.isCancelled, self.incoming == request { self.answerIncoming(false) }
                }
            }
        }
    }

    nonisolated func connectOutgoingSAS(_ info: OutgoingPairingSAS) {
        Task { @MainActor in self.outgoing = info }
    }

    nonisolated func connectPing(from device: DeviceInfo, message: String?) {
        Notifier.post(title: "Ping from \(device.name)", body: (message?.isEmpty == false ? message : nil) ?? "Ping!")
    }

    nonisolated func connectLog(_ message: String) {
        Task { @MainActor in self.message = message }
    }
}
