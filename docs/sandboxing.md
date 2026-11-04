# Isolation of cloned apps (`bwrap`)

*Settings → Cloned apps → Isolation.* **`bwrap` is the default.** (`env` only
swaps `HOME`/`XDG_*`; the app can still read your real home — Blue warns when
you pick it, or when bubblewrap is missing and an app falls back to it.)

| resource | in a `bwrap` clone |
|---|---|
| system (`/`) | read-only |
| `$HOME` | the clone's private home only; your real home and other users' homes are hidden |
| `/tmp`, `/mnt`, `/media`, `/run/media` | empty |
| devices | minimal `/dev` + GPU, sound, webcams |
| `$XDG_RUNTIME_DIR` | empty + Wayland socket, PipeWire, PulseAudio |
| session D-Bus | **filtered** by `xdg-dbus-proxy` (portals, notifications, tray, MPRIS) — or none if it isn't installed. The raw bus is never shared, so a clone cannot read the vault through the Secret Service |
| processes / IPC / hostname | own namespaces, `--new-session`, all capabilities dropped |
| syscalls | seccomp deny-list: `bpf`, `perf_event_open`, `userfaultfd`, kernel keyrings, `mount`, module loading, `kexec`, `reboot`, `process_vm_writev`, clock/hostname changes… (`ptrace`, `clone`, `unshare` stay allowed so browsers/Electron work) |
| network | shared |

Install `bubblewrap` and `xdg-dbus-proxy`. The seccomp filter is generated for
x86_64 and aarch64 (`src-tauri/src/sandbox.rs`) and verified against the kernel
headers; 32-bit processes are not filtered. Apps that need more (a USB device,
a folder) are best given those through the portals ([portals.md](portals.md)).
