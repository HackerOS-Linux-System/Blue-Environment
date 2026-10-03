import BlueConnectCore
import SwiftUI

private func spaced(_ sas: String) -> String { sas.count == 6 ? "\(sas.prefix(3)) \(sas.suffix(3))" : sas }

struct ContentView: View {
    @EnvironmentObject var model: ConnectModel
    @State private var smsDevice: DeviceInfo?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Blue Connect").font(.largeTitle.bold())
                Text(model.running ? "Visible as “\(model.deviceName)” on this network" : "Not running")
                    .foregroundStyle(.secondary)
            }

            HStack {
                Button(model.scanning ? "Scanning…" : "Scan for devices") { model.scan() }
                    .disabled(!model.running || model.scanning)
                    .keyboardShortcut("r")
                if model.scanning { ProgressView().controlSize(.small) }
            }

            if let m = model.message {
                Text(m).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }

            if let o = model.outgoing {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Confirm on “\(o.deviceName)”").font(.headline)
                    Text(spaced(o.sas)).font(.system(size: 34, weight: .bold, design: .monospaced))
                    Text("Check that the other device shows the same code, then accept the request there.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 10))
            }

            Text("Devices").font(.headline)
            if model.devices.isEmpty {
                Text("Nothing found yet. On your other device open Blue Connect and press Scan — or press “Scan for devices” here.")
                    .foregroundStyle(.secondary)
            }
            List(model.devices) { d in
                DeviceRow(device: d, busy: model.busyId == d.id,
                          onPair: { model.pair(d) }, onPing: { model.ping(d) },
                          onForget: { model.forget(d) }, onSMS: { smsDevice = d })
            }
            .listStyle(.inset)

            if !model.fingerprint.isEmpty {
                Text("This Mac's certificate: \(model.fingerprint.prefix(16))…")
                    .font(.system(size: 11, design: .monospaced)).foregroundStyle(.secondary)
            }
        }
        .padding(20)
        .sheet(item: Binding(get: { model.incoming }, set: { if $0 == nil, model.incoming != nil { model.answerIncoming(false) } })) { req in
            IncomingPairingSheet(request: req)
        }
        .sheet(item: $smsDevice) { dev in
            SMSSheet(device: dev, service: model.smsService())
        }
    }
}

struct DeviceRow: View {
    let device: DeviceInfo
    let busy: Bool
    let onPair: () -> Void
    let onPing: () -> Void
    let onForget: () -> Void
    let onSMS: () -> Void

    private var isPhone: Bool { device.deviceType == "phone" || device.deviceType == "tablet" }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(device.name).font(.headline)
            Text("\(device.deviceType) · \(device.address):\(device.tcpPort)\(device.paired ? " · paired" : "")")
                .font(.caption).foregroundStyle(.secondary)
            HStack {
                if device.paired {
                    Button("Ping", action: onPing).disabled(busy)
                    if isPhone { Button("Phone SMS…", action: onSMS) }
                    Button("Forget", role: .destructive, action: onForget).disabled(busy)
                } else {
                    Button(busy ? "Waiting…" : "Pair", action: onPair).disabled(busy)
                    Button("Remove", role: .destructive, action: onForget).disabled(busy)
                }
            }
        }
        .padding(.vertical, 4)
    }
}

struct IncomingPairingSheet: View {
    @EnvironmentObject var model: ConnectModel
    let request: IncomingPairingRequest

    var body: some View {
        VStack(spacing: 12) {
            Text("Pair with “\(request.deviceName)”?").font(.title2.bold())
            Text("A device at \(request.address) wants to pair with this Mac.").foregroundStyle(.secondary)
            Text(spaced(request.sas)).font(.system(size: 44, weight: .bold, design: .monospaced))
                .padding().background(.quaternary, in: RoundedRectangle(cornerRadius: 12))
            Text("Pair only if this code is EXACTLY the same on the other device. If it differs, someone may be intercepting the connection.")
                .font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
            HStack {
                Button("Reject", role: .cancel) { model.answerIncoming(false) }
                Button("Codes match — pair") { model.answerIncoming(true) }.keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 420)
        .interactiveDismissDisabled()
    }
}

struct SMSSheet: View {
    @Environment(\.dismiss) private var dismiss
    let device: DeviceInfo
    let service: ConnectService?

    @State private var number = ""
    @State private var text = ""
    @State private var status = ""
    @State private var messages: [SMSMessage] = []
    @State private var busy = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("SMS via \(device.name)").font(.title2.bold())
            TextField("Phone number, e.g. +48 600 100 200", text: $number)
            TextEditor(text: $text).frame(height: 70).border(.quaternary)
            HStack {
                Button("Send") { send() }.disabled(busy || number.isEmpty || text.isEmpty)
                Button("Load phone messages") { load() }.disabled(busy)
                Spacer()
                Button("Close") { dismiss() }
            }
            Text(status).font(.callout).foregroundStyle(.secondary)
            List(messages) { m in
                VStack(alignment: .leading) {
                    Text(m.address).font(.headline)
                    Text(m.body).foregroundStyle(.secondary)
                }
            }
        }
        .padding(20)
        .frame(width: 480, height: 540)
    }

    private func send() {
        guard let service else { return }
        busy = true
        Task {
            do {
                try await service.sendSMS(deviceId: device.id, phoneNumber: number, body: text)
                status = "Asked \(device.name) to send the message to \(number)."
                text = ""
            } catch { status = error.localizedDescription }
            busy = false
        }
    }

    private func load() {
        guard let service else { return }
        busy = true
        status = "Asking the phone…"
        Task {
            do {
                messages = try await service.requestSMS(deviceId: device.id)
                status = messages.isEmpty ? "The phone returned no messages (is SMS access allowed in Blue Connect on the phone?)." : "\(messages.count) message(s), newest first."
            } catch { status = error.localizedDescription }
            busy = false
        }
    }
}
