# Settings sync

Keep appearance and behaviour in step between your own Blue computers over
**Blue Connect** (mutual TLS, certificates pinned at pairing).

*Settings → Sync.* Enable it **on both computers** for each other — nothing
listens (port 1718) until at least one paired device has it enabled, and a peer
is recognised by the certificate it pinned when you paired, never by a name.

**Synced:** colour scheme, shell theme, **custom themes**, icon/cursor theme, panel
and top-bar layout, language, night light, hot corners, OSD, weather, pinned apps,
accessibility and focus preferences (the full list is shown in the page).
**Never synced:** wallpaper paths and display scale (per machine), accounts, AI API
keys, cloned apps, plugins, **Parental Controls**, pairings, the focus block list.

**Conflicts:** per setting, the latest edit wins (ties broken by device id, clocks
more than a day ahead are clamped). **Sync now** exchanges and merges both ways in one
round trip; **Send this computer's settings** makes this machine win. Applying a
setting that arrived never counts as a local edit, so it cannot bounce back.
