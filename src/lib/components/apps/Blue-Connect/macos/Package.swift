import PackageDescription

// Blue Connect for macOS.
//   BlueConnectCore  – protocol, TLS identity, discovery, pairing (no UI)
//   BlueConnect      – SwiftUI app (window + menu-bar item)
// Build:  swift build -c release      Test:  swift test
// Make a double-clickable app bundle:  scripts/build-app.sh
let package = Package(
    name: "BlueConnect",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "BlueConnect", targets: ["BlueConnect"]),
        .library(name: "BlueConnectCore", targets: ["BlueConnectCore"]),
    ],
    targets: [
        .target(name: "BlueConnectCore"),
        .executableTarget(name: "BlueConnect", dependencies: ["BlueConnectCore"]),
        .testTarget(name: "BlueConnectCoreTests", dependencies: ["BlueConnectCore"]),
    ]
)
