# Blue Environment — Changelog

## v0.4.0 (2026-05)

### Compositor (blue-compositor)
- Production Smithay Wayland compositor with DRM/KMS backend
- Nested mode (winit) for VM/existing DE
- XWayland support (X11 app compatibility)
- Layer Shell, xdg-activation, fractional-scale, viewporter protocols
- IPC Unix socket (blue-compositor.sock)
- VT switch via libseat
- Uses smithay::reexports::calloop (no version conflict)

### Shell (blue-environment)
- 4 virtual workspaces
- PowerOverlay (shutdown/restart/suspend/hibernate/logout)
- LockScreen
- ErrorBoundary — app crashes isolated
- App enable/disable (Settings → Applications)
- TopBar with pinned apps, workspace, weather, clipboard, clock
- StartMenu with category view + full-screen picker
- ControlCenter, NotificationCenter, ClipboardPanel

### Applications
- BlueAI: ChatGPT/Claude/Gemini/DeepSeek/Ollama/Grok
- BlueCode: Monaco + xterm.js
- TerminalApp: xterm.js with tabs and themes
- MailApp: full client with folders, reply, CC
- SettingsApp: display, personalization, Wi-Fi, Bluetooth, power, apps
- AboutApp: reads /etc/xdg/kcm-about-distrorc + /etc/os-release
- CalculatorApp: safe math parser (no eval())
- ExplorerApp, NotepadApp, SystemMonitorApp, BlueSoftwareApp, BlueWebApp

### Blue Launcher (Crystal)
- blue start/dev/update/install/remove/search/info

### License
GNU GPL v3.0
