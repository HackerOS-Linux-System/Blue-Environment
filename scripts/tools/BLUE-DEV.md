# blue-dev

A single-file CLI, written in Lua 5.5, for building **Blue Store** packages
(apps, plugins, themes) for [Blue Environment](https://github.com/HackerOS-Linux-System/Blue-Environment).

It turns a project directory into an installable `<id>-<version>.blue`
archive (a `tar` stream compressed with `zstd`) — exactly the format the
shell's own installer (`src-tauri/src/BlueStore/install.rs`) expects.

## Install

Copy the one file (`blue-dev` — no other files in this directory are
required at run time; everything lives in that single script) anywhere on
your `PATH`:

```sh
cp blue-dev /usr/local/bin/
chmod +x /usr/local/bin/blue-dev
```

**Requirements:** a Lua interpreter (written for **5.5**; also runs
unchanged on 5.4, since it uses nothing 5.4 lacks — useful today since 5.5
isn't yet packaged on most distributions), plus `tar`, `zstd`, `sha256sum`
and `find` on `PATH` (all standard on any Linux system Blue Environment
targets).

```sh
lua5.5 blue-dev help   # or: lua5.4 blue-dev help
```

## Quick start

```sh
mkdir my-first-app && cd my-first-app
blue-dev init                  # scaffolds blue.hk + index.js + README + .blueignore
# edit index.js / blue.hk...
blue-dev pack                  # -> com.example.my-first-app-0.1.0.blue
```

That's it — `com.example.my-first-app-0.1.0.blue` is now installable via
Blue Software's "Store" tab (pointing at a hosted `blue.hk`) or directly:

```sh
blue-environment --store install ./blue.hk   # once blue.hk/archive are hosted somewhere
```

## Commands

### `blue-dev init [--type app|plugin|theme] [--id ID] [DIR]`

Scaffolds a new package in `DIR` (default: current directory):

- `blue.hk` — the manifest, pre-filled with sensible defaults
- `index.js` (apps/plugins) — a zero-build-step starting point; see
  [`@blue-environment/api`](../../../api/README.md)'s README for
  TypeScript/Svelte/React examples if you'd rather point `entry` at a
  bundler's output instead
- `styles.css` (themes)
- `.blueignore` — gitignore-style exclude list for `pack` (defaults to
  excluding `node_modules/`, `dist/`, and any previously-built `*.blue`)
- `README.md`

`--type` defaults to `app`. `--id` defaults to `com.example.<directory
name>` — always rename it before publishing (see the id rules below).

### `blue-dev pack [--write-manifest] [--install] [-o OUTPUT.blue] [DIR]`

1. Reads and validates `blue.hk` (same rules the shell's installer
   enforces — invalid `id`/`version`/`type`/`permissions`, a missing
   `entry` file, a missing `styles.css` for a theme, etc. are caught here,
   before a real install attempt would reject them).
2. Bundles the directory (minus anything `.blueignore` excludes) into
   `<id>-<version>.blue`.
3. Computes the archive's SHA-256 and fills `archive`/`sha256` into the
   **returned/written manifest text** — this is the `blue.hk` you actually
   publish (e.g. commit to your repo, or attach as a release asset
   alongside the `.blue` file) so a Blue Store install can verify the
   download before trusting it.

By default the `blue.hk` **on disk** is left untouched — most people
re-run `pack` many times while iterating and wouldn't want a git diff on
every build. Pass `--write-manifest` to update it in place once you're
ready to publish. Pass `--install` to install the freshly built package
into this machine's Blue Environment immediately afterwards (via
`blue-environment --store install`), for a fast local test loop.

> **Why doesn't the archive's own, bundled copy of `blue.hk` have the
> "real" `sha256`?** It structurally can't — a file cannot contain the
> hash of itself. The installer already accounts for this: it only checks
> the archive's *inner* `blue.hk` for `id`/`version`/`type` agreement with
> the *external* one it already fetched and hashed; the inner copy's
> `sha256` field is never itself checked against anything. The external,
> separately-published `blue.hk` this command hands you — with the real,
> final archive hash — is the one that matters, and the one `pack`
> generates carefully to be exactly right.

### `blue-dev validate [DIR]`

Checks `blue.hk` without packing anything. Exit code `0` if valid, `1`
otherwise — useful in CI.

### `blue-dev help`

Prints usage.

## `blue.hk` field reference

See [`@blue-environment/api`](../../../api/README.md)'s README and
`api/src/index.ts`'s `BlueManifest`/`BlueManifestPackage` types for the
full reference (kept in sync with what the shell's own parser,
`src-tauri/src/BlueStore/manifest.rs`, actually accepts). The short
version:

```
[package]
-> id => com.example.hello        ! lowercase letters/digits/./-/_, 1-64 chars
-> name => "Hello"                ! up to 80 chars
-> type => app                    ! app | plugin | theme
-> version => 0.1.0               ! semver, always x.y.z
-> author => "Your Name"
-> description => "A tiny app"
-> icon => Sparkles               ! a Lucide icon name, or a file in the archive

[app]                             ! or [plugin] / omit entirely for a theme
-> entry => index.js              ! a single ES module, default export = mount(root, api)
-> width => 800
-> height => 560

[permissions]
-> list => ["notifications", "storage"]
```

## `.blueignore`

One glob-ish pattern per line (`#` for comments, blank lines ignored).
`*` matches anything within a path segment or across `/` (kept simple on
purpose — this isn't `.gitignore`'s full syntax). Matched both against the
full relative path and against just the final path component, so a bare
`node_modules` excludes `node_modules/` anywhere in the tree, not only at
the root.

## Design notes

- **One file, no dependencies.** Everything (the `.hk` reader/writer, the
  packer, the scaffolder, the CLI) lives in the single `blue-dev` file —
  copy it anywhere, no `require` of sibling files, no third-party Lua
  libraries.
- **Shells out for tar/zstd/sha256**, exactly like the shell's own
  installer does — no pure-Lua reimplementation of archive framing or a
  hash function, so the format is guaranteed to agree with what actually
  gets installed (this was verified directly: a `blue-dev`-packed archive
  was fed straight into the real installer's Rust test suite, and
  installs, reads back, and uninstalls cleanly).
- **Deterministic-ish archives.** `pack` sorts entries and fixes
  `mtime`/`owner`/`group` in the tar output, so packing an unchanged
  project twice produces byte-identical archives (falls back to plain
  `tar --zstd` if the installed `tar` doesn't support `--sort`/`--mtime`,
  e.g. some BSD tars).
