# Permissions

You declare what your package uses in `[permissions] -> list`. The person sees
that list — with a risk level for each entry — **before anything is downloaded**,
chooses what to grant, and that choice is what the shell enforces.

| id | risk | what it means |
|---|---|---|
| `notifications` | low | post desktop notifications |
| `storage` | low | private key/value store (`api.storage`) |
| `clipboard` | medium | read/write the clipboard (`api.clipboard`) |
| `network` | medium | use the network |
| `files` | high | read/write files and folders |
| `system` | high | system information / settings (`api.system.platform()`) |
| `camera` | high | `api.media.camera()` |
| `microphone` | high | `api.media.microphone()` |
| `screen_capture` | high | `api.media.screen()` — goes through the ScreenCast portal |

## What happens at install

1. `store_resolve` returns the permissions (riskiest first), whether the update
   **adds** permissions, what was granted before, and the signature status.
2. A first install, or an update that adds a permission, shows the consent dialog.
   Medium- and high-risk permissions are never pre-ticked on a first install.
3. The grant is stored per user (`~/.config/Blue-Environment/store/consents.json`).
   A grant can never exceed the declaration — the backend refuses it.
4. `store_list_installed` returns `granted ∩ declared` as the package's
   permissions, so `api.app.permissions` is what the person allowed.
5. An update that asks for nothing new keeps the earlier decision silently.
   Uninstalling forgets it, so a reinstall asks again.

From the command line: `blue-store install <url> --grant-all | --grant a,b | --no-permissions`
(on a terminal it prints the list and asks `y/N`; without a terminal and without
flags it refuses rather than granting silently).

## What is enforced — honestly

* **Enforced:** every gated call in `api` (`notify`, `clipboard.*`, `storage.*`,
  `system.platform`, `media.*`) throws if the permission was not granted. Plugins
  run in a sandboxed iframe whose bridge checks the same list.
* **Not enforced:** apps run **in the shell's process** (see the SDK README,
  "Why no sandbox?"). Code that reaches for `fetch` or `navigator` directly is
  not intercepted. For `network` and `files` the list is an honest, reviewable
  description plus the consent record — not a containment boundary. If you need
  to run genuinely untrusted code, use a plugin, or isolate the whole app with
  [`bwrap` cloning](../sandboxing.md).
