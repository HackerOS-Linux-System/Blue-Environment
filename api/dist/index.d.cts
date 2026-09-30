declare const KNOWN_PERMISSIONS: readonly ["notifications", "clipboard", "storage", "network", "files", "system"];
type BluePermission = (typeof KNOWN_PERMISSIONS)[number];
/** The three kinds of package `blue.hk`'s `[package] -> type` accepts. */
type PackageKind = 'app' | 'plugin' | 'theme';
/** Static identity of the running package — `api.app` in {@link BlueApi}.
 * Comes straight from your own `blue.hk`, so it's always in sync with
 * what you shipped. */
interface BlueAppInfo {
    /** Your `blue.hk` `[package] -> id` (e.g. `com.example.hello`). */
    id: string;
    /** Your `blue.hk` `[package] -> name`. */
    name: string;
    /** Your `blue.hk` `[package] -> version` (semver `x.y.z`). */
    version: string;
    kind: PackageKind;
    /** Exactly what you declared in `[permissions] -> list`, nothing more —
     * safe to read to feature-detect what your own app can do (useful if
     * you ship one bundle for both a "full" and a "restricted" blue.hk). */
    permissions: BluePermission[];
}
/** The clipboard surface — requires the `"clipboard"` permission. */
interface BlueClipboardApi {
    readText(): Promise<string>;
    writeText(text: string): Promise<void>;
}
/**
 * Small persistent key/value store, private to your package (namespaced by
 * `kind:id` on the shell side — no other package, installed or built-in,
 * can read or write your keys). Requires the `"storage"` permission.
 *
 * Values are structured-cloned through `JSON.stringify`/`JSON.parse`, so
 * store plain JSON-serialisable data (objects, arrays, strings, numbers,
 * booleans) — not `Map`/`Set`/class instances/functions/`undefined`.
 */
interface BlueStorageApi {
    get<T = unknown>(key: string): T | null;
    set(key: string, value: unknown): void;
    remove(key: string): void;
    /** Every key currently stored by your package (already de-namespaced —
     * exactly the strings you originally passed to `set`). */
    keys(): string[];
}
/** System/environment info — `platform()` requires the `"system"`
 * permission; `isTauri()` is always available (no permission needed)
 * since it's just "am I running for real or in a plain web preview". */
interface BlueSystemApi {
    isTauri(): boolean;
    platform(): Promise<string>;
}
/**
 * The object your `mount(root, api)` receives. Every method beyond
 * `app`/`system.isTauri()`/`close()` is gated by the permission noted on
 * its own interface — see this file's module doc for the full rule.
 */
interface BlueApi {
    app: BlueAppInfo;
    /** Shows a desktop notification. Requires `"notifications"`. */
    notify(title: string, message?: string): void;
    clipboard: BlueClipboardApi;
    storage: BlueStorageApi;
    system: BlueSystemApi;
    /** Closes the window hosting your app — same effect as the person
     * clicking the titlebar's close button. Always available, no
     * permission needed: closing your own window can't affect anything
     * outside it. */
    close(): void;
}
/**
 * What your `entry.js` must default-export. Called once, when the shell
 * mounts your window's content into `root` (an empty `<div>` sized to the
 * window's client area — see `width`/`height` in `blue.hk`'s `[app]`
 * section for the window's starting size). The optional returned function
 * is your teardown: called once, when the window closes, so you can clear
 * timers, remove listeners, or unmount your own framework's root.
 */
type BlueMount = (root: HTMLElement, api: BlueApi) => (() => void) | void;
/**
 * Identity helper with no runtime effect — its only job is to give you
 * autocomplete/type-checking on `root`/`api` at the call site without
 * writing out `BlueMount` yourself:
 *
 * ```ts
 * export default defineBlueApp((root, api) => { ... });
 * ```
 */
declare function defineBlueApp(mount: BlueMount): BlueMount;
/**
 * Runtime guard for a permission your app is about to rely on — throws the
 * same message the shell itself would throw if you called a gated method
 * without declaring it, but lets you check *before* attempting the call
 * (e.g. to show your own fallback UI instead of hitting a thrown error).
 *
 * ```ts
 * if (hasPermission(api, 'clipboard')) { ... }
 * ```
 */
declare function hasPermission(api: BlueApi, permission: BluePermission): boolean;
/**
 * Every field `blue.hk`'s `[package]` section accepts. This type exists
 * purely as living documentation / for tooling (e.g. a future `blue-dev
 * validate` or an editor plugin) that wants to check a manifest
 * programmatically — `blue.hk` itself is parsed by the shell's own `hk`
 * format, never by JSON/TypeScript, so nothing here is imported by your
 * app at run time.
 */
interface BlueManifestPackage {
    /** `^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9])?$`, reverse-DNS recommended
     * (`com.example.hello`). Must not collide with a built-in app's id. */
    id: string;
    /** Up to 80 characters. */
    name: string;
    type: PackageKind;
    /** Semver, always three numeric parts: `1.0.0`, not `1.0`. */
    version: string;
    author?: string;
    description?: string;
    /** A Lucide icon name (e.g. `"Sparkles"`), or a path to an image file
     * bundled inside your archive (e.g. `"icon.png"`). */
    icon?: string;
    /** The `.blue` archive's file name or absolute URL — set automatically
     * by `blue-dev pack`, resolved relative to wherever your `blue.hk` is
     * hosted when a person installs it. Must end in `.blue`. */
    archive?: string;
    /** SHA-256 of the `.blue` archive, lowercase hex, 64 characters — set
     * automatically by `blue-dev pack`. Strongly recommended: without it,
     * a corrupted or tampered download can't be detected before install. */
    sha256?: string;
    homepage?: string;
    license?: string;
    category?: string;
}
/** `blue.hk`'s `[app]` (or `[plugin]`) section. */
interface BlueManifestAppSection {
    /** Path to your bundled entry file, relative to the archive root
     * (default: `index.js`). Must be a single ES module. */
    entry?: string;
    /** Path to an optional bundled stylesheet, relative to the archive
     * root — loaded and scoped to your window automatically. */
    style?: string;
    /** Starting window size in pixels (200-4000). */
    width?: number;
    height?: number;
    minWidth?: number;
    minHeight?: number;
}
/** The full shape of a `blue.hk` file, as documentation for tooling. */
interface BlueManifest {
    package: BlueManifestPackage;
    app?: BlueManifestAppSection;
    plugin?: BlueManifestAppSection & {
        kind?: string;
    };
    permissions?: {
        list: BluePermission[];
    };
}

export { type BlueApi, type BlueAppInfo, type BlueClipboardApi, type BlueManifest, type BlueManifestAppSection, type BlueManifestPackage, type BlueMount, type BluePermission, type BlueStorageApi, type BlueSystemApi, KNOWN_PERMISSIONS, type PackageKind, defineBlueApp, hasPermission };
