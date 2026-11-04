# Your first Blue Store app

A Blue Store app is **one ES module** plus a `blue.hk` manifest, packed into a
`.blue` archive. The shell imports your module and calls it with an empty
`<div>` and an `api` object — there is no iframe and no separate process for
apps (plugins are different, see [plugins.md](plugins.md)).

## 1. Copy the template

```bash
cp -r api/templates/app my-app && cd my-app
npm install
```

You get `blue.hk`, `src/index.ts` (a counter that uses `storage` and `onBack`),
a Vite config that emits a single `dist/index.js`, and the packaging script.

## 2. Edit `blue.hk`

Change `id` (reverse-DNS, lowercase), `name`, `version` and list **only the
permissions you use** — people see exactly this list before installing
([permissions.md](permissions.md)).

## 3. Build and pack

```bash
npm run build                # → dist/index.js
blue-dev pack                # → <id>-<version>.blue, fills in archive + sha256
```

`blue-dev` lives in `scripts/tools/` ([BLUE-DEV.md](../../scripts/tools/BLUE-DEV.md)).

## 4. (Recommended) sign it

```bash
node scripts/blue-package-sign.mjs keygen acme          # once; keep acme.key secret
node scripts/blue-package-sign.mjs sign --key acme.key --kind app \
     --id com.acme.counter --version 1.0.0 --sha256 $(sha256sum com.acme.counter-1.0.0.blue | cut -d' ' -f1) --signer acme
```

Paste the two printed lines under `[package]`. See [signing.md](signing.md).

## 5. Publish

Host `blue.hk` and the `.blue` next to each other (HTTPS) and add the `blue.hk`
URL to a store index, or have people paste the URL into Blue Store.

## Test without installing

`api.system.isTauri()` is `false` in a plain browser, so you can develop with
`vite dev` and only gated calls need the shell.
