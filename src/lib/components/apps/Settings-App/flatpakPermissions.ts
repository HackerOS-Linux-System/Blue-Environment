export interface FlatpakApp { id: string; name: string; version: string; }

/** Parses `flatpak list --app --columns=application,name,version` (tab separated; header optional). */
export function parseFlatpakList(out: string): FlatpakApp[] {
  const apps: FlatpakApp[] = [];
  for (const line of out.split('\n')) {
    if (!line.trim()) continue;
    const [id, name = '', version = ''] = line.split('\t').map((c) => c.trim());
    if (!id || /^application( id)?$/i.test(id)) continue;
    if (!/^[A-Za-z0-9_.-]+$/.test(id)) continue;
    apps.push({ id, name: name || id, version });
  }
  return apps.sort((a, b) => a.name.localeCompare(b.name));
}

// ── static context (permissions an app declares / user overrides) ─────────

export type ContextKey = 'shared' | 'sockets' | 'devices' | 'features' | 'filesystems';
export type Context = Record<ContextKey, string[]>;

export const emptyContext = (): Context => ({ shared: [], sockets: [], devices: [], features: [], filesystems: [] });

/** Parses the keyfile printed by `flatpak info --show-permissions` / `flatpak override --show`. */
export function parseContext(keyfile: string): Context {
  const ctx = emptyContext();
  let inContext = false;
  for (const raw of keyfile.split('\n')) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    if (line.startsWith('[')) { inContext = line === '[Context]'; continue; }
    if (!inContext) continue;
    const eq = line.indexOf('=');
    if (eq < 0) continue;
    const key = line.slice(0, eq).trim() as ContextKey;
    if (!(key in ctx)) continue;
    ctx[key] = line.slice(eq + 1).split(';').map((v) => v.trim()).filter(Boolean);
  }
  return ctx;
}

/** Applies user overrides (`x` adds, `!x` removes) on top of the app's own metadata. */
export function mergeContext(base: Context, overrides: Context): Context {
  const out = emptyContext();
  (Object.keys(out) as ContextKey[]).forEach((k) => {
    const set = new Set(base[k]);
    for (const v of overrides[k]) {
      if (v.startsWith('!')) set.delete(v.slice(1)); else set.add(v);
    }
    out[k] = [...set];
  });
  return out;
}

/** Whether a filesystem entry (`home`, `home:ro`, `~/Docs:rw`…) grants the given base path. */
export function hasFilesystem(list: string[], base: string): boolean {
  return list.some((f) => f === base || f.startsWith(base + ':'));
}

/** A toggle in the "system access" group. */
export interface SystemToggle {
  id: string;
  kind: 'share' | 'socket' | 'device' | 'feature' | 'filesystem';
  value: string;
  labelKey: string;
}

export const SYSTEM_TOGGLES: SystemToggle[] = [
  { id: 'network', kind: 'share', value: 'network', labelKey: 'permissions.network' },
  { id: 'ipc', kind: 'share', value: 'ipc', labelKey: 'permissions.ipc' },
  { id: 'wayland', kind: 'socket', value: 'wayland', labelKey: 'permissions.wayland' },
  { id: 'x11', kind: 'socket', value: 'x11', labelKey: 'permissions.x11' },
  { id: 'pulseaudio', kind: 'socket', value: 'pulseaudio', labelKey: 'permissions.audio' },
  { id: 'dri', kind: 'device', value: 'dri', labelKey: 'permissions.dri' },
  { id: 'all-devices', kind: 'device', value: 'all', labelKey: 'permissions.all_devices' },
  { id: 'kvm', kind: 'device', value: 'kvm', labelKey: 'permissions.kvm' },
  { id: 'bluetooth', kind: 'feature', value: 'bluetooth', labelKey: 'permissions.bluetooth' },
  { id: 'fs-host', kind: 'filesystem', value: 'host', labelKey: 'permissions.fs_host' },
  { id: 'fs-home', kind: 'filesystem', value: 'home', labelKey: 'permissions.fs_home' },
];

export function isToggleOn(ctx: Context, t: SystemToggle): boolean {
  switch (t.kind) {
    case 'share': return ctx.shared.includes(t.value);
    case 'socket': return ctx.sockets.includes(t.value);
    case 'device': return ctx.devices.includes(t.value);
    case 'feature': return ctx.features.includes(t.value);
    case 'filesystem': return hasFilesystem(ctx.filesystems, t.value);
  }
}

const APP_ID_RE = /^[A-Za-z0-9_]+(\.[A-Za-z0-9_-]+)+$/;
export const isValidAppId = (id: string) => APP_ID_RE.test(id);

/** `flatpak override --user …` arguments for flipping one toggle. */
export function overrideArgs(appId: string, t: SystemToggle, enable: boolean): string[] {
  if (!isValidAppId(appId)) throw new Error(`invalid app id: ${appId}`);
  const flag = (() => {
    switch (t.kind) {
      case 'share': return enable ? '--share' : '--unshare';
      case 'socket': return enable ? '--socket' : '--nosocket';
      case 'device': return enable ? '--device' : '--nodevice';
      case 'feature': return enable ? '--allow' : '--disallow';
      case 'filesystem': return enable ? '--filesystem' : '--nofilesystem';
    }
  })();
  return ['override', '--user', `${flag}=${t.value}`, appId];
}

// ── portal permission store (camera, microphone, location, …) ─────────────

export interface PortalPermission {
  id: string;
  table: string;
  object: string;
  labelKey: string;
  /** value written when the toggle is switched on / off */
  on: string;
  off: string;
}

export const PORTAL_PERMISSIONS: PortalPermission[] = [
  { id: 'camera', table: 'devices', object: 'camera', labelKey: 'permissions.camera', on: 'yes', off: 'no' },
  { id: 'microphone', table: 'devices', object: 'microphone', labelKey: 'permissions.microphone', on: 'yes', off: 'no' },
  { id: 'speakers', table: 'devices', object: 'speakers', labelKey: 'permissions.speakers', on: 'yes', off: 'no' },
  { id: 'location', table: 'location', object: 'location', labelKey: 'permissions.location', on: 'EXACT', off: 'NONE' },
  { id: 'notifications', table: 'notifications', object: 'notification', labelKey: 'permissions.notifications', on: 'yes', off: 'no' },
  { id: 'screenshot', table: 'screenshot', object: 'screenshot', labelKey: 'permissions.screenshot', on: 'yes', off: 'no' },
  { id: 'background', table: 'background', object: 'background', labelKey: 'permissions.background', on: 'yes', off: 'no' },
];

export type PortalState = 'granted' | 'denied' | 'unset';

const DENY = new Set(['no', 'none', 'deny', 'denied', 'false']);
const ASK = new Set(['ask']);

/** Parses `flatpak permission-show APP` into `{ "table/object": state }`. Unlisted entries are 'unset'. */
export function parsePermissionShow(out: string): Record<string, PortalState> {
  const res: Record<string, PortalState> = {};
  for (const line of out.split('\n')) {
    const cells = line.trim().split(/\s+/);
    if (cells.length < 4 || /^table$/i.test(cells[0])) continue;
    const [table, object, , perm] = cells;
    const first = perm.split(',')[0].trim().toLowerCase();
    res[`${table}/${object}`] = DENY.has(first) ? 'denied' : ASK.has(first) ? 'unset' : 'granted';
  }
  return res;
}

export function portalState(map: Record<string, PortalState>, p: PortalPermission): PortalState {
  return map[`${p.table}/${p.object}`] ?? 'unset';
}

/** `flatpak permission-set TABLE ID APP VALUE` arguments. */
export function portalSetArgs(appId: string, p: PortalPermission, enable: boolean): string[] {
  if (!isValidAppId(appId)) throw new Error(`invalid app id: ${appId}`);
  return ['permission-set', p.table, p.object, appId, enable ? p.on : p.off];
}

/** Arguments that wipe everything this panel can change for one app. */
export const resetArgs = (appId: string): string[][] => {
  if (!isValidAppId(appId)) throw new Error(`invalid app id: ${appId}`);
  return [['permission-reset', appId], ['override', '--user', '--reset', appId]];
};

/** Does the user override anything for this app? (used for the "modified" badge) */
export function hasOverrides(o: Context): boolean {
  return (Object.keys(o) as ContextKey[]).some((k) => o[k].length > 0);
}
