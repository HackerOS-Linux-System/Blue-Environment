<script lang="ts">
  import { BatteryMedium, BatteryCharging, BellRing, ClipboardCopy, ClipboardPaste, Share2, Bell, Loader2 } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { batteryLabel, type DeviceNotification } from './deviceTools';

  interface Props { deviceId: string; deviceName: string }
  let { deviceId, deviceName }: Props = $props();

  let busy = $state<string | null>(null);
  let notice = $state('');
  let error = $state('');
  let battery = $state<{ percent: number; charging: boolean } | null | undefined>(undefined);
  let shareText = $state('');
  let notifications = $state<DeviceNotification[] | null>(null);

  const call = <T,>(cmd: string, args: Record<string, unknown> = {}) => SystemBridge.invokeCommand<T>(cmd, { deviceId, ...args });

  async function run(name: string, fn: () => Promise<void>) {
    if (busy) return;
    busy = name; error = ''; notice = '';
    try { await fn(); } catch (e) { error = String((e as Error)?.message ?? e); } finally { busy = null; }
  }

  const readBattery = () => run('battery', async () => { battery = await call<{ percent: number; charging: boolean } | null>('bc_device_battery'); });
  const find = () => run('find', async () => { await call('bc_find_device'); notice = $t('devtools.find_sent').replace('{name}', deviceName); });
  const sendClipboard = () => run('send', async () => {
    const text = await SystemBridge.readText();
    if (!text) { notice = $t('devtools.clipboard_empty'); return; }
    await call('bc_send_clipboard', { text });
    notice = $t('devtools.clipboard_sent').replace('{name}', deviceName);
  });
  const getClipboard = () => run('get', async () => {
    const text = await call<string>('bc_get_device_clipboard');
    if (!text) { notice = $t('devtools.clipboard_empty'); return; }
    await SystemBridge.copyText(text);
    notice = $t('devtools.clipboard_got').replace('{name}', deviceName);
  });
  const share = () => run('share', async () => {
    if (!shareText.trim()) return;
    await call('bc_share', { textOrUrl: shareText.trim() });
    shareText = '';
    notice = $t('devtools.shared').replace('{name}', deviceName);
  });
  const loadNotifications = () => run('notifications', async () => { notifications = await call<DeviceNotification[]>('bc_device_notifications'); });

  const btn = 'flex items-center gap-1 px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-[11px] text-slate-200 disabled:opacity-50';
</script>

<div class="mt-2 pt-2 border-t border-white/5 space-y-2">
  <div class="flex flex-wrap gap-1.5">
    <button class={btn} disabled={!!busy} onclick={readBattery}>
      {#if busy === 'battery'}<Loader2 class="w-3 h-3 animate-spin" />{:else if battery?.charging}<BatteryCharging class="w-3 h-3" />{:else}<BatteryMedium class="w-3 h-3" />{/if}
      {battery === undefined ? $t('devtools.battery') : battery === null ? $t('devtools.battery_none') : batteryLabel(battery, $t('devtools.charging'))}
    </button>
    <button class={btn} disabled={!!busy} onclick={find}><BellRing class="w-3 h-3" /> {$t('devtools.find')}</button>
    <button class={btn} disabled={!!busy} onclick={sendClipboard}><ClipboardCopy class="w-3 h-3" /> {$t('devtools.send_clipboard')}</button>
    <button class={btn} disabled={!!busy} onclick={getClipboard}><ClipboardPaste class="w-3 h-3" /> {$t('devtools.get_clipboard')}</button>
    <button class={btn} disabled={!!busy} onclick={loadNotifications}><Bell class="w-3 h-3" /> {$t('devtools.notifications')}</button>
  </div>

  <form class="flex gap-1.5" onsubmit={(e) => { e.preventDefault(); share(); }}>
    <input bind:value={shareText} placeholder={$t('devtools.share_placeholder')} class="flex-1 min-w-0 bg-slate-800 border border-white/10 rounded-md px-2 py-1 text-[11px] text-white placeholder:text-slate-500" />
    <button class={btn} disabled={!!busy || !shareText.trim()}><Share2 class="w-3 h-3" /> {$t('devtools.share')}</button>
  </form>

  {#if notice}<p class="text-[11px] text-emerald-400">{notice}</p>{/if}
  {#if error}<p class="text-[11px] text-red-400">{error}</p>{/if}

  {#if notifications}
    <ul class="space-y-1 max-h-40 overflow-y-auto">
      {#each notifications as n (n.id)}
        <li class="rounded-md bg-slate-800/70 px-2 py-1">
          <div class="text-[10px] text-slate-500 truncate">{n.appName}</div>
          <div class="text-[11px] text-slate-200 truncate">{n.title}</div>
          {#if n.text}<div class="text-[11px] text-slate-400 line-clamp-2">{n.text}</div>{/if}
        </li>
      {:else}
        <li class="text-[11px] text-slate-500">{$t('devtools.no_notifications')}</li>
      {/each}
    </ul>
  {/if}
</div>
