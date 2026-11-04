# Security notes

## Webview permissions (`src-tauri/capabilities/default.json`)

The main window may call every Blue command (generated from `main.rs`, checked in
CI) and **only** the plugin permissions the frontend really uses: core window /
event / webview / path, `dialog`, `opener:allow-open-url`, and clipboard text.

Removed in this release: `fs:allow-read-text-file`, `fs:allow-read-dir`,
`fs:allow-exists` and `fs:scope` (which exposed all of `$HOME` — SSH keys, the
vault file — to any script running in the webview), `shell:*`, `http:default`,
`process:default`, `os:default`, `notification:default`, `global-shortcut:default`.
The plugins stay registered for Rust-side use; the webview just cannot call them.
If you add a JS import of one of them, add its permission back deliberately.
`capabilities/web-content.json` (embedded web pages) grants one command only.

## Content-Security-Policy

`tauri.conf.json` still has `style-src 'self' 'unsafe-inline'`, because Svelte
writes `style=""` attributes and some libraries inject `<style>` elements at run
time. Two steps toward removing it are done:

1. System themes and community-app stylesheets are now applied as **constructed
   stylesheets** (`utils/managedStyle.ts`), which CSP does not treat as inline style.
2. `src-tauri/tauri.csp-strict.conf.json` is the target policy:
   `style-src 'self'; style-src-elem 'self'; style-src-attr 'unsafe-inline'` — style
   *elements* can no longer be injected (the CSS-exfiltration vector); style
   *attributes* stay allowed.

Try it with `cargo tauri build --config src-tauri/tauri.csp-strict.conf.json` (or
`dev`) and watch the console for `Refused to apply a stylesheet` / `Refused to apply
inline style`. Known remaining blockers to check before making it the default:
libraries that create `<style>` elements (xterm's DOM renderer, any editor extension
not using adoptedStyleSheets) and `PluginRuntime`'s `srcdoc` (it carries one inline
`<style>`). Once the console is clean, copy the `csp` string into `tauri.conf.json`.

## Isolation, vault, packages

See [sandboxing.md](sandboxing.md), [secret-service.md](secret-service.md),
[sdk/permissions.md](sdk/permissions.md) and [sdk/signing.md](sdk/signing.md).
