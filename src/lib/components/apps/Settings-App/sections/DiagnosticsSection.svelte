<script lang="ts">
  import { Stethoscope, Copy, Download, Trash2, Send, ShieldCheck } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';

  interface Info { shellVersion: string; tauriVersion: string; webkitVersion: string | null; os: string | null; kernel: string | null; arch: string; backend: string; compositorVersion: string | null; pipewireVersion: string | null; bwrapVersion: string | null; portalProblems: string[] }
  interface Crash { id: string; time: string; message: string; location: string; sent: boolean }
  interface CrashSettings { enabled: boolean; endpoint: string; autoSend: boolean }
  interface Iso { bwrapAvailable: boolean; dbusProxyAvailable: boolean; seccompAvailable: boolean }

  let info: Info | null = null;
  let iso: Iso | null = null;
  let report = '';
  let crashes: Crash[] = [];
  let settings: CrashSettings = { enabled: false, endpoint: '', autoSend: false };
  let message = '';
  let showReport = false;

  async function load() {
    try {
      info = await SystemBridge.invoke<Info>('diagnostics_info');
      iso = await SystemBridge.cloneIsolationStatus();
      settings = await SystemBridge.invoke<CrashSettings>('crash_reporting_get');
      crashes = await SystemBridge.invoke<Crash[]>('crash_reports_list');
    } catch (e) { message = String(e); }
  }
  load();

  async function loadReport() {
    report = await SystemBridge.invoke<string>('diagnostics_report', { logLines: 300 });
    showReport = true;
  }
  async function copy() {
    if (!report) await loadReport();
    try { await navigator.clipboard.writeText(report); message = $t('diag.copied'); } catch (e) { message = String(e); }
  }
  async function save() {
    try { message = $t('diag.saved', { path: await SystemBridge.invoke<string>('diagnostics_save_report') }); } catch (e) { message = String(e); }
  }
  async function saveSettings(patch: Partial<CrashSettings>) {
    try { settings = await SystemBridge.invoke<CrashSettings>('crash_reporting_set', { settings: { ...settings, ...patch } }); message = ''; } catch (e) { message = String(e); }
  }
  async function del(id: string) { await SystemBridge.invoke('crash_report_delete', { id }); await load(); }
  async function send(id: string) {
    try { await SystemBridge.invoke('crash_report_send', { id }); message = $t('diag.sent'); } catch (e) { message = String(e); }
    await load();
  }
  const row = (label: string, v: string | null | undefined) => ({ label, v: v ?? $t('diag.not_found') });
  $: rows = info ? [
    row($t('diag.shell'), info.shellVersion), row('Tauri', info.tauriVersion), row('WebKitGTK', info.webkitVersion), row($t('diag.os'), info.os), row($t('diag.kernel'), info.kernel),
    row($t('diag.backend'), `${info.backend}${info.compositorVersion ? ' — ' + info.compositorVersion : ''}`), row('PipeWire', info.pipewireVersion), row('bubblewrap', info.bwrapVersion),
  ] : [];
</script>

<div class="space-y-5 max-w-2xl">
  <p class="text-sm text-slate-400">{$t('diag.subtitle')}</p>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5">
    <h3 class="flex items-center gap-2 text-white font-medium mb-3"><Stethoscope size={16} class="text-violet-300" /> {$t('diag.versions')}</h3>
    <dl class="grid grid-cols-[9rem_1fr] gap-x-3 gap-y-1 text-sm">
      {#each rows as r}<dt class="text-slate-500">{r.label}</dt><dd class="text-slate-200 break-words">{r.v}</dd>{/each}
    </dl>
  </section>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-2">
    <h3 class="text-white font-medium">{$t('diag.health')}</h3>
    {#if info && info.portalProblems.length === 0}<p class="text-xs text-emerald-300">{$t('diag.portals_ok')}</p>{/if}
    {#if info}{#each info.portalProblems as p}<p class="text-xs text-amber-300">• {p}</p>{/each}{/if}
    {#if iso && !iso.bwrapAvailable}<p class="text-xs text-amber-300">• {$t('settings.cloned_apps.bwrap_missing_warning')}</p>{/if}
    {#if iso && iso.bwrapAvailable && !iso.dbusProxyAvailable}<p class="text-xs text-slate-400">• {$t('settings.cloned_apps.dbus_proxy_missing')}</p>{/if}
  </section>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="text-white font-medium">{$t('diag.report')}</h3>
    <p class="text-xs text-slate-500">{$t('diag.report_hint')}</p>
    <div class="flex flex-wrap gap-2">
      <button on:click={loadReport} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs">{$t('diag.show')}</button>
      <button on:click={copy} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs flex items-center gap-1.5"><Copy size={12} /> {$t('diag.copy')}</button>
      <button on:click={save} class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-xs flex items-center gap-1.5"><Download size={12} /> {$t('diag.save')}</button>
    </div>
    {#if showReport}<pre class="max-h-72 overflow-auto rounded-lg bg-slate-900 p-3 text-[11px] text-slate-300 whitespace-pre-wrap">{report}</pre>{/if}
  </section>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="flex items-center gap-2 text-white font-medium"><ShieldCheck size={16} class="text-violet-300" /> {$t('diag.crash')}</h3>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('diag.crash_enable')}<span class="block text-xs text-slate-500">{$t('diag.crash_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={settings.enabled} on:change={(e) => saveSettings({ enabled: e.currentTarget.checked })} />
    </label>
    {#if settings.enabled}
      <label class="block text-xs text-slate-400">{$t('diag.endpoint')}
        <input value={settings.endpoint} placeholder="https://" on:change={(e) => saveSettings({ endpoint: e.currentTarget.value })} class="mt-1 w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white" />
      </label>
      <label class="flex items-center justify-between gap-4 {settings.endpoint ? '' : 'opacity-50'}">
        <span class="text-sm text-slate-200">{$t('diag.auto_send')}</span>
        <input type="checkbox" class="w-5 h-5" disabled={!settings.endpoint} checked={settings.autoSend} on:change={(e) => saveSettings({ autoSend: e.currentTarget.checked })} />
      </label>
    {/if}
    {#if crashes.length}
      <ul class="divide-y divide-white/5">
        {#each crashes as c (c.id)}
          <li class="py-2 flex items-start gap-3">
            <div class="min-w-0 flex-1"><div class="text-sm text-slate-200 truncate">{c.message}</div><div class="text-[11px] text-slate-500">{new Date(c.time).toLocaleString()} · {c.location}{c.sent ? ' · ' + $t('diag.was_sent') : ''}</div></div>
            {#if settings.endpoint && !c.sent}<button on:click={() => send(c.id)} class="p-1.5 rounded hover:bg-white/10" title={$t('diag.send')} aria-label={$t('diag.send')}><Send size={14} /></button>{/if}
            <button on:click={() => del(c.id)} class="p-1.5 rounded hover:bg-white/10" title={$t('diag.delete')} aria-label={$t('diag.delete')}><Trash2 size={14} /></button>
          </li>
        {/each}
      </ul>
    {:else}<p class="text-xs text-slate-500">{$t('diag.no_crashes')}</p>{/if}
  </section>

  {#if message}<p class="text-xs text-slate-300" role="status">{message}</p>{/if}
</div>
