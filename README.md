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
- **Blue Connect — phone, remote and KDE Connect** — besides LAN discovery and
  pairing, Blue Connect now shows and controls an **Android phone inside the
  shell** (live screen, touch / keyboard, apps, unlock, files, notifications,
  calls & SMS, screenshots), turns a phone into a **remote control** for a Blue
  Environment **TV** (or any Blue Environment), and bridges to **KDE Connect**.
- **Blue Media Writer** — writes `.iso` / `.img` (also `.xz`/`.gz`/`.zst`/`.bz2`)
  to USB sticks and SD cards, with a download catalog, checksum check,
  verify-after-write and a "restore drive" mode (inspired by Fedora Media Writer).
- **Blue Backup** — incremental, browsable snapshots (rsync hard links) with
  retention, restore and a systemd user timer. **Blue PDF** — built-in PDF reader.
- **Spotlight search** (apps, files, Settings, calendar, mail, calculator),
  **printer management** (CUPS), a **multi-monitor layout editor** (wlroots),
  **accessibility** (high contrast, text size, magnifier, Orca) and **Do Not Disturb**.
- **System-level Parental Controls** — a root service enforces hours and limits
  per user id on every session and TTY (see *Features in depth*).
- **Packaging** for Debian/Ubuntu, Fedora, LegendaryOS, Arch, Alpine,
  openSUSE, Gentoo, Void, Nix, Snap, and Flatpak (the latter two/Gentoo/
  Void as submission-ready templates — see `packaging/`).

## Build Instructions

### Prerequisites

```bash
# System packages (Debian/Ubuntu/HackerOS) — the Tauri 2 build dependencies.
# (Same list the CI uses in .github/workflows/test.yml.)
sudo apt install \
    build-essential curl git pkg-config \
    libssl-dev libudev-dev \
    libgtk-3-dev libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev librsvg2-dev

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

# Tauri CLI v2 (matches `tauri = "2"` in src-tauri/Cargo.toml; `npm install`
# also provides it as `npx tauri`)
cargo install tauri-cli --version "^2" --locked

# Only if you start a compositor (sway / labwc / wayfire) from a bare TTY:
# they use libseat, so seatd must be running and your user in the `seat` group.
# Not needed to build or to run Blue Environment inside an existing session.
# sudo apt install seatd && sudo systemctl enable --now seatd && sudo usermod -aG seat $USER
```

### Build the frontend + Tauri shell (the main app)

```bash
npm install
npm run tauri build
# This runs: npm run build  →  vite build  →  tauri build
```

> **Note:** Blue Environment is the *shell* (panel, launcher, apps). It does not
> ship its own compositor: it runs under an existing Wayland/X11 session (GNOME,
> KDE, …) or becomes the desktop of HackerOS-Comp / labwc / sway / wayfire — see
> *Compositor backends* below. (Older revisions of this README and CI described a
> Smithay-based `compositor/` crate; it never landed in this tree, so that text and
> the CI probe for it have been removed.)

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

## Features in depth

### Optional runtime tools

Everything below degrades gracefully: when a tool is missing the UI says what to install.

| Feature | Tools (Debian/Ubuntu package names) |
|---|---|
| Blue PDF | `poppler-utils` (`pdfinfo`, `pdftoppm`, `pdftotext`) |
| Blue Backup | `rsync` (backup location must be ext4 / btrfs / xfs — hard links) |
| Blue Media Writer | `util-linux` (`lsblk`, `umount`), `parted`, `dosfstools` (restore), `xz-utils` / `zstd` / `bzip2` (compressed images), `pkexec` (polkit) |
| Printers | `cups` (`lpstat`, `lpadmin`, `lpinfo`, `lp`) |
| Monitor layout | `wlr-randr` (wlroots compositors: labwc, sway, wayfire, HackerOS-Comp) |
| Accessibility | `orca` (screen reader), `speech-dispatcher` (`spd-say`) |
| Blue Connect → Phone | `adb` (`android-tools-adb`); `ffmpeg` for a smooth picture (without it: ~3 fps `screencap`); `scrcpy` for sound |
| Blue Connect → KDE Connect | `kdeconnect` (`kdeconnect-cli`) |
| Blue Connect → Remote | `wpctl` or `pactl` (volume), `playerctl` (media keys), `ydotool` + `ydotoold` (optional: control *native* apps) |

### Blue Connect → Phone (Android)

Enable *Developer options → USB debugging* on the phone (or *Wireless debugging*,
Android 11+, and pair it from Blue Connect → Phone → Wireless). **Show screen**
streams the phone as H.264 → MJPEG into the shell; click = tap, drag = swipe,
wheel = scroll, typing = text, plus Back / Home / Recents / volume / power. Also:
app launcher (launch, force-stop, uninstall, install APK), **unlock** (wake + swipe +
optional PIN — never stored), file browser with copy to / from the phone,
notification list, dialer / call / answer / hang up, SMS composer, quick toggles and
screenshots. Limits: apps that set `FLAG_SECURE` (banking, some video apps) are black in
the stream; `input text` only types ASCII; no audio in the embedded view (use the
*scrcpy* button). All `adb` calls are argument vectors and every value that reaches the
phone's shell is validated or single-quoted (`BlueConnect/phone.rs`, unit-tested).

### Blue Connect → Remote (phone / TV remote control)

On the machine to be controlled — typically the **TV UI** (`hl build.hl shell tv`),
where Blue Connect opens on this tab — open **Blue Connect → Remote → Pair a remote**.
The screen shows a QR code, the address and a 6-digit code; on the phone open the address
(any browser, nothing to install), enter the code and use the D-pad, OK / Back / Home /
Menu, media keys, volume, touchpad and keyboard. Another Blue Environment can also be the
remote: *Use this device as a remote*. A paired remote drives the TV through the same code
path as a game pad (`blue:remote-action` → `perform()` in `src/tv/TvShell.svelte`).

Security: off until enabled; pairing is open only while shown (≤ 10 min), one code pairs
one device, 5 wrong codes lock it for a minute; each remote gets a random 256-bit token
(only its SHA-256 is stored, files are `0600`); cross-site `Origin` is rejected; all input is
validated; remotes can be revoked. Traffic is plain HTTP on the LAN (no TLS) — use a network
you trust (`BlueConnect/remote.rs`, covered by unit and end-to-end socket tests).

### System-level Parental Controls

The in-shell controls live in the child's own `~/.config` and session, so a text console
(Ctrl+Alt+F3), another session or editing the JSON bypasses them. *Settings → Parental
Controls → System-level enforcement* copies the policy to `/etc/blue-environment/parental/<uid>.json`
(root-owned) and starts `blue-parental.service` (`blue-environment --parental-enforce`), which every
15 s enforces allowed hours and the daily total limit per uid (`loginctl terminate-user`, 5-minute
warning), counts usage itself in `/var/lib/blue-environment/parental/`, and kills blocked / over-limit
native apps. Changing or removing the policy needs polkit authentication **and** the PIN. Requirements and
limits: Blue Environment must be installed in a root-owned location (e.g. `/usr/bin`) — the service
refuses a user-writable binary; apps built into the shell cannot be killed individually (they run inside the
shell) and are covered by hours / total limit; give the child a **standard** account — an administrator can always
stop a root service. A reference unit is in `packaging/systemd/blue-parental.service`.

### Internationalisation

`node scripts/check-i18n-keys.mjs` keeps every locale in sync with `english.ts`; missing keys are filled with
the English text under a `FIXME i18n` marker. CI runs `--check --ratchet`: every locale must have every key, and the
number of still-untranslated keys may never exceed `scripts/i18n-baseline.json`. After translating, lower the baseline
with `node scripts/check-i18n-keys.mjs --report-only --update-baseline`.

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
| `Super+S` / `Ctrl+Alt+Space` | Spotlight search (also `blue-environment --ctl toggle-spotlight`) |
| `Ctrl+Alt+=` / `Ctrl+Alt+-` / `Ctrl+Alt+M` | Screen magnifier: zoom in / out / on-off |

## VM / VirtualBox Support

Inside a VM Blue Environment behaves like on any machine: it needs a Wayland or
X11 session, which the guest's compositor provides.

- Use a guest with 3D acceleration enabled (VirtualBox: *VMSVGA* + *Enable 3D
  Acceleration*) so the WebKit-based shell renders smoothly.
- To develop or try it **nested inside your normal desktop**, start a wlroots
  compositor in a window and run the shell in it, e.g.
  `WLR_BACKENDS=wayland labwc` (or `sway`), then launch `blue-environment` there.
- On bare metal from a TTY, the compositor you choose (labwc / sway / wayfire /
  HackerOS-Comp) talks to the GPU through DRM/KMS and `libseat`, so `seatd` must be
  running and your user in the `seat` group.

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
