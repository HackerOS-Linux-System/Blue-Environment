# xdg-desktop-portal

Flatpak apps, Firefox, Chromium and every sandboxed program ask the desktop for
files, screenshots and screen sharing through `xdg-desktop-portal`. Blue ships a
backend for its session (`XDG_CURRENT_DESKTOP=…:Blue`):

| portal | served by |
|---|---|
| FileChooser (open / save / save several / folder) | **Blue**, a native chooser; filters from the app (globs and MIME types) are honoured |
| Screenshot | **Blue**: asks "Allow *app* to take a screenshot?", captures with `grim` (full screen, or `slurp` region) into `~/Pictures/Screenshots` |
| ScreenCast / RemoteDesktop (screen sharing in Meet, Discord, OBS, Firefox) | **xdg-desktop-portal-wlr** — it streams the compositor's output over PipeWire; Blue configures the routing and checks it in Diagnostics |
| everything else | gtk |

`PickColor` answers "not supported".

## Install

```
/usr/share/xdg-desktop-portal/portals/blue.portal      ← packaging/portals/blue.portal
/usr/share/xdg-desktop-portal/blue-portals.conf         ← packaging/portals/blue-portals.conf
```

and the packages `xdg-desktop-portal`, `xdg-desktop-portal-wlr`, `xdg-desktop-portal-gtk`,
`pipewire`, `wireplumber`, `grim`, `slurp`. The shell claims
`org.freedesktop.impl.portal.desktop.blue` at start-up. **Settings → Diagnostics**
lists exactly what is missing.
