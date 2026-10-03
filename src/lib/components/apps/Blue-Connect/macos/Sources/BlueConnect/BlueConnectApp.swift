import BlueConnectCore
import SwiftUI

@main
struct BlueConnectApp: App {
    @StateObject private var model = ConnectModel()

    var body: some Scene {
        WindowGroup("Blue Connect") {
            ContentView()
                .environmentObject(model)
                .frame(minWidth: 480, minHeight: 540)
                .task { await model.start() }
        }

        MenuBarExtra("Blue Connect", systemImage: "link") {
            Text(model.running ? "Visible as “\(model.deviceName)”" : "Not running")
            Divider()
            Button("Scan for devices") { model.scan() }.disabled(!model.running)
            ForEach(model.devices.filter { $0.paired }) { d in
                Button("Ping \(d.name)") { model.ping(d) }
            }
            Divider()
            Button("Quit Blue Connect") {
                model.stop()
                NSApplication.shared.terminate(nil)
            }
        }
    }
}
