# Diagnostics and crash reports

*Settings → Diagnostics* shows versions (shell, Tauri, WebKitGTK, OS, kernel,
compositor backend, PipeWire, bubblewrap), what is missing for portals and sandboxing,
and builds a report from the shell log and the compositor's `launch.log`.
**Copy** it or **Save** it to `~/Downloads`.

Everything is redacted first: home paths and user names, e-mail / IP / MAC addresses,
the host name, URL credentials, `password=`/`token=` style values, bearer tokens and
long random-looking strings.

**Crash reporting is off by default.** Turn it on and a panic writes a redacted
report to `~/.local/share/Blue-Environment/crashes/` (newest 20 kept). Reports leave
the computer only when you press Send — or enable auto-send — **and** you typed an
`https://` endpoint yourself; there is no built-in server and no telemetry.
