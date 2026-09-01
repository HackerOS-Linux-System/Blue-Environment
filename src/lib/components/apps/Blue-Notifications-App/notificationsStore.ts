import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import { notificationManager } from '../../../utils/notificationManager';
import type { NotificationRule } from './types';

function newId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

// Module-level (not per-component-instance) so polling keeps running
// across the Blue Notifications *window* opening/closing — see
// mod.rs's module doc on the polling model. One shared set of timers
// for the whole app session, started once from App.svelte.
const timers = new Map<string, ReturnType<typeof setInterval>>();
let pollingStarted = false;

export function createNotificationsStore() {
  const rules = writable<NotificationRule[]>([]);
  const loading = writable(true);
  const error = writable<string | null>(null);
  const checking = writable<Set<string>>(new Set());

  async function load() {
    loading.set(true);
    try {
      rules.set(await SystemBridge.notifRulesLoad());
    } finally {
      loading.set(false);
    }
  }

  async function saveRule(rule: Omit<NotificationRule, 'id' | 'lastSeenGuids'> & { id?: string }): Promise<NotificationRule> {
    const full: NotificationRule = { ...rule, id: rule.id ?? newId(), lastSeenGuids: [] };
    const res = await SystemBridge.notifRulesSave(full);
    if (!res.ok) { error.set(res.error ?? 'Failed to save rule'); return full; }
    rules.update((prev) => {
      const idx = prev.findIndex((r) => r.id === full.id);
      if (idx >= 0) { const next = [...prev]; next[idx] = { ...prev[idx], ...full, lastSeenGuids: prev[idx].lastSeenGuids }; return next; }
      return [...prev, full];
    });
    restartTimerFor(full);
    return full;
  }

  async function deleteRule(id: string) {
    const res = await SystemBridge.notifRulesDelete(id);
    if (!res.ok) { error.set(res.error ?? 'Failed to delete rule'); return; }
    rules.update((prev) => prev.filter((r) => r.id !== id));
    const t = timers.get(id);
    if (t) { clearInterval(t); timers.delete(id); }
  }

  /**
   * Checks one rule right now (outside its normal interval — used by
   * the UI's manual "Check now" button and by the polling loop itself).
   * New items get pushed into the shell's shared `notificationManager`
   * — the same bus every other real desktop notification in this shell
   * goes through (see mod.rs's module doc for why that's the design,
   * not a Blue-Notifications-specific alert popup).
   */
  async function checkNow(rule: NotificationRule) {
    checking.update((s) => { const n = new Set(s); n.add(rule.id); return n; });
    try {
      const res = await SystemBridge.notifCheckFeed(rule);
      if (res.ok) {
        for (const item of res.newItems) {
          notificationManager.add({
            title: rule.name,
            message: item.title,
            body: item.link,
            app: 'Blue Notifications',
            appId: 'blue_notifications',
          });
        }
        // Refresh this rule's persisted `lastSeenGuids` from the
        // backend rather than guessing at it locally — the backend is
        // the source of truth for the seen-set (it also caps/dedups it).
        const updated = await SystemBridge.notifRulesLoad();
        const match = updated.find((r) => r.id === rule.id);
        if (match) rules.update((prev) => prev.map((r) => (r.id === rule.id ? match : r)));
      } else if (res.error) {
        error.set(res.error);
      }
    } finally {
      checking.update((s) => { const n = new Set(s); n.delete(rule.id); return n; });
    }
  }

  function restartTimerFor(rule: NotificationRule) {
    const existing = timers.get(rule.id);
    if (existing) clearInterval(existing);
    if (!rule.enabled) return;
    const ms = Math.max(1, rule.intervalMinutes) * 60_000;
    const id = setInterval(() => checkNow(rule), ms);
    timers.set(rule.id, id);
  }

  /**
   * Starts (or is a no-op if already running) the background polling
   * loop for every enabled rule. Called unconditionally from
   * App.svelte's `onMount` — not from this app's own component mount —
   * so feed checks keep happening for the whole shell session
   * regardless of whether a Blue Notifications window is ever opened.
   * See mod.rs's module doc on exactly what "background" does and
   * doesn't mean here (no OS-level daemon, just a `setInterval` that
   * lives as long as the shell process does).
   */
  async function startPolling() {
    if (pollingStarted) return;
    pollingStarted = true;
    const all = await SystemBridge.notifRulesLoad();
    for (const rule of all) restartTimerFor(rule);
  }

  return { rules, loading, error, checking, load, saveRule, deleteRule, checkNow, startPolling };
}

export type NotificationsStore = ReturnType<typeof createNotificationsStore>;
