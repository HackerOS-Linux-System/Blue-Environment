<script lang="ts">
  import { X, Bell, Trash2, Check } from 'lucide-svelte';
  import { notificationManager } from '../utils/notificationManager';
  import type { Notification } from '../types';
  import { createEventDispatcher } from 'svelte';

  export let isOpen = false;
  // Same hardcoded-`top-14` bug ControlCenter.svelte had (found while
  // fixing that one, for the same reason — a bottom-positioned panel,
  // e.g. the Hydra shell theme, needs this popup to open from the
  // bottom instead) — fixed here too rather than leaving one sibling
  // popup correct and the other still broken.
  export let panelPosition: 'top' | 'bottom' = 'top';
  export let panelSize = 48;
  export let shellThemeId: string | undefined = undefined;

  const dispatch = createEventDispatcher<{ close: void }>();

  let notifications: Notification[] = [];
  let unsub: (() => void) | undefined;

  $: if (isOpen) {
    unsub?.();
    unsub = notificationManager.subscribe((n) => (notifications = n));
  } else {
    unsub?.();
    unsub = undefined;
  }

  function formatTime(ts: number) {
    return new Date(ts).toLocaleTimeString();
  }
</script>

{#if isOpen}
  <div class="absolute right-4 w-96 border rounded-3xl shadow-2xl z-40 flex flex-col overflow-hidden {shellThemeId === 'hydra' ? 'bg-slate-900 border-pink-500/30' : 'bg-slate-900 border-white/10'}"
    style="{panelPosition === 'top' ? `top:${panelSize + 8}px; bottom:16px;` : `top:16px; bottom:${panelSize + 8}px;`} {shellThemeId === 'hydra' ? 'box-shadow: 0 0 40px rgba(236,72,153,0.2);' : ''}"
  >
    <div class="p-5 border-b border-white/5 flex items-center justify-between">
      <h2 class="font-semibold text-lg text-white flex items-center gap-2">
        <Bell size={18} /> Notifications
      </h2>
      <div class="flex gap-2">
        <button on:click={() => notificationManager.clearAll()} class="text-slate-400 hover:text-white bg-white/5 p-1.5 rounded-full transition-colors" title="Clear all">
          <Trash2 size={15} />
        </button>
        <button on:click={() => dispatch('close')} class="text-slate-400 hover:text-white bg-white/5 p-1.5 rounded-full transition-colors">
          <X size={15} />
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-4 space-y-3">
      {#if notifications.length === 0}
        <div class="text-center text-slate-500 py-12 flex flex-col items-center gap-3">
          <Bell size={32} class="opacity-30" />
          <p>No notifications</p>
        </div>
      {/if}
      {#each notifications as notif (notif.id)}
        <div class="bg-slate-800 border-l-4 {notif.read ? 'border-slate-600' : 'border-blue-500'} rounded-xl p-3">
          <div class="flex items-start justify-between">
            <div>
              <div class="font-semibold text-white text-sm">{notif.title}</div>
              <div class="text-xs text-slate-300 mt-1">{notif.message}</div>
              <div class="text-[10px] text-slate-500 mt-2">{formatTime(notif.timestamp)}</div>
            </div>
            {#if !notif.read}
              <button on:click={() => notificationManager.markAsRead(notif.id)} class="p-1 hover:bg-white/10 rounded transition-colors" title="Mark as read">
                <Check size={14} class="text-blue-400" />
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </div>
{/if}
