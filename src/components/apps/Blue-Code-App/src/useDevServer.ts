import { useState, useRef, useCallback, useEffect } from 'react';
import { SystemBridge } from '../../../../utils/systemBridge';

interface DevServerState {
    running: boolean;
    starting: boolean;
    command: string;
    port: number | null;
    log: string[];
    url: string | null;
}

const PORT_RE = /(?:localhost|127\.0\.0\.1|0\.0\.0\.0):(\d{2,5})/;

/**
 * "Active Dev Mode" — detects and runs the project's dev server (npm run
 * dev / cargo run / python -m http.server / etc.), reusing the same PTY
 * infrastructure as Blue Terminal (pty_create / pty_write / pty_close) so
 * no new backend process-management code is needed. Output is scanned for
 * a "host:port" pattern to auto-detect which port the server bound to,
 * and "Open" hands that URL to Blue Web's native-webview opener
 * (web_open_native) so the running app can be viewed without leaving the
 * shell's window management.
 */
export function useDevServer(rootPath: string) {
    const [state, setState] = useState<DevServerState>({
        running: false, starting: false, command: '', port: null, log: [], url: null,
    });
    const [detectedCommand, setDetectedCommand] = useState<string>('');
    const ptyIdRef = useRef<string | null>(null);
    const unlistenRef = useRef<(() => void)[]>([]);

    // Try to detect a sensible default dev command from the workspace root.
    useEffect(() => {
        if (!rootPath) return;
        let cancelled = false;

        (async () => {
            try {
                const pkgRaw = await SystemBridge.readFile(`${rootPath}/package.json`).catch(() => null);
                if (pkgRaw) {
                    const pkg = JSON.parse(pkgRaw);
                    const scripts = pkg.scripts || {};
                    const scriptName = scripts.dev ? 'dev' : scripts.start ? 'start' : null;
                    if (scriptName && !cancelled) {
                        setDetectedCommand(`npm run ${scriptName}`);
                        return;
                    }
                }
            } catch { /* not an npm project */ }

            try {
                const hasCargo = await SystemBridge.readFile(`${rootPath}/Cargo.toml`).catch(() => null);
                if (hasCargo && !cancelled) { setDetectedCommand('cargo run'); return; }
            } catch {}

            try {
                const hasManagePy = await SystemBridge.readFile(`${rootPath}/manage.py`).catch(() => null);
                if (hasManagePy && !cancelled) { setDetectedCommand('python3 manage.py runserver'); return; }
            } catch {}

            if (!cancelled) setDetectedCommand('python3 -m http.server 8000');
        })();

        return () => { cancelled = true; };
    }, [rootPath]);

    const appendLog = useCallback((line: string) => {
        setState(s => ({ ...s, log: [...s.log.slice(-300), line] }));
    }, []);

    const start = useCallback(async (command: string) => {
        if (!SystemBridge.isTauri()) {
            appendLog('[Dev mode requires the desktop app — not available in browser preview]');
            return;
        }
        const id = `devserver-${Date.now()}`;
        ptyIdRef.current = id;
        setState(s => ({ ...s, starting: true, command, log: [`$ cd ${rootPath} && ${command}`], port: null, url: null }));

        try {
            await SystemBridge.invoke('pty_create', { id, cols: 120, rows: 30 });

            const { listen } = await import('@tauri-apps/api/event');
            const unData = await listen(`pty-data-${id}`, (e: any) => {
                const text: string = e.payload;
                appendLog(text.replace(/\x1b\[[0-9;]*m/g, '').trimEnd());
                const match = text.match(PORT_RE);
                if (match) {
                    const port = parseInt(match[1], 10);
                    setState(s => s.port ? s : { ...s, port, url: `http://localhost:${port}`, running: true, starting: false });
                }
            });
            const unExit = await listen(`pty-exit-${id}`, () => {
                appendLog('[Process exited]');
                setState(s => ({ ...s, running: false, starting: false }));
            });
            unlistenRef.current = [unData, unExit];

            await SystemBridge.invoke('pty_write', { id, data: `cd "${rootPath}" && ${command}\n` });
            setState(s => ({ ...s, running: true, starting: false }));
        } catch (e: any) {
            appendLog(`[Failed to start: ${e?.message ?? e}]`);
            setState(s => ({ ...s, starting: false, running: false }));
        }
    }, [rootPath, appendLog]);

    const stop = useCallback(async () => {
        const id = ptyIdRef.current;
        if (!id) return;
        try { await SystemBridge.invoke('pty_close', { id }); } catch {}
        unlistenRef.current.forEach(fn => fn());
        unlistenRef.current = [];
        ptyIdRef.current = null;
        setState(s => ({ ...s, running: false, starting: false, port: null, url: null }));
        appendLog('[Stopped]');
    }, [appendLog]);

    const openInBrowser = useCallback(async () => {
        if (!state.url) return;
        if (SystemBridge.isTauri()) {
            await SystemBridge.invoke('web_open_native', { url: state.url }).catch(() => {});
        } else {
            window.open(state.url, '_blank');
        }
    }, [state.url]);

    useEffect(() => () => { unlistenRef.current.forEach(fn => fn()); }, []);

    return { ...state, detectedCommand, start, stop, openInBrowser };
}

export type DevServerState2 = ReturnType<typeof useDevServer>;
