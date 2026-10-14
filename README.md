# ![Blue Enviroment - Graphical environment for HackerOS.](https://github.com/HackerOS-Linux-System/Blue-Environment/blob/main/images/default.png)
# Blue Environment v0.9

Production-grade Wayland desktop environment for HackerOS, built on
[Tauri](https://tauri.app) + Svelte (desktop shell).

## Features

- **Desktop shell** (`src/` + `src-tauri/`) — panel, launcher, window
  switcher, workspaces, notification center, control center, and a suite
  of first-party apps: Mail (IMAP/SMTP), Web, Docs (with PDF/DOCX
  import/export), Code editor, Terminal, File explorer, Camera, Archive
  manager, System Monitor, Partition Manager, Settings (including
  **Parental Controls**: PIN-protected app blocking, daily time limits,
  allowed-hours windows).
- **Alternative shell UIs** — the same apps and window manager behind two
  extra front ends that are **not part of a regular build** (they are only
  compiled in when you ask for them, see *Shell UI variants* below):
  a touch UI for **phones** (`src/phones`) and a 10-foot UI for **TVs**
  (`src/tv`, remote / game-pad navigation).
- **Customisable look** — shell themes control colours, corner radius,
  window shadow / opacity / border, title-bar height, interface scale,
  scrollbars, wallpaper tone and panel colour (Settings → Themes, with a live
  preview); the bar can be restyled and its right-hand widgets reordered
  (Settings → Bar): colours, spacing, text size, button/indicator shapes,
  **one-click presets** (minimal, macOS-style dock, compact, classic taskbar),
  **drag-and-drop** reordering of the left and right items and optional
  *Show desktop* / *Power* buttons. The theme editor is fully translated,
  warns about low contrast (WCAG), can **try an unsaved theme live** and exports
  a theme **together with its wallpaper**. Themes can also restyle the
  **full-screen application menu** (background colour, opacity, blur, category
  column, tile size and shape, search-field shape) — so community themes can
  give it their own look; the menu has a corner **–** to close it and a **×** in
  the search field to clear the text.
- **Language-aware folders** — the file manager, file picker and "Save as"
  dialogs use the user's real XDG folders (`~/Pobrane`, `~/Pulpit`, …), follow
  changes **live** (e.g. after the system language changes), and the installer
  creates them in the chosen language. Systems that still have English folders
  can be migrated in Settings → Language → *Personal folders* (files are moved,
  never copied; folders that exist under both names are left alone). The folder
  names for every language live in one file,
  `src/lib/data/userDirNames.json`, shared by the installer and the backend.
- **Responsive apps** — Settings, Files and Blue Software switch to a
  single-column, list → detail layout when their window is narrow (and on the
  phone UI), so they are usable on a small touch screen.
- **Packaging** for Debian/Ubuntu, Fedora, LegendaryOS, Arch, Alpine,
  openSUSE, Gentoo, Void, Nix, Snap, and Flatpak (the latter two/Gentoo/
  Void as submission-ready templates — see `packaging/`).

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
npm run tauri build
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

### Shell UI variants (default / phones / tv)

One codebase, three front ends. Pick the UI at build time:

| UI | What it is | Source | Build |
|----|------------|--------|-------|
| `default` | Desktop UI for computers (what you get without any flag) | `src/App.svelte`, `src/lib/` | `hl build.hl shell` |
| `phones` | Touch UI: status bar, paged app grid, dock, recents, pull-down shade, Back/Home/Recents bar | `src/phones/` | `hl build.hl shell phones` |
| `tv` | 10-foot UI: big tiles, focus ring, D-pad / game-pad / remote navigation, quick-settings side panel | `src/tv/` | `hl build.hl shell tv` |

```bash
hl build.hl shell phones            # variant + default backend
hl build.hl shell debian tv         # variant + Blue Software backend (any order)
hl build.hl shell --ui=tv           # same as 'hl build.hl shell tv'
hl build.hl android                 # phone UI packaged as an Android .apk

# or directly with npm
npm run dev:phones                  # hot-reload the phone UI
npm run dev:tv
npm run build:tauri:phones          # full Tauri build with the phone UI
npm run build:tauri:tv
```

Under the hood the variant is the `VITE_BLUE_UI` environment variable;
`vite.config.ts` swaps the entry point in `index.html` (`src/main.ts` →
`src/phones/main.ts` / `src/tv/main.ts`). The variants you did **not** pick are
never imported, so a regular build contains none of their code. Shared pieces
(app launcher list, spatial focus navigation, gamepad mapping, swipe gestures,
app viewport) live in `src/lib/altShell/` and are unit-tested.

TV controls: arrows / D-pad / left stick move the focus, **Enter / A**
activates, **Esc / Backspace / B** goes back, **Home / Start / Guide** returns
to the home screen, **Menu / Y / context-menu key** opens quick settings. Focus is remembered, so
closing an app returns to the tile that launched it. The TV UI has its own
remote-friendly **Settings** (language, Wi-Fi, Bluetooth, volume, brightness,
theme, HDMI-CEC) and understands remote keys from HDMI-CEC / IR / smart-TV
remotes (Back, Home, Menu, media keys). HDMI-CEC needs `cec-ctl` (package
`v4l-utils`); enable it once in the TV Settings.
An app can handle "back" itself by cancelling the `blue:back` window event
(`window.addEventListener('blue:back', e => { …; e.preventDefault(); })`).

In CI (`.github/workflows/build.yml`) choose the UI with the `ui` input of the
manual run; in HackerOS' `build-blue.yml` use `blue_env_ui` together with
`blue_env_source=source`.

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

## Live session / installer

When booted from the live image (`~/.config/Blue-Environment/.live` exists) Blue Environment shows **Blue Installer** full-screen.

- **Classic desktop** (top-right of the installer) switches to the normal Blue desktop; a floating **Return to Blue Installer** button brings you back. The installer stays mounted, so nothing you entered is lost. Not available while an installation is running.
- **Power** (top-right) shuts down or restarts the computer from the installer; the final screen also has **Shut Down** next to **Restart Now**.
- **Mounted target disk**: the installer unmounts every partition of the chosen disk (and swap, stale `/mnt/blue-install` mounts, LUKS/LVM holders) by itself. It only refuses when the chosen disk is the one the live system is running from.
- **Startup splash**: `src/index.html` + `src/splash.css` paint a loading screen instantly; `hideSplash()` (`src/lib/utils/splash.ts`) fades it out when the shell is ready.

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
