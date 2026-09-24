# ![Blue Enviroment - Graphical environment for LegendaryOS.](https://github.com/HackerOS-Linux-System/Blue-Environment/blob/main/images/banner.png)
# Blue Environment v0.7

Production-grade Wayland desktop environment for LegendaryOS, built on
[Smithay](https://github.com/Smithay/smithay) (compositor) and
[Tauri](https://tauri.app) + Svelte (desktop shell).

## Features

- **Wayland compositor** (`compositor/`) — xdg-shell, layer-shell,
  XWayland, session-lock, idle/idle-inhibit, cursor-shape,
  fractional-scale, data-device/primary-selection, pointer-constraints
  and relative-pointer (pointer lock for games), tablet input,
  text-input/input-method (IME), `wlr-foreign-toplevel-management` (native
  window list for the panel/switcher — no `wmctrl`/`xdotool` needed when
  running under HackerOS-Comp), `wlr-output-management` (multi-monitor
  configuration as a protocol), `wlr-screencopy` (native screenshot
  support). Both a nested/dev backend (winit) and a bare-metal
  DRM/KMS/libseat backend for TTY sessions.
- **Desktop shell** (`src/` + `src-tauri/`) — panel, launcher, window
  switcher, workspaces, notification center, control center, and a suite
  of first-party apps: Mail (IMAP/SMTP), Web, Docs (with PDF/DOCX
  import/export), Code editor, Terminal, File explorer, Camera, Archive
  manager, System Monitor, Partition Manager, Settings (including
  **Parental Controls**: PIN-protected app blocking, daily time limits,
  allowed-hours windows).
- **Packaging** for Debian/Ubuntu, Fedora, LegendaryOS, Arch, Alpine,
  openSUSE, Gentoo, Void, Nix, Snap, and Flatpak (the latter two/Gentoo/
  Void as submission-ready templates — see `packaging/`).

See [`ROADMAP.md`](./ROADMAP.md) for exactly what's implemented, what's
best-effort/needs on-hardware verification, and what's still planned.

## Build Instructions

### Prerequisites

```bash
# System packages (Debian/Ubuntu/HackerOS)
sudo apt install \
    build-essential curl git \
    libssl-dev libgbm-dev libseat-dev \
    libinput-dev libxkbcommon-dev \
    libudev-dev libdrm-dev \
    libgtk-3-dev libwebkit2gtk-4.0-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev pkg-config \
    seatd

# wmctrl/xdotool are OPTIONAL — only used as a fallback when the shell
# isn't actually running under HackerOS-Comp (e.g. a nested dev session
# under a different desktop environment). Under a real HackerOS-Comp
# session, window listing/focus/close/minimize all go through the
# compositor's own IPC and the wlr-foreign-toplevel-management protocol,
# so these packages aren't required for normal use.
# sudo apt install wmctrl xdotool

# Node.js 18+
curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
sudo apt install nodejs

# Rust stable
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Tauri CLI v1
cargo install tauri-cli --version "^1"

# Enable seatd (needed for DRM/bare-metal mode)
sudo systemctl enable --now seatd
sudo usermod -aG seat $USER
# (re-login after this)
```

### Build the frontend + Tauri shell (the main app)

```bash
npm install
npm run build:tauri
# This runs: npm run build  →  vite build  →  tauri build
```

> **Note:** the Wayland compositor (Smithay) described under Features
> above is planned architecture — `compositor/` doesn't exist in this
> tree yet, so there is currently no `npm run build:compositor` or
> `npm run build:all` command, and the CI workflows (`test.yml`,
> `build.yml`) don't assume it exists either (they probe for
> `compositor/Cargo.toml` and skip compositor-specific steps when it's
> absent, rather than failing). The shell above already runs standalone
> today under any existing Wayland/X11 compositor (GNOME, KDE, sway,
> ...) — it doesn't require HackerOS-Comp specifically.

### Development (hot-reload)

```bash
npm run dev          # Start Vite dev server on :1420
cargo tauri dev      # Or: npm run tauri -- dev
```

## How the build works

```
npm run build:tauri
  └─ tauri build
       ├─ beforeBuildCommand: "npm run build"
       │     ├─ tsc --noEmit   (type-check)
       │     └─ vite build     → dist/
       └─ cargo build (src-tauri/)  → blue-environment binary
```

The key insight: **`tauri build` calls `npm run build` automatically** via
`beforeBuildCommand` in `tauri.conf.json`. You should NOT call
`npm run build` manually before `npm run build:tauri`.

## Project layout

```
blue-environment/
├── index.html                     ← entry HTML (project root)
├── src/                           ← TypeScript/React frontend
│   ├── App.tsx                    ← Desktop shell
│   ├── constants.tsx              ← App registry
│   ├── types.ts                   ← All TypeScript types
│   ├── vite.config.ts             ← Vite config (root = ..)
│   ├── tsconfig.json
│   ├── index.tsx                  ← React entry point
│   ├── components/
│   │   ├── Window.tsx
│   │   ├── TopBar.tsx
│   │   ├── StartMenu.tsx
│   │   ├── ControlCenter.tsx
│   │   ├── NotificationCenter.tsx
│   │   ├── WindowSwitcher.tsx
│   │   ├── WorkspaceSwitcher.tsx
│   │   ├── ClipboardPanel.tsx
│   │   ├── ToastContainer.tsx
│   │   └── apps/
│   │       ├── BlueAI.tsx
│   │       ├── BlueCodeApp.tsx    ← Monaco + xterm
│   │       ├── BlueSoftwareApp.tsx
│   │       ├── BlueWebApp.tsx
│   │       ├── ExplorerApp.tsx
│   │       ├── MailApp.tsx        ← Full mail client
│   │       ├── SettingsApp.tsx    ← Full settings
│   │       ├── TerminalApp.tsx
│   │       ├── SystemMonitorApp.tsx
│   │       ├── NotepadApp.tsx
│   │       ├── CalculatorApp.tsx
│   │       ├── AboutApp.tsx
│   │       └── MailApp.tsx
│   ├── hooks/
│   │   ├── useWindowManager.ts
│   │   └── useKeyboardShortcuts.ts
│   ├── utils/
│   │   ├── systemBridge.ts        ← Tauri IPC bridge
│   │   ├── configStore.ts         ← Reactive config (wallpaper etc.)
│   │   └── notificationManager.ts
│   └── contexts/
│       └── LanguageContext.tsx
├── src-tauri/                     ← Rust/Tauri backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/icon.png
│   └── src/
│       ├── main.rs                ← Tauri commands
│       ├── ai.rs                  ← AI API proxy
│       ├── weather.rs             ← Weather widget backend (IP geolocation + Open-Meteo)
│       ├── parental_controls.rs   ← PIN-protected app blocking, time limits
│       ├── apps.rs                ← .desktop scanner
│       ├── cache.rs               ← Config/cache
│       ├── session.rs             ← Session detection
│       └── window_tracker.rs     ← External windows (compositor IPC first, wmctrl/xdotool fallback)
└── compositor/                    ← Smithay compositor (separate crate)
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── state/                 ← BlueState + protocol handler impls
        ├── input/                 ← libinput dispatch, move/resize grabs
        ├── render/                ← winit (nested) + DRM/KMS (bare-metal) backends
        ├── xwayland/               ← XWayland integration
        ├── ipc/                   ← Unix socket protocol to the shell
        └── protocols/              ← idle, session-lock, decoration, cursor-shape,
                                       foreign-toplevel-management, output-management,
                                       screencopy
```

## Compositor backends (hackeros-comp / labwc / sway / wayfire)

Blue Environment can run on four compositors. The choice is made in the
`[backend]` section of `config.hk` (HackerOS Configuration Format):

```
[backend]
-> compositor => wayfire          ! or: labwc, sway, hackeros-comp (the default)
-> labwc_binary => labwc          ! optional, one set per backend (<name> is
-> labwc_args =>                  ! labwc, sway or wayfire — only the active
-> labwc_config_dir =>            ! one's settings are read):
-> generate_labwc_config => true  !   <name>_binary / <name>_args /
-> sway_binary => sway            !   <name>_config_dir / generate_<name>_config
-> generate_sway_config => true
-> wayfire_binary => wayfire
-> generate_wayfire_config => true
```

**Which file?** An existing `config.hk` is used wherever it already is
(`$BLUE_CONFIG_HK`, `$XDG_CONFIG_HOME/Blue-Environment/`,
`~/.config/Blue-Environment/`, then `/etc/xdg/Blue-Environment/`). If there
is none, a default one is created at `~/.config/Blue-Environment/config.hk`.

**Start-up** — the classic `blue-environment` invocation reads that file:

| `compositor` | What happens |
|---|---|
| `hackeros-comp` | Nothing changes: the binary runs as the shell, exactly as before. |
| `labwc` / `sway` / `wayfire` | Missing config is generated (never overwriting anything — see below), then Blue becomes that compositor, which starts the shell itself. **HackerOS-Comp is not required.** |
| chosen compositor isn't installed | Warning, then the classic behaviour. |

If a display session already exists, the compositor is not nested (force
with `--start-backend`); `--no-backend` always just runs the shell;
`--backend-info` prints what was detected.

**What's shared across all three** (`src-tauri/src/backend/`) — labwc, sway
and wayfire are all wlroots-based and implement the same protocols, so this
code is written once and just works on any of them:

* window list / focus / minimize / maximize / close for every native and
  XWayland window — `wlr-foreign-toplevel-management`, pushed to the UI as
  the same `compositor:window-list` / `compositor:window-focused` events
  HackerOS-Comp emits;
* system-wide clipboard history, including copies made in external apps
  (`wl-paste --watch`, needs `wl-clipboard`);
* global shortcuts while a native app has focus: each backend's keybinds
  call `blue-environment --ctl <command>` (`toggle-start-menu`,
  `fullscreen-menu`, `toggle-control-center`, `toggle-clipboard`,
  `open-terminal`, `screenshot`, `lock`, `show-desktop`, `switcher-next`,
  `switcher-prev`, …), which talks to the running shell over
  `$XDG_RUNTIME_DIR/blue-environment.sock`;
* the `CompositorBridge` command set (focus/close/…, screenshots via
  `grim`, lock, reload, workspace count) translated to each backend's own
  mechanism, with a "not supported here" error where a backend genuinely
  has no equivalent (e.g. labwc has no fixed workspace count to change on
  sway/wayfire, wayfire has no live config-reload in this version);
* launching native apps goes through `backend/launcher.rs`: explicit
  session environment (`WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS`, …),
  own session (`setsid`), stderr drained to
  `~/.cache/Blue-Environment/launch.log`, and a notification with the
  reason if the app exits with an error right after start. Starting a
  compositor from a bare TTY with no session bus is wrapped in
  `dbus-run-session` automatically;
* Alt+Tab is decided by who has focus: with the Blue shell focused, the
  compositor hands the key to Blue's own switcher (Blue windows + native
  windows, Alt release commits); with a native app focused it's that
  compositor's own switcher (native windows only). Choosing a Blue window
  minimizes native windows first (`raise_shell`), since native windows are
  stacked above the shell on every one of these backends;
* shell overlays (Start menu, Control Center, the switcher…) temporarily
  minimize native windows while open and restore them afterward, since the
  shell sits beneath native windows and would otherwise be hidden by a
  maximized app.

**What differs per backend** — only two things, each isolated in its own
`*_config.rs` module:

* **how the shell gets started.** labwc takes a startup command directly on
  its command line (`-s`). sway and wayfire only start programs named
  *inside their own config* (an `exec` line for sway, an `[autostart]`
  entry for wayfire), so Blue makes sure such a line exists:
  * **no config at all** → Blue writes a complete default (keybinds,
    floating-by-default, theming) with the shell already wired in;
  * **a config already exists** (prepared by HackerOS, or the person's
    own) → for sway, Blue never touches it — it generates a separate tiny
    file that does `include "<their config>"` plus the one `exec` line
    sway needs (a real, documented sway directive), and launches with
    that instead. wayfire's `.ini` format has no confirmed equivalent to
    `include`, so there Blue makes the smallest possible edit instead: it
    adds one `[autostart]` line, only if one isn't already there, and
    changes nothing else in the file. Either way, a distro-shipped stock
    config (e.g. the `sway`/`wayfire` package's own default) is never
    mistaken for "already prepared" — only a file that actually starts
    Blue, or one the person has clearly customised themselves, counts.
  * **the shipped/generated default is floating, not tiling** — sway
    tiles by default (i3 heritage); Blue adds `for_window [all] floating
    enable` so it behaves like a normal desktop instead.
* **config format** for keybinds/theming/window rules — `labwc_config`,
  `sway_config` and `wayfire_config` each generate a sensible default in
  that backend's own syntax, matching the shell's own window-chrome
  palette, only when nothing is already there.

**Verification.** labwc and sway were both run and driven end-to-end in a
real headless instance (window tracking, clipboard, global shortcuts
including Alt+Tab, config generation in every branch above, `swaymsg
reload`/`exit`). wayfire's plugin configuration (`[autostart]`,
`foreign-toplevel`, `wm-actions`, `[input]`, `[decoration]`) is written
against its own shipped plugin documentation and its `main.cpp` source (for
`SIGTERM`-based clean shutdown), and the config-generation logic is unit
tested the same way as sway's, but the compositor itself could not be
started in the environment this was built in — wayfire requires a real DRM
render device even with `WLR_BACKENDS=headless`, which wasn't available
there. If something in the wayfire config needs adjusting in practice,
that's the most likely place.

Known limits: Blue's in-shell windows live in the shell layer, i.e.
beneath native windows; live workspace switching / DPMS timeout have no
IPC on sway/wayfire and are keybind/idle-daemon matters; wayfire has no
live config-reload in this version (settings changes need a fresh login).

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Super` | Toggle Start Menu |
| `Super+Tab` | Full-screen App Picker |
| `Super+1–4` | Switch Workspace |
| `Super+←/→` | Switch Workspace |
| `Super+↑` | Maximize Window |
| `Super+↓` | Minimize Window |
| `Super+D` | Show Desktop |
| `Super+L` | Lock Screen |
| `Alt+Tab` | Window Switcher |
| `Alt+Shift+Tab` | Window Switcher (backwards) |
| `Alt+F4` | Close Window |
| `Ctrl+Alt+T` | Open Terminal |
| `Ctrl+Alt+C` | Control Center |
| `Ctrl+Shift+V` | Clipboard History |
| `PrintScreen` | Screenshot |
| `Escape` | Close Panels / Cancel |

## VM / VirtualBox Support

When running inside VirtualBox or any VM:
- Compositor auto-detects `WAYLAND_DISPLAY`/`DISPLAY` → uses **winit** (nested) backend
- Full 3D rendering via host GPU
- XWayland started automatically for X11 app support

On bare metal (TTY, no display server):
- Uses **DRM/KMS** backend via libseat
- Requires seatd running and user in `seat` group

## Common Issues

### "Unable to find your web assets"
This means `npm run build` was not run before `tauri build`.
**Solution:** Always use `npm run build:tauri` (not `npm run tauri`).
The `beforeBuildCommand` in `tauri.conf.json` handles this automatically.

### chrono feature error
Ensure `Cargo.toml` has `chrono = "0.4"` (no features).
The `local-offset` feature does not exist in chrono 0.4.x.

### seatd / seat permission error
```bash
sudo systemctl enable --now seatd
sudo usermod -aG seat $USER
# Then re-login
```

© 2026 HackerOS Team
