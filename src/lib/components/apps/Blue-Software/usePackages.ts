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

  async function loadPackages() {
    loading.set(true);
    error.set(null);
    try {
      const [native, flatpak, appimage] = await Promise.all([
        SystemBridge.invokeCommand<PackageInfo[]>('get_native_packages').catch((): PackageInfo[] => []),
        SystemBridge.getFlatpakPackages().catch((): PackageInfo[] => []),
        SystemBridge.getAppImagePackages().catch((): PackageInfo[] => []),
      ]);
      packages.set([...native, ...flatpak, ...appimage]);
    } catch (e: any) {
      error.set(e?.message ?? String(e));
    } finally {
      loading.set(false);
    }
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
    packages, loading, error, activeAction, installLog, backend, bootcStatus,
    loadPackages, performAction, bootcUpgrade,
    closeLog: () => installLog.set(null),
  };
}
