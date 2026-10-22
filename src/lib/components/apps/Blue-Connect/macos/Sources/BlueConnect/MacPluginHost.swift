import AppKit
import BlueConnectCore
import Foundation
import IOKit.ps

/// macOS implementation of `PluginHost`: lets a paired phone read this Mac's battery, make it
/// ring, push clipboard text, open a link here or drop text into a notification.
final class MacPluginHost: PluginHost {
    private var ringing = false
    private let lock = NSLock()

    func batteryStatus() -> BatteryStatus? {
        guard let info = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let list = IOPSCopyPowerSourcesList(info)?.takeRetainedValue() as? [CFTypeRef] else { return nil }
        for source in list {
            guard let d = IOPSGetPowerSourceDescription(info, source)?.takeUnretainedValue() as? [String: Any],
                  (d[kIOPSTypeKey] as? String) == kIOPSInternalBatteryType,
                  let current = d[kIOPSCurrentCapacityKey] as? Int,
                  let max = d[kIOPSMaxCapacityKey] as? Int, max > 0 else { continue }
            let charging = (d[kIOPSIsChargingKey] as? Bool) ?? ((d[kIOPSPowerSourceStateKey] as? String) == kIOPSACPowerValue)
            return BatteryStatus(percent: current * 100 / max, charging: charging)
        }
        return nil // a Mac without a battery
    }

    /// Plays the alert sound for ~15 s and bounces the Dock icon, unless it is already ringing.
    func ring() {
        lock.lock()
        if ringing { lock.unlock(); return }
        ringing = true
        lock.unlock()
        Notifier.post(title: "Find my device", body: "A paired device is looking for this Mac.")
        Task { @MainActor in
            NSApp.requestUserAttention(.criticalRequest)
            for _ in 0..<15 {
                (NSSound(named: "Sosumi") ?? NSSound(named: "Funk"))?.play()
                try? await Task.sleep(nanoseconds: 1_000_000_000)
            }
            self.lock.lock(); self.ringing = false; self.lock.unlock()
        }
    }

    func setClipboard(_ text: String) {
        DispatchQueue.main.async {
            let pb = NSPasteboard.general
            pb.clearContents()
            pb.setString(text, forType: .string)
        }
    }

    func currentClipboard() -> String? {
        var result: String?
        let read = { result = NSPasteboard.general.string(forType: .string) }
        if Thread.isMainThread { read() } else { DispatchQueue.main.sync(execute: read) }
        return result
    }

    /// The core has already restricted this to http/https/mailto/tel.
    func open(url: String, from: DeviceInfo) {
        guard let u = URL(string: url) else { return }
        DispatchQueue.main.async { NSWorkspace.shared.open(u) }
    }

    func receive(text: String, from: DeviceInfo) {
        setClipboard(text)
        let preview = text.count > 120 ? String(text.prefix(120)) + "…" : text
        Notifier.post(title: "Text from \(from.name)", body: preview + "\n(copied to the clipboard)")
    }

    // macOS has no public API to read other apps' notifications.
    func currentNotifications() -> [NotificationInfo] { [] }
}
