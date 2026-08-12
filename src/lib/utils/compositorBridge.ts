let realInvoke: ((cmd: string, args?: any) => Promise<any>) | null = null;

async function ensureInvoke(): Promise<void> {
    if (realInvoke) return;
    try {
        const core = await import('@tauri-apps/api/core');
        realInvoke = core.invoke;
    } catch {
        // Not in Tauri — no-op stub
        realInvoke = async () => ({ success: true });
    }
}
ensureInvoke();

async function invoke<T = any>(cmd: string, args?: any): Promise<T> {
    await ensureInvoke();
    return realInvoke!(cmd, args);
}

// Listen helper — only works inside Tauri; silently ignored in browser
async function listen(event: string, cb: (payload: any) => void): Promise<() => void> {
    try {
        const mod = await import('@tauri-apps/api/event');
        const unlisten = await mod.listen(event, (e: any) => cb(e.payload));
        return unlisten;
    } catch {
        return () => {};
    }
}

// ── Types ──────────────────────────────────────────────────────────────────

export interface WindowInfo {
    id:            number;
    title:         string;
    app_id:        string;
    x:             number;
    y:             number;
    width:         number;
    height:        number;
    is_fullscreen: boolean;
    is_minimized:  boolean;
    workspace:     number;
}

// ── Send helper ────────────────────────────────────────────────────────────

async function send(type: string, payload: Record<string, unknown> = {}): Promise<boolean> {
    try {
        const result = await invoke<{ success: boolean }>('settings_send_to_compositor', {
            command: { type, ...payload },
        });
        return result.success;
    } catch (e) {
        console.warn('[compositorBridge] send error:', e);
        return false;
    }
}

// ── Public API ─────────────────────────────────────────────────────────────

export const CompositorBridge = {
    focusWindow:            (id: number)                                      => send('focus_window', { id }),
    closeWindow:            (id: number)                                      => send('close_window', { id }),
    killWindow:             (id: number)                                      => send('kill_window',  { id }),
    toggleMaximize:         (id: number)                                      => send('toggle_maximize', { id }),
    minimizeWindow:         (id: number)                                      => send('minimize_window', { id }),
    restoreWindow:          (id: number)                                      => send('restore_window', { id }),
    setFullscreen:          (id: number, fullscreen: boolean)                 => send('set_fullscreen', { id, fullscreen }),
    tileWindow:             (id: number, position: 'left'|'right'|'full'|'restore') => send('tile_window', { id, position }),
    moveWindowToWorkspace:  (id: number, workspace: number)                   => send('move_window_to_workspace', { id, workspace }),
    switchWorkspace:        (index: number)                                   => send('switch_workspace', { index }),
    setWorkspaceCount:      (count: number)                                   => send('set_workspace_count', { count }),
    setDpmsTimeout:         (seconds: number)                                 => send('set_dpms_timeout', { seconds }),
    lockScreen:             ()                                                => send('lock_screen'),
    takeScreenshot:         (path: string, mode: 'full'|'focused' = 'full')  => send('take_screenshot', { path, mode }),
    setKeyboardLayout:      (layout: string, variant?: string)                => send('set_keyboard_layout', { layout, variant: variant ?? null }),
    setCursor:              (theme: string, size: number)                     => send('set_cursor', { theme, size }),
    reloadConfig:           ()                                                => send('reload_config'),
    getWindowList:          ()                                                => send('get_window_list'),
    /// Requests HDR on/off for one output — see `protocols/color_management.rs`
    /// on the compositor side for how far this reaches today: the
    /// parametric negotiation path is real, but the compositor honestly
    /// reports `hdr_active: false` back until the render-side tone-mapping
    /// stub is filled in (see `HdrStateChanged` doc in ipc/messages.rs).
    setHdrEnabled:          (output: string, enabled: boolean)                => send('set_hdr_enabled', { output, enabled }),

    onWindowList:           (cb: (windows: WindowInfo[]) => void)            => listen('compositor:window-list', d => cb(d.windows)),
    onWindowFocused:        (cb: (id: number) => void)                       => listen('compositor:window-focused', d => cb(d.id)),
    onWindowOpened:         (cb: (w: WindowInfo) => void)                    => listen('compositor:window-opened', d => cb(d.window)),
    onWindowClosed:         (cb: (id: number) => void)                       => listen('compositor:window-closed', d => cb(d.id)),
    onWorkspaceSwitched:    (cb: (index: number, count: number) => void)     => listen('compositor:workspace-switched', d => cb(d.index, d.count)),
    onToggleStartMenu:      (cb: () => void)                                 => listen('compositor:toggle-start-menu', () => cb()),
    onIdleChanged:          (cb: (idle: boolean) => void)                    => listen('compositor:idle-changed', d => cb(d.idle)),
    onScreenshotReady:      (cb: (path: string) => void)                     => listen('compositor:screenshot-ready', d => cb(d.path)),
    /// Candidate-window (IME popup) visibility/geometry — informational
    /// only, see `protocols/input_method.rs` module doc: the shell never
    /// draws the candidate list itself, only reacts to where it is (e.g.
    /// to avoid overlapping it with its own overlay chrome).
    onImeCandidateWindow:   (cb: (visible: boolean, x: number, y: number, width: number, height: number) => void) =>
        listen('compositor:ime-candidate-window', d => cb(d.visible, d.x, d.y, d.width, d.height)),
    onHdrStateChanged:      (cb: (output: string, hdrActive: boolean) => void) =>
        listen('compositor:hdr-state-changed', d => cb(d.output, d.hdr_active)),
    /// Real GPU inventory from `UdevData::gpu_manager` — see
    /// `CompositorMessage::GpuList`'s doc comment on the compositor side
    /// for exactly what's covered (primary GPU always accurate; a
    /// hotplugged secondary GPU found after startup won't retrigger this
    /// today).
    onGpuList:              (cb: (gpus: { node: string; primary: boolean; output_count: number }[]) => void) =>
        listen('compositor:gpu-list', d => cb(d.gpus)),
};

export default CompositorBridge;

// ── Unified screenshot path ──────────────────────────────────────────────
//
// Previously there were two independent, inconsistent screenshot paths:
//   1. `CompositorBridge.takeScreenshot()` here — sends IPC to the
//      compositor, which (in `compositor/src/ipc/handler.rs`) shells out
//      to `grim`/`import` itself. Nothing in the UI actually called this.
//   2. `SystemBridge.takeScreenshot()` (systemBridge.ts) — a Tauri
//      command (`BlueScreenshot::take_screenshot`) that *also* shells out
//      to `grim`/`import`/etc, independently, and is what
//      `BlueScreenshot.svelte` actually calls.
// Both ultimately depend on `grim` working, and `grim` itself needs the
// compositor to implement `wlr-screencopy-v1` — which the compositor now
// does (see `compositor/src/protocols/screencopy.rs`), so route through
// the compositor path first since it's the one that can be IPC-driven
// without a `sh -c` round trip, and only fall back to the Tauri
// CLI-fallback command for sessions where Blue Compositor plainly isn't
// running the show (X11, or a different Wayland compositor).
export async function takeScreenshotUnified(mode: 'full' | 'focused' = 'full'): Promise<string | null> {
    const path: string = await invoke('default_screenshot_path').catch(() => '');
    if (!path) {
        // Couldn't even compute a target path (e.g. no home dir) — skip
        // straight to the CLI fallback, which computes its own path.
        try { return (await invoke<string>('take_screenshot')) || null; } catch { return null; }
    }

    const compositorResult = await new Promise<string | null>(async (resolve) => {
        let settled = false;
        const timeout = setTimeout(() => { if (!settled) { settled = true; resolve(null); } }, 3000);
        const unlisten = await CompositorBridge.onScreenshotReady((readyPath) => {
            if (!settled) {
                settled = true;
                clearTimeout(timeout);
                resolve(readyPath || path);
            }
        });
        const sent = await CompositorBridge.takeScreenshot(path, mode);
        if (!sent && !settled) {
            settled = true;
            clearTimeout(timeout);
            unlisten();
            resolve(null);
        }
    });
    if (compositorResult) return compositorResult;

    // Fallback: CLI-based Tauri command (works under X11 or a
    // non-Blue-Compositor Wayland session).
    try {
        const cliPath: string = await invoke('take_screenshot');
        return cliPath || null;
    } catch {
        return null;
    }
}
