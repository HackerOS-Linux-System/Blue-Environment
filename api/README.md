# @blue-environment/api

TypeScript SDK for building **Blue Store** apps and plugins for
[Blue Environment](https://github.com/HackerOS-Linux-System/Blue-Environment) —
the ones installed as `.blue` packages described by a `blue.hk` manifest and
launched as normal, native-feeling windows (no iframe, no sandboxed plugin
host, no separate process — your code mounts straight into the shell's own
window).

This package has exactly one job: define the contract between your compiled
`entry.js` and the shell. It ships zero runtime code of its own beyond one
identity helper (`defineBlueApp`) — everything else is types plus the actual
object shape (`BlueApi`) the shell hands you at run time.

## Install

```bash
npm install @blue-environment/api
```

## The contract

Your bundler (Vite, esbuild, rollup, tsc, whatever your framework compiles
down to) must produce **one ES module file** whose default export matches
`BlueMount`:

```ts
import { defineBlueApp, type BlueApi } from '@blue-environment/api';

export default defineBlueApp((root: HTMLElement, api: BlueApi) => {
  root.innerHTML = `<h1>Hello, ${api.app.name}!</h1>`;
  api.notify('Started', `${api.app.name} v${api.app.version} is running`);

  // optional cleanup, called when the window closes
  return () => {
    // clear timers, remove listeners, unmount your own framework root...
  };
});
```

The shell loads this file as a same-origin `blob:` URL and imports it as a
real ES module, then calls `mount(root, api)` with `root` being an empty
`<div>` already sized to your window. There is no `postMessage` bridge to
keep in sync — your DOM *is* the window's content.

## Framework examples

**Svelte:**

```ts
import { mount as svelteMount, unmount } from 'svelte';
import App from './App.svelte';
import { defineBlueApp } from '@blue-environment/api';

export default defineBlueApp((root, api) => {
  const instance = svelteMount(App, { target: root, props: { api } });
  return () => unmount(instance);
});
```

**React:**

```tsx
import { createRoot } from 'react-dom/client';
import App from './App';
import { defineBlueApp } from '@blue-environment/api';

export default defineBlueApp((root, api) => {
  const reactRoot = createRoot(root);
  reactRoot.render(<App api={api} />);
  return () => reactRoot.unmount();
});
```

**Plain TypeScript/JS:** just build DOM directly in `mount` — see the first
example above.

## Building and packaging a `.blue` file

This package only defines the contract; turning a project into an
installable `.blue` archive is `blue-dev`'s job (see
`scripts/tools/blue-dev` in this repository — a small standalone CLI
written in Lua 5.5):

```bash
# from your project root, after your bundler has produced dist/index.js
blue-dev init                 # scaffold a blue.hk in the current directory
blue-dev pack                 # bundle blue.hk + your built files into <id>-<version>.blue
```

`blue-dev pack` also fills in `archive` and `sha256` in `blue.hk`
automatically. See that tool's own README for the full manifest reference.

## The `BlueApi` object

| Member | Permission required | Notes |
|---|---|---|
| `app` | — | Static identity: `id`, `name`, `version`, `kind`, `permissions` — straight from your `blue.hk`. |
| `notify(title, message?)` | `notifications` | Desktop notification. |
| `clipboard.readText()` / `.writeText(text)` | `clipboard` | System clipboard. |
| `storage.get/set/remove/keys` | `storage` | Small JSON key-value store, private to your package. |
| `system.isTauri()` | — | Always available. |
| `system.platform()` | `system` | Currently always `"linux"` (Blue Environment is Linux-only). |
| `close()` | — | Closes your own window. |

Declare exactly what you use in `blue.hk`:

```
[permissions]
-> list => ["notifications", "clipboard", "storage"]
```

Calling a gated method without declaring its permission throws immediately
— see `hasPermission(api, permission)` if you'd rather check first and show
your own fallback UI. The full permission list is exported as
`KNOWN_PERMISSIONS`; anything else in `blue.hk` fails validation when the
package is installed, so there's no way to end up with a permission that
silently does nothing.

## Why no sandbox?

This is a deliberate design choice, not an oversight: Blue Store packages
run as **installed, trusted code** in the shell's own process — the same
trust level as any other program a person installs on their system. The
permission list exists to keep `blue.hk` an honest, readable description of
what a package does (and to keep well-behaved apps from reaching for things
they didn't declare), not to contain code that's assumed hostile. If you
need to load genuinely untrusted, arbitrary code, that's a different
problem than this SDK solves.

## License

GPL-3.0
