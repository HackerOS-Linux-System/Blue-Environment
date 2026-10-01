import { writable } from 'svelte/store';

/**
 * Persisted Blue Code preferences (localStorage, per user profile).
 * Everything the editor does "by default" that someone might reasonably
 * want different lives here and is editable from the ⚙ panel
 * (SettingsPanel.svelte) or the command palette.
 */
export interface BlueCodeSettings {
    /** true  → one click on a file in the explorer opens it (default)
     *  false → double click, the classic behaviour */
    openOnSingleClick: boolean;
    minimap: boolean;
    wordWrap: boolean;
    tabSize: number;
    /** Open files as "preview" tabs that are replaced by the next single-click. Off by default. */
    showTerminalOnStart: boolean;
}

export const DEFAULT_BLUE_CODE_SETTINGS: BlueCodeSettings = {
    openOnSingleClick: true,
    minimap: true,
    wordWrap: false,
    tabSize: 4,
    showTerminalOnStart: true,
};

const KEY = 'blue-code-settings-v1';

function load(): BlueCodeSettings {
    try {
        const raw = localStorage.getItem(KEY);
        if (raw) return { ...DEFAULT_BLUE_CODE_SETTINGS, ...JSON.parse(raw) };
    } catch { /* corrupt / unavailable → defaults */ }
    return { ...DEFAULT_BLUE_CODE_SETTINGS };
}

function create() {
    const { subscribe, update, set } = writable<BlueCodeSettings>(load());
    function persist(v: BlueCodeSettings) { try { localStorage.setItem(KEY, JSON.stringify(v)); } catch { /* ignore */ } }
    return {
        subscribe,
        patch(p: Partial<BlueCodeSettings>) { update((s) => { const n = { ...s, ...p }; persist(n); return n; }); },
        reset() { const d = { ...DEFAULT_BLUE_CODE_SETTINGS }; set(d); persist(d); },
    };
}

export const blueCodeSettings = create();
