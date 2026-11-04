# Plugins

A plugin is a package with `type => plugin`. Unlike an app it runs in
`<iframe sandbox="allow-scripts">` **without** `allow-same-origin`: it gets an
opaque origin, cannot touch the shell's DOM, storage or JS, and talks to the
shell only through `postMessage`. `PluginRuntime.svelte` is the other end of that
bridge; before it performs any call it checks that the call's permission is in
the plugin's **granted** list ([permissions.md](permissions.md)).

Bridge capabilities today: `notify` (`notifications`), clipboard read / write
(`clipboard`) and `storage`. Anything else is rejected with an error, not
ignored. Declare `[plugin] -> kind` to say where the shell should show it.

```
[package]
-> id => com.acme.clock-widget
-> type => plugin
...
[plugin]
-> kind  => widget
-> entry => index.js
[permissions]
-> list => ["storage"]
```

Inside the iframe your code sees a small global (`BlueEnvPluginAPI`) whose
methods return promises; a missing permission rejects the promise. Use the
template in `api/templates/plugin`.

## Blue Connect plugins

Separate from store plugins: Blue Connect speaks a KDE-Connect-compatible,
line-framed JSON protocol over mutual TLS (`kdeconnect.battery`,
`kdeconnect.clipboard`, `kdeconnect.share.request`, notifications…). The packet
types and size limits are in `src-tauri/src/BlueConnect/plugin_proto.rs`; the
settings-sync channel ([settings-sync.md](../settings-sync.md)) is a Blue-only
extension (`blue.sync.exchange`) on its own port.
