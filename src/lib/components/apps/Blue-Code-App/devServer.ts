import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import { tauriListen } from '../Terminal-App/tauriBridge';

interface DevServerState {
  running: boolean; starting: boolean; command: string; port: number | null; log: string[]; url: string | null;
}

const PORT_RE = /(?:localhost|127\.0\.0\.1|0\.0\.0\.0):(\d{2,5})/;

/**
 * "Active Dev Mode" — detects and runs the project's dev server, reusing
 * the same PTY infrastructure as Blue Terminal (pty_create / pty_write /
 * pty_close via SystemBridge.invokeCommand), same as the original hook.
 */
export function createDevServer(rootPathStore: { subscribe: (fn: (v: string) => void) => () => void }) {
  const state = writable<DevServerState>({ running: false, starting: false, command: '', port: null, log: [], url: null });
  const detectedCommand = writable('');

  let rootPath = '';
  let ptyId: string | null = null;
  let unlisten: (() => void)[] = [];

  rootPathStore.subscribe(async (v) => {
    rootPath = v;
    if (!v) return;
    try {
      const pkgRaw = await SystemBridge.readFile(`${v}/package.json`).catch(() => null);
      if (pkgRaw) {
        const pkg = JSON.parse(pkgRaw);
        const scripts = pkg.scripts || {};
        const scriptName = scripts.dev ? 'dev' : scripts.start ? 'start' : null;
        if (scriptName) { detectedCommand.set(`npm run ${scriptName}`); return; }
      }
    } catch {}
    try { if (await SystemBridge.readFile(`${v}/Cargo.toml`).catch(() => null)) { detectedCommand.set('cargo run'); return; } } catch {}
    try { if (await SystemBridge.readFile(`${v}/manage.py`).catch(() => null)) { detectedCommand.set('python3 manage.py runserver'); return; } } catch {}
    detectedCommand.set('python3 -m http.server 8000');
  });

  function appendLog(line: string) { state.update((s) => ({ ...s, log: [...s.log.slice(-300), line] })); }

  async function start(command: string) {
    if (!SystemBridge.isTauri()) { appendLog('[Dev mode requires the desktop app — not available in browser preview]'); return; }
    const id = `devserver-${Date.now()}`;
    ptyId = id;
    state.update((s) => ({ ...s, starting: true, command, log: [`$ cd ${rootPath} && ${command}`], port: null, url: null }));

    try {
      await SystemBridge.invokeCommand('pty_create', { id, cols: 120, rows: 30 });

      const unData = await tauriListen(`pty-data-${id}`, (payload: string) => {
        appendLog(payload.replace(/\x1b\[[0-9;]*m/g, '').trimEnd());
        const match = payload.match(PORT_RE);
        if (match) {
          const port = parseInt(match[1], 10);
          state.update((s) => (s.port ? s : { ...s, port, url: `http://localhost:${port}`, running: true, starting: false }));
        }
      });
      const unExit = await tauriListen(`pty-exit-${id}`, () => {
        appendLog('[Process exited]');
        state.update((s) => ({ ...s, running: false, starting: false }));
      });
      unlisten = [unData, unExit];

      await SystemBridge.invokeCommand('pty_write', { id, data: `cd "${rootPath}" && ${command}\n` });
      state.update((s) => ({ ...s, running: true, starting: false }));
    } catch (e: any) {
      appendLog(`[Failed to start: ${e?.message ?? e}]`);
      state.update((s) => ({ ...s, starting: false, running: false }));
    }
  }

  async function stop() {
    if (!ptyId) return;
    try { await SystemBridge.invokeCommand('pty_close', { id: ptyId }); } catch {}
    unlisten.forEach((fn) => fn());
    unlisten = [];
    ptyId = null;
    state.update((s) => ({ ...s, running: false, starting: false, port: null, url: null }));
    appendLog('[Stopped]');
  }

  async function openInBrowser() {
    const url = get(state).url;
    if (!url) return;
    if (SystemBridge.isTauri()) await SystemBridge.invokeCommand('web_open_native', { url }).catch(() => {});
    else window.open(url, '_blank');
  }

  return { state, detectedCommand, start, stop, openInBrowser };
}

export type DevServerState2 = ReturnType<typeof createDevServer>;
