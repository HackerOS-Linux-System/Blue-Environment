export function isTauriEnv(): boolean {
    return typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;
}

export async function tauriInvoke(cmd: string, args?: any): Promise<any> {
    const core = await import('@tauri-apps/api/core');
    return core.invoke(cmd, args);
}

export async function tauriListen(event: string, handler: (payload: any) => void): Promise<() => void> {
    const { listen } = await import('@tauri-apps/api/event');
    return listen(event, (e: any) => handler(e.payload));
}
