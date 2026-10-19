<script lang="ts">
  import { onMount } from 'svelte';
  import { RefreshCw, Link2, Unlink, BellRing, Send, FileUp, MessageSquare, Smartphone, Loader2, AlertTriangle } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';

  interface KdeDevice { id: string; name: string; paired: boolean; reachable: boolean }
  interface KdeStatus { installed: boolean; devices: KdeDevice[] }

  let status: KdeStatus | null = null;
  let loading = true;
  let error = '';
  let notice = '';
  let text = '';
  let smsTo = '';
  let smsBody = '';
  let target = '';

  const call = <T,>(cmd: string, args: Record<string, unknown> = {}) => SystemBridge.invokeCommand<T>(cmd, args);

  async function refresh() {
    loading = true;
    status = await call<KdeStatus>('kde_status').catch(() => ({ installed: false, devices: [] }));
    if (status && !status.devices.some((d) => d.id === target)) target = status.devices.find((d) => d.paired && d.reachable)?.id ?? '';
    loading = false;
  }
  onMount(refresh);

  async function run(fn: () => Promise<unknown>, ok = '') {
    error = ''; notice = '';
    try { await fn(); notice = ok; } catch (e: any) { error = String(e?.message ?? e); }
    await refresh();
  }
  $: current = status?.devices.find((d) => d.id === target) ?? null;
  $: usable = !!current && current.paired && current.reachable;
</script>

<div class="flex-1 overflow-y-auto p-4 space-y-4">
  {#if loading && !status}
    <div class="flex justify-center py-10"><Loader2 class="w-5 h-5 animate-spin text-blue-400" /></div>
  {:else if status && !status.installed}
    <div class="p-4 rounded-xl bg-slate-900 border border-white/10 space-y-2">
      <div class="font-medium">{$t('kde.not_installed')}</div>
      <p class="text-xs text-slate-400">{$t('kde.not_installed_hint')}</p>
      <code class="block bg-black/40 rounded px-3 py-2 text-xs">sudo apt install kdeconnect</code>
    </div>
  {:else if status}
    <div class="flex items-center justify-between">
      <p class="text-xs text-slate-400">{$t('kde.subtitle')}</p>
      <button on:click={refresh} class="flex items-center gap-1 text-xs px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700"><RefreshCw class="w-3.5 h-3.5 {loading ? 'animate-spin' : ''}" /> {$t('phone.refresh')}</button>
    </div>
    {#if error}<div class="flex items-start gap-2 bg-red-500/10 border border-red-500/30 text-red-200 rounded-lg px-3 py-2 text-xs" role="alert"><AlertTriangle class="w-4 h-4 mt-0.5 shrink-0" /><span class="break-words">{error}</span></div>{/if}
    {#if notice}<div class="bg-green-500/10 border border-green-500/30 text-green-200 rounded-lg px-3 py-2 text-xs" role="status">{notice}</div>{/if}

    {#if status.devices.length === 0}
      <div class="p-6 text-center text-slate-500 text-sm border border-dashed border-white/10 rounded-xl"><Smartphone class="w-8 h-8 mx-auto mb-2 text-slate-600" />{$t('kde.none')}</div>
    {:else}
      <div class="grid gap-2">
        {#each status.devices as d (d.id)}
          <div class="flex items-center gap-3 p-3 rounded-xl border {target === d.id ? 'border-blue-500 bg-blue-500/10' : 'border-transparent bg-slate-900'}">
            <button class="flex items-center gap-3 flex-1 min-w-0 text-left" on:click={() => (target = d.id)}>
              <Smartphone class="w-5 h-5 text-slate-300 shrink-0" />
              <div class="min-w-0"><div class="text-sm truncate">{d.name}</div><div class="text-[11px] text-slate-500">{d.reachable ? $t('kde.reachable') : $t('kde.offline')} · {d.paired ? $t('blueconnect.paired') : $t('kde.not_paired')}</div></div>
            </button>
            {#if d.paired}
              <button on:click={() => run(() => call('kde_unpair', { id: d.id }))} class="flex items-center gap-1 text-xs px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700"><Unlink class="w-3.5 h-3.5" /> {$t('kde.unpair')}</button>
            {:else if d.reachable}
              <button on:click={() => run(() => call('kde_pair', { id: d.id }), $t('kde.pair_sent'))} class="flex items-center gap-1 text-xs px-2.5 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500"><Link2 class="w-3.5 h-3.5" /> {$t('blueconnect.pair')}</button>
            {/if}
          </div>
        {/each}
      </div>

      <div class="grid gap-3 {usable ? '' : 'opacity-40 pointer-events-none'}">
        <div class="flex flex-wrap gap-2 text-xs">
          <button on:click={() => run(() => call('kde_ring', { id: target }), $t('kde.ringing'))} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700"><BellRing class="w-3.5 h-3.5" /> {$t('kde.find_phone')}</button>
          <button on:click={() => run(() => call('kde_ping', { id: target }), $t('kde.pinged'))} class="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700">{$t('kde.ping')}</button>
          <button on:click={async () => { const f = await SystemBridge.pickFile([], $t('kde.send_file'), 'blue-kde.share'); if (f) await run(() => call('kde_share_file', { id: target, path: f }), $t('kde.sent')); }} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700"><FileUp class="w-3.5 h-3.5" /> {$t('kde.send_file')}</button>
        </div>
        <div class="flex gap-2"><input bind:value={text} placeholder={$t('kde.text_or_link')} class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs" /><button on:click={() => run(() => call('kde_share_text', { id: target, text }).then(() => (text = '')), $t('kde.sent'))} disabled={!text.trim()} class="flex items-center gap-1 px-3 py-1.5 rounded-lg text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-40"><Send class="w-3.5 h-3.5" /> {$t('kde.send')}</button></div>
        <div class="grid gap-2">
          <div class="flex gap-2"><input bind:value={smsTo} placeholder="+48 600 700 800" class="w-44 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs" /><input bind:value={smsBody} placeholder={$t('phone.sms_text')} class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs" /><button on:click={() => run(() => call('kde_send_sms', { id: target, destination: smsTo, message: smsBody }).then(() => (smsBody = '')), $t('kde.sms_sent'))} disabled={!smsTo || !smsBody.trim()} class="flex items-center gap-1 px-3 py-1.5 rounded-lg text-xs bg-slate-700 hover:bg-slate-600 disabled:opacity-40"><MessageSquare class="w-3.5 h-3.5" /> {$t('kde.send_sms')}</button></div>
        </div>
      </div>
    {/if}
  {/if}
</div>
