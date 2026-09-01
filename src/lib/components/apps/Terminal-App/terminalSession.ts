import { writable, get } from 'svelte/store';
import { THEMES, type ThemeName, type Tab } from './themes';
import { isTauriEnv, tauriInvoke, tauriListen } from './tauriBridge';
import { loadXterm, getXtermClasses } from './xtermLoader';

/**
 * createTerminalSession() — Svelte replacement for the React
 * useTerminalSession() hook. Call this once per <TerminalApp> instance
 * (e.g. in its <script> block) rather than importing a singleton, so each
 * terminal window gets its own independent set of tabs/PTYs.
 */
export function createTerminalSession() {
  const tabs = writable<Tab[]>([]);
  const activeTab = writable<string | null>(null);
  // Defaults to the Hydra terminal palette when the Hydra shell theme is
  // active (see `ShellThemeStyle.svelte`, which sets
  // `data-shell-theme` on `documentElement` — read directly here rather
  // than prop-drilling the active theme id into this standalone session
  // module, or duplicating configStore's own persistence logic just to
  // peek at one field), 'Blue Dark' otherwise — same as before. This
  // app's own theme picker (SettingsPanel.svelte) is untouched by this:
  // there's no persistence for an explicit choice to check against yet
  // (`themeName` has always just reset to a hardcoded default on every
  // launch), so there's nothing to accidentally override.
  const themeName = writable<ThemeName>(
    typeof document !== 'undefined' && document.documentElement.dataset.shellTheme === 'hydra'
      ? 'Hydra'
      : 'Blue Dark'
  );
  const fontSize = writable(14);
  const loaded = writable(false);

  const termRefs = new Map<string, { term: any; fitAddon: any; container: HTMLDivElement }>();
  const unlistenRefs = new Map<string, (() => void)[]>();

  loadXterm().finally(() => loaded.set(true));

  function newTab() {
    const id = `tab-${Date.now()}`;
    tabs.update((t) => [...t, { id, title: 'Terminal' }]);
    activeTab.set(id);
  }

  function closeTab(id: string, e?: MouseEvent) {
    e?.stopPropagation();
    if (isTauriEnv()) tauriInvoke('pty_close', { id }).catch(() => {});

    const unlisteners = unlistenRefs.get(id) || [];
    unlisteners.forEach((fn) => fn());
    unlistenRefs.delete(id);

    const ref = termRefs.get(id);
    ref?.term?.dispose();
    termRefs.delete(id);

    tabs.update((prev) => {
      const next = prev.filter((t) => t.id !== id);
      if (get(activeTab) === id) activeTab.set(next[next.length - 1]?.id || null);
      return next;
    });
  }

  function startFallbackMode(term: any, id: string) {
    const useTauri = isTauriEnv();
    term.write('\x1b[32mBlue Terminal\x1b[0m\r\n');
    if (useTauri) term.write('\x1b[33mℹ PTY unavailable — using execute_command fallback\x1b[0m\r\n');

    let line = '';
    let cwd = '~';
    const prompt = () => term.write(`\r\n\x1b[36m${cwd}\x1b[0m \x1b[32m$\x1b[0m `);

    const run = async (cmd: string) => {
      const trimmed = cmd.trim();
      if (!trimmed) { prompt(); return; }
      if (trimmed === 'clear') { term.clear(); prompt(); return; }
      if (trimmed.startsWith('cd ')) {
        const dir = trimmed.slice(3).trim();
        cwd = dir === '~' ? '~' : dir;
        prompt(); return;
      }
      if (useTauri) {
        try {
          const result = await tauriInvoke('execute_command', { command: trimmed });
          if (result?.stdout) term.write('\r\n' + result.stdout.replace(/\n/g, '\r\n'));
          if (result?.stderr) term.write('\r\n\x1b[31m' + result.stderr.replace(/\n/g, '\r\n') + '\x1b[0m');
        } catch (e) {
          term.write(`\r\n\x1b[31mError: ${e}\x1b[0m`);
        }
      } else {
        term.write(`\r\n\x1b[2m[No backend — cannot run: ${trimmed}]\x1b[0m`);
      }
      prompt();
    };

    prompt();

    term.onKey(({ key, domEvent }: any) => {
      const code = domEvent.keyCode;
      if (code === 13) { run(line); line = ''; }
      else if (code === 8) { if (line.length > 0) { line = line.slice(0, -1); term.write('\b \b'); } }
      else if (key.charCodeAt(0) >= 32) { line += key; term.write(key); }
    });
  }

  function initTerminal(id: string, container: HTMLDivElement | null) {
    const { Terminal, FitAddon, WebLinksAddon } = getXtermClasses();
    if (!container || !get(loaded) || !Terminal || termRefs.get(id)?.term) return;

    const theme = THEMES[get(themeName)];
    const term = new Terminal({
      theme, fontSize: get(fontSize),
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      cursorBlink: true, scrollback: 10000,
      allowTransparency: false, bellStyle: 'none', convertEol: true,
    });

    const fitAddon = FitAddon ? new FitAddon() : null;
    if (fitAddon) term.loadAddon(fitAddon);
    if (WebLinksAddon) term.loadAddon(new WebLinksAddon());
    term.open(container);
    fitAddon?.fit();
    termRefs.set(id, { term, fitAddon, container });

    if (isTauriEnv()) {
      tauriInvoke('pty_create', { id, cols: term.cols, rows: term.rows }).catch((e: any) => {
        term.write(`\r\x1b[31mFailed to start PTY: ${e}\x1b[0m\r\n`);
        startFallbackMode(term, id);
      });

      const unlisten1 = tauriListen(`pty-data-${id}`, (payload: string) => term.write(payload));
      const unlisten2 = tauriListen(`pty-exit-${id}`, () => {
        term.write('\r\n\x1b[33m[Process exited — press any key to close]\x1b[0m\r\n');
      });

      Promise.all([unlisten1, unlisten2]).then((fns) => unlistenRefs.set(id, fns));

      term.onData((data: string) => tauriInvoke('pty_write', { id, data }).catch(() => {}));
      term.onResize(({ cols, rows }: any) => tauriInvoke('pty_resize', { id, cols, rows }).catch(() => {}));
    } else {
      startFallbackMode(term, id);
    }

    const ro = new ResizeObserver(() => fitAddon?.fit());
    ro.observe(container);
  }

  loaded.subscribe(($loaded) => {
    if ($loaded && get(tabs).length === 0) newTab();
  });

  themeName.subscribe(($themeName) => {
    termRefs.forEach(({ term }) => { if (term?.options) term.options.theme = THEMES[$themeName]; });
  });

  fontSize.subscribe(($fontSize) => {
    termRefs.forEach(({ term, fitAddon }) => {
      if (term?.options) term.options.fontSize = $fontSize;
      fitAddon?.fit();
    });
  });

  function dispose() {
    termRefs.forEach(({ term }) => term?.dispose());
    unlistenRefs.forEach((fns) => fns.forEach((fn) => fn()));
  }

  return { tabs, activeTab, themeName, fontSize, loaded, newTab, closeTab, initTerminal, dispose };
}

export type TerminalSession = ReturnType<typeof createTerminalSession>;
