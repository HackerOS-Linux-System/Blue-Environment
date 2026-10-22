import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { Author } from './docReview';

/**
 * Author profiles for comments and tracked changes. Several can exist side by side (e.g. two
 * people sharing one machine, or "me" and "reviewer"), one is active at a time. They are stored per
 * OS user in `~/.config/Blue-Environment/docs-profiles.json`.
 */
export interface DocsProfile extends Author { id: string }

const PATH = 'HOME/.config/Blue-Environment/docs-profiles.json';
export const PROFILE_COLORS = ['#3b82f6', '#ef4444', '#10b981', '#f59e0b', '#8b5cf6', '#ec4899', '#14b8a6', '#f97316'];

export interface ProfileState { profiles: DocsProfile[]; activeId: string }

export function defaultState(userName: string): ProfileState {
  const p: DocsProfile = { id: 'p-default', name: userName || 'Me', color: PROFILE_COLORS[0] };
  return { profiles: [p], activeId: p.id };
}

/** Tolerant parser — falls back to a default profile when the file is missing or damaged. */
export function parseProfiles(raw: string | null | undefined, userName: string): ProfileState {
  try {
    const v = JSON.parse(raw ?? '');
    const profiles: DocsProfile[] = (Array.isArray(v?.profiles) ? v.profiles : [])
      .filter((p: any) => p && typeof p.id === 'string' && typeof p.name === 'string' && p.name.trim())
      .map((p: any) => ({ id: p.id, name: p.name.trim().slice(0, 40), color: typeof p.color === 'string' ? p.color : PROFILE_COLORS[0] }));
    if (!profiles.length) return defaultState(userName);
    const activeId = profiles.some((p) => p.id === v.activeId) ? v.activeId : profiles[0].id;
    return { profiles, activeId };
  } catch { return defaultState(userName); }
}

export function addProfile(s: ProfileState, name: string): ProfileState {
  const clean = name.trim().slice(0, 40);
  if (!clean || s.profiles.some((p) => p.name.toLowerCase() === clean.toLowerCase())) return s;
  const p: DocsProfile = { id: `p-${Date.now().toString(36)}-${s.profiles.length}`, name: clean, color: PROFILE_COLORS[s.profiles.length % PROFILE_COLORS.length] };
  return { profiles: [...s.profiles, p], activeId: p.id };
}

export function removeProfile(s: ProfileState, id: string): ProfileState {
  if (s.profiles.length <= 1) return s;
  const profiles = s.profiles.filter((p) => p.id !== id);
  return { profiles, activeId: s.activeId === id ? profiles[0].id : s.activeId };
}

export const profileState = writable<ProfileState>(defaultState(''));
let loaded = false;

export async function loadProfiles() {
  if (loaded) return;
  const user = SystemBridge.getUsername();
  let raw: string | null = null;
  try { raw = await SystemBridge.readFile(PATH); } catch { /* first run */ }
  profileState.set(parseProfiles(raw, user));
  loaded = true;
}

export function updateProfiles(fn: (s: ProfileState) => ProfileState) {
  profileState.update(fn);
  if (loaded) SystemBridge.writeFile(PATH, JSON.stringify(get(profileState))).catch(() => {});
}

export const activeAuthor = (s: ProfileState): Author => {
  const p = s.profiles.find((x) => x.id === s.activeId) ?? s.profiles[0];
  return { name: p.name, color: p.color };
};
