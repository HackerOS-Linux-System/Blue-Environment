import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import { clearLiveMode } from '../../../utils/liveMode';
import type { InstallStep, InstallConfig, DiskInfo, PartitionPlanEntry } from './types';

function out(r: any): string { return typeof r === 'string' ? r : (r?.stdout ?? '') + (r?.stderr ?? ''); }

/** Default two-partition layout, mirrored from blue-installer-apply.rb's hardcoded "erase" path. */
export function defaultPartitionPlan(): PartitionPlanEntry[] {
  return [
    { id: 'p-esp', role: 'esp', filesystem: 'fat32', mountpoint: '/boot/efi', sizeMiB: 512 },
    { id: 'p-root', role: 'root', filesystem: 'ext4', mountpoint: '/', sizeMiB: null },
  ];
}

export function createInstallState() {
  const step = writable<InstallStep>('welcome');
  const config = writable<InstallConfig>({
    locale: 'en_US.UTF-8', keyboardLayout: 'us', disk: null, diskMode: 'erase',
    partitions: defaultPartitionPlan(),
    hostname: 'blue-pc', username: '', fullName: '', password: '', autoLogin: false,
  });

  const disks = writable<DiskInfo[]>([]);
  const disksLoading = writable(false);
  const progressPct = writable(0);
  const progressLabel = writable('');
  const installLog = writable<string[]>([]);
  const installError = writable<string | null>(null);

  async function loadDisks() {
    disksLoading.set(true);
    try {
      const result = await SystemBridge.executeCommand(
        `lsblk -d -b -o NAME,SIZE,MODEL,RM,TYPE -J 2>/dev/null`
      );
      const json = JSON.parse(out(result) || '{}');
      const list: DiskInfo[] = (json.blockdevices ?? [])
        .filter((d: any) => d.type === 'disk')
        .map((d: any) => ({
          path: `/dev/${d.name}`,
          model: (d.model ?? 'Unknown disk').trim(),
          sizeBytes: parseInt(d.size, 10) || 0,
          sizeLabel: formatSize(parseInt(d.size, 10) || 0),
          removable: d.rm === '1' || d.rm === true,
        }));
      disks.set(list);
    } catch {
      disks.set([]);
    } finally {
      disksLoading.set(false);
    }
  }

  function formatSize(bytes: number): string {
    if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1)} TB`;
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(0)} GB`;
    return `${(bytes / 1e6).toFixed(0)} MB`;
  }

  /** Basic sanity checks before allowing "Next": an ESP, a root mount, and only one "rest of disk" entry. */
  function validatePartitionPlan(plan: PartitionPlanEntry[]): string | null {
    if (!plan.some((p) => p.role === 'esp')) return 'A manual layout needs an EFI System Partition (ESP).';
    if (!plan.some((p) => p.mountpoint === '/')) return 'A manual layout needs a partition mounted at /.';
    const openEnded = plan.filter((p) => p.sizeMiB === null);
    if (openEnded.length > 1) return 'Only one partition can use "remaining space".';
    if (plan.some((p) => p.sizeMiB !== null && p.sizeMiB < 32)) return 'Partitions must be at least 32 MiB.';
    return null;
  }

  function next() {
    const order: InstallStep[] = ['welcome', 'language', 'keyboard', 'disk', 'account', 'summary', 'installing', 'done'];
    const i = order.indexOf(get(step));
    if (i >= 0 && i < order.length - 1) step.set(order[i + 1]);
  }
  function back() {
    const order: InstallStep[] = ['welcome', 'language', 'keyboard', 'disk', 'account', 'summary'];
    const i = order.indexOf(get(step));
    if (i > 0) step.set(order[i - 1]);
  }

  /**
   * Hands off to installer/blue-installer-apply.sh via pkexec (polkit —
   * prompts for the live user's password once, then runs as root). The
   * script streams progress lines of the form "PROGRESS <pct> <label>" on
   * stdout, which we parse live; see the script itself for the full,
   * heavily-commented step list (partitioning, rootfs copy, chroot config,
   * bootloader, user creation).
   *
   * SAFETY: this is the one place in the whole app that can destroy data.
   * The script itself re-validates the disk path and requires an explicit
   * `--confirm-erase <path>` argument that must match, as a second guard
   * beyond this UI's own confirmation step.
   */
  async function startInstall() {
    step.set('installing');
    progressPct.set(0);
    installLog.set([]);
    installError.set(null);

    const cfg = get(config);
    if (!cfg.disk) { installError.set('No target disk selected'); step.set('error'); return; }

    const args = [
      `--disk ${cfg.disk.path}`,
      `--confirm-erase ${cfg.disk.path}`,
      `--locale ${cfg.locale}`,
      `--keyboard ${cfg.keyboardLayout}`,
      `--hostname ${JSON.stringify(cfg.hostname)}`,
      `--username ${JSON.stringify(cfg.username)}`,
      `--fullname ${JSON.stringify(cfg.fullName || cfg.username)}`,
      cfg.autoLogin ? '--autologin' : '',
      // Manual partitioning: hand the whole plan to the privileged backend as JSON.
      // The default "erase" mode omits this and blue-installer-apply.rb falls back
      // to its built-in two-partition (ESP + root) layout.
      cfg.diskMode === 'manual' && cfg.partitions.length
        ? `--partitions ${JSON.stringify(JSON.stringify(cfg.partitions))}`
        : '',
    ].filter(Boolean).join(' ');

    try {
      if (!SystemBridge.isTauri()) {
        installError.set('Installation requires the desktop app (Tauri) — not available in browser preview.');
        step.set('error');
        return;
      }

      // Password is piped via stdin (never placed on the command line /
      // process listing) — see script's `read -r -s PASSWORD` step.
      await SystemBridge.invokeCommand('installer_run', { args, password: cfg.password });
      // installer_run is expected to emit `installer-progress` events while
      // running; the caller (BlueInstallerApp.svelte) subscribes to those
      // via tauriListen and updates progressPct/installLog directly.
    } catch (e: any) {
      installError.set(e?.message ?? String(e));
      step.set('error');
    }
  }

  async function finishAndReboot() {
    await clearLiveMode();
    await SystemBridge.powerAction('reboot').catch(() => {});
  }

  return {
    step, config, disks, disksLoading, progressPct, progressLabel, installLog, installError,
    loadDisks, next, back, startInstall, finishAndReboot, validatePartitionPlan,
  };
}

export type InstallState = ReturnType<typeof createInstallState>;
