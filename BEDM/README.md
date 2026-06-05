# BEDM — Blue Environment Display Manager

**BEDM** is a production display manager for Linux, built with:
- **Rust** daemon (`bedm`) — manages PAM auth, sessions, VT switching
- **Tauri + React** greeter (`bedm-greeter`) — the login UI
- **Unix socket IPC** — secure daemon↔greeter communication

BEDM is a rival to SDDM, GDM, and LightDM, designed for the
[Blue Environment](https://github.com/HackerOS-Linux-System/Blue-Environment)
Wayland desktop but works with any session.

---

## Features

- 🔐 **Real PAM authentication** via `/etc/shadow` + crypt(3)
- 🖼️ **Aurora glassmorphism UI** — animated background, user avatars
- 🖥️ **Session management** — Wayland & X11 sessions from `.desktop` files
- 👥 **Multi-user** — lists system users (UID ≥ 1000), user avatars from `~/.face`
- ⚡ **Autologin** support with configurable delay
- 🔌 **Power menu** — shutdown, reboot, suspend, hibernate with countdown
- 🔒 **Brute-force protection** — 5 attempt limit per session
- 📋 **systemd integration** — replaces `display-manager.service`
- 🎨 **Wallpaper support** — reads `/etc/bedm/wallpaper.png`

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│  TTY1 / VT1                                      │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │  bedm (daemon, root)                         │ │
│  │    PAM authentication                        │ │
│  │    Session launching (drop privs to user)    │ │
│  │    VT management                             │ │
│  │    IPC: /run/bedm/bedm.sock                 │ │
│  └───────────────┬─────────────────────────────┘ │
│                  │ Unix socket (JSON)             │
│  ┌───────────────▼─────────────────────────────┐ │
│  │  bedm-greeter (Tauri, runs as _bedm user)   │ │
│  │    React UI (TypeScript + Tailwind)          │ │
│  │    Clock, user list, password input          │ │
│  │    Session picker, power menu                │ │
│  └─────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

---

## Quick Install

```bash
# Clone / extract BEDM
cd BEDM

# Build and install (requires Rust + Node.js)
sudo bash install.sh

# With autologin:
sudo bash install.sh --autologin myusername
```

---

## Manual Build

```bash
# Install build dependencies (Debian/Ubuntu)
apt install cargo nodejs npm libpam-dev

# Build
make build

# Install (as root)
sudo make install

# Enable
sudo systemctl enable --now bedm
```

---

## Configuration

Edit `/etc/bedm/bedm.toml`:

```toml
[general]
greeter_path = "/usr/bin/bedm-greeter"
vt = 1
theme = "blue"
show_user_list = true
allow_root = false
minimum_uid = 1000

# Autologin (optional)
# autologin_user    = "username"
# autologin_session = "blue-environment"

[power]
shutdown  = "shutdown -h now"
reboot    = "reboot"
suspend   = "systemctl suspend"
hibernate = "systemctl hibernate"
```

---

## User Avatars

BEDM reads user avatars from (in priority order):
1. `~/.face`
2. `~/.face.icon`
3. `~/.config/bedm/avatar.png`
4. `/var/lib/AccountsService/icons/<username>`

---

## Logs

```bash
# View BEDM logs
journalctl -u bedm -f

# Or from file
tail -f /var/log/bedm/bedm.log
```

---

## Comparison

| Feature               | BEDM | SDDM | GDM  | LightDM |
|-----------------------|------|------|------|---------|
| Wayland native        | ✅   | ✅   | ✅   | ⚠️      |
| X11 support           | ✅   | ✅   | ✅   | ✅      |
| PAM auth              | ✅   | ✅   | ✅   | ✅      |
| Autologin             | ✅   | ✅   | ✅   | ✅      |
| Custom themes         | ✅   | ✅   | ❌   | ✅      |
| User avatars          | ✅   | ✅   | ✅   | ✅      |
| Blue Environment      | ✅   | ❌   | ❌   | ❌      |
| Aurora UI             | ✅   | ❌   | ❌   | ❌      |
| Glassmorphism         | ✅   | ❌   | ❌   | ❌      |
| Rust backend          | ✅   | ✅   | ❌   | ❌      |
| React frontend        | ✅   | ❌   | ❌   | ❌      |

---

## License

GPL-3.0 — © 2026 HackerOS Team
