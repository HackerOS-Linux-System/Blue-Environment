import { writable, get } from 'svelte/store';
import { SystemBridge, type PackageInfo } from '../../../utils/systemBridge';
import type { InstallLog } from './types';

export type PkgBackend = 'Dnf' | 'Apt' | 'Pacman' | 'Zypper' | 'RpmOstree' | 'Flatpak' | 'AppImage' | 'Unknown';

export interface BootcStatus {
  image: string;
  version: string;
  booted_digest: string;
  staged_image?: string;
  update_pending: boolean;
}

export function createPackages() {
  const packages = writable<PackageInfo[]>([]);
  const loading = writable(false);
  const error = writable<string | null>(null);
  const activeAction = writable<string | null>(null);
  const installLog = writable<InstallLog | null>(null);
  const backend = writable<PkgBackend>('Unknown');
  const bootcStatus = writable<BootcStatus | null>(null);

  SystemBridge.invokeCommand<string>('get_detected_backend').then((b) => backend.set(b as PkgBackend)).catch(() => {});
  SystemBridge.invokeCommand<BootcStatus | null>('get_bootc_status').then((s) => bootcStatus.set(s)).catch(() => {});

  // Loading is now PROGRESSIVE. The old version awaited native + flatpak +
  // appimage together (Promise.all) and painted nothing until the slowest
  // one finished — and the native query also did network work (update
  // checks, "Available" repoquery) and per-package icon lookups, so on a
  // real system the spinner could run for minutes or hit the timeout with
  // an empty list. Now:
  //   1. `get_installed_packages_fast` (local DB only) paints the
  //      Installed list right away;
  //   2. the full native query (updates + Available suggestions), Flatpak
  //      and AppImage each merge in as soon as THEY finish, each with its
  //      own timeout, so one hung source never blocks the others.
  const SOURCE_TIMEOUT_MS = 45_000;
  const refreshing = writable(false);

  function withTimeout<T>(p: Promise<T>, label: string): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const t = setTimeout(() => reject(new Error(`${label} timed out`)), SOURCE_TIMEOUT_MS);
      p.then((v) => { clearTimeout(t); resolve(v); }, (e) => { clearTimeout(t); reject(e); });
    });
  }

  // Replace every package of `source`-group with fresh data, keep the rest.
  function mergeGroup(group: 'native' | 'flatpak' | 'appimage', incoming: PackageInfo[]) {
    const inGroup = (p: PackageInfo) =>
      group === 'flatpak' ? p.source === 'flatpak' : group === 'appimage' ? p.source === 'appimage' : !['flatpak', 'appimage'].includes(p.source);
    packages.update((prev) => [...prev.filter((p) => !inGroup(p)), ...incoming]);
  }

  let loadSeq = 0;
  async function loadPackages() {
    const seq = ++loadSeq;
    loading.set(true);
    refreshing.set(true);
    error.set(null);
    const failures: string[] = [];

    // 1. Fast local list → UI is usable immediately.
    try {
      const fast = await withTimeout(SystemBridge.invokeCommand<PackageInfo[]>('get_installed_packages_fast'), 'Installed packages');
      if (seq === loadSeq && fast?.length) mergeGroup('native', fast);
    } catch (e: any) {
      failures.push(e?.message ?? String(e));
    }
    if (seq === loadSeq) loading.set(false);

    // 2. Slower sources, all in parallel, each merged when it completes.
    const tasks: Promise<void>[] = [
      withTimeout(SystemBridge.invokeCommand<PackageInfo[]>('get_native_packages'), 'System packages')
        .then((r) => { if (seq === loadSeq && r?.length) mergeGroup('native', r); })
        .catch((e) => { failures.push(e?.message ?? String(e)); }),
      withTimeout(SystemBridge.getFlatpakPackages(), 'Flatpak')
        .then((r) => { if (seq === loadSeq) mergeGroup('flatpak', r ?? []); })
        .catch((e) => { failures.push(e?.message ?? String(e)); }),
      withTimeout(SystemBridge.getAppImagePackages(), 'AppImage')
        .then((r) => { if (seq === loadSeq) mergeGroup('appimage', r ?? []); })
        .catch((e) => { failures.push(e?.message ?? String(e)); }),
    ];
    await Promise.all(tasks);

    if (seq !== loadSeq) return;
    if (failures.length && get(packages).length === 0) {
      error.set(`Could not load packages: ${failures.join('; ')}. Check your network connection and package manager, then refresh.`);
    } else if (failures.length) {
      error.set(`Some sources did not respond (${failures.join('; ')}). Showing what was loaded.`);
    }
    loading.set(false);
    refreshing.set(false);
  }

  function addLog(line: string) {
    installLog.update((prev) => (prev ? { ...prev, lines: [...prev.lines, line] } : null));
  }

  async function performAction(pkg: PackageInfo, action: 'install' | 'remove' | 'update') {
    activeAction.set(pkg.id);
    const label = action === 'install' ? 'Installing' : action === 'remove' ? 'Removing' : 'Updating';
    installLog.set({ pkgId: pkg.id, lines: [`${label} ${pkg.name}…`, `Backend: ${get(backend)}`], done: false, success: false });

    try {
      addLog(`Source: ${pkg.source}`);
      let ok = false;
      const isNative = !['flatpak', 'appimage'].includes(pkg.source);

      if (action === 'install') {
        if (isNative) ok = await SystemBridge.invokeCommand<boolean>('install_native_package', { pkgId: pkg.id });
        else if (pkg.source === 'flatpak') ok = await SystemBridge.installFlatpakPackage(pkg.id);
        else ok = await SystemBridge.installAppImage(pkg.id);
      } else if (action === 'remove') {
        if (isNative) ok = await SystemBridge.invokeCommand<boolean>('remove_native_package', { pkgId: pkg.id });
        else if (pkg.source === 'flatpak') ok = await SystemBridge.removeFlatpakPackage(pkg.id);
        else ok = await SystemBridge.removeAppImage(pkg.id);
      } else {
        if (isNative) ok = await SystemBridge.invokeCommand<boolean>('install_native_package', { pkgId: pkg.id });
        else if (pkg.source === 'flatpak') ok = await SystemBridge.updateFlatpakPackage(pkg.id);
        else ok = false;
      }

      addLog(ok ? 'Done.' : 'Failed.');
      installLog.update((prev) => (prev ? { ...prev, done: true, success: ok } : null));

      if (ok) {
        packages.update((prev) =>
          action === 'remove' ? prev.filter((p) => p.id !== pkg.id) : prev.map((p) => (p.id === pkg.id ? { ...p, installed: action === 'install' } : p))
        );
      }
    } catch (e: any) {
      addLog(`Error: ${e?.message ?? String(e)}`);
      installLog.update((prev) => (prev ? { ...prev, done: true, success: false } : null));
    } finally {
      activeAction.set(null);
    }
  }

  async function bootcUpgrade() {
    installLog.set({ pkgId: '__bootc__', lines: ['Upgrading system image via bootc…'], done: false, success: false });
    const ok = await SystemBridge.invokeCommand<boolean>('bootc_upgrade').catch(() => false);
    installLog.update((prev) =>
      prev ? { ...prev, done: true, success: ok, lines: [...prev.lines, ok ? 'Upgrade staged. Reboot to apply.' : 'Upgrade failed.'] } : null
    );
    if (ok) bootcStatus.update((prev) => (prev ? { ...prev, update_pending: true } : null));
  }

  return {
    packages, loading, refreshing, error, activeAction, installLog, backend, bootcStatus,
    loadPackages, performAction, bootcUpgrade,
    closeLog: () => installLog.set(null),
  };
}
