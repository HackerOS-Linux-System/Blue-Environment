<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Plus, Trash2, Printer as PrinterIcon, RefreshCw, Search, FileText, Power, Star, X, Loader2, AlertTriangle, ListOrdered } from 'lucide-svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import { dialogConfirm } from '../../stores/dialog';
  import { t } from '../../stores/language';

  interface PrinterInfo { name: string; state: 'idle' | 'printing' | 'disabled'; reason: string; uri: string; isDefault: boolean }
  interface PrintersState { cupsAvailable: boolean; printers: PrinterInfo[] }
  interface Discovered { uri: string; kind: string; label: string }
  interface Job { id: number; printer: string; user: string; sizeBytes: number; submitted: string }

  let cupsAvailable = true;
  let printers: PrinterInfo[] = [];
  let jobs: Job[] = [];
  let loading = true;
  let error = '';
  let notice = '';
  let busyName = '';

  // add-printer panel
  let adding = false;
  let discovering = false;
  let discovered: Discovered[] = [];
  let newName = '';
  let newUri = '';
  let driver = 'everywhere';
  let makeDefault = false;
  let saving = false;

  let timer: ReturnType<typeof setInterval> | undefined;

  const validName = (n: string) => /^[A-Za-z0-9_-]{1,127}$/.test(n);
  $: canAdd = validName(newName) && /^[a-z0-9]+:\/\/\S+$/i.test(newUri) && !saving;

  async function load(quiet = false) {
    if (!quiet) loading = true;
    try {
      const s = await SystemBridge.invokeCommand<PrintersState>('printers_list', {});
      cupsAvailable = s.cupsAvailable; printers = s.printers;
      jobs = cupsAvailable ? await SystemBridge.invokeCommand<Job[]>('printers_jobs', {}) : [];
    } catch (e: any) { error = String(e?.message ?? e); }
    loading = false;
  }

  onMount(() => { void load(); timer = setInterval(() => load(true), 5000); });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function act(name: string, fn: () => Promise<unknown>, ok = '') {
    busyName = name; error = ''; notice = '';
    try { await fn(); notice = ok; } catch (e: any) { error = String(e?.message ?? e); }
    busyName = ''; await load(true);
  }

  const setDefault = (n: string) => act(n, () => SystemBridge.invokeCommand('printers_set_default', { name: n }));
  const toggleEnabled = (p: PrinterInfo) => act(p.name, () => SystemBridge.invokeCommand('printers_set_enabled', { name: p.name, enabled: p.state === 'disabled' }));
  const testPage = (n: string) => act(n, () => SystemBridge.invokeCommand('printers_test_page', { name: n }), $t('printers.test_sent'));
  const cancelJob = (id: number) => act('', () => SystemBridge.invokeCommand('printers_cancel_job', { id }));

  async function printFile(n: string) {
    const f = await SystemBridge.pickFile([], $t('printers.print_file'), 'printers.print-file');
    if (f) await act(n, () => SystemBridge.invokeCommand('printers_print_file', { name: n, path: f, copies: 1, duplex: false }), $t('printers.job_sent'));
  }

  async function remove(n: string) {
    const ok = await dialogConfirm({ title: $t('printers.remove_title'), message: $t('printers.remove_body').replace('{name}', n), confirmLabel: $t('common.remove'), cancelLabel: $t('common.cancel'), danger: true });
    if (ok) await act(n, () => SystemBridge.invokeCommand('printers_remove', { name: n }));
  }

  async function discover() {
    discovering = true; error = '';
    try { discovered = await SystemBridge.invokeCommand<Discovered[]>('printers_discover', {}); }
    catch (e: any) { error = String(e?.message ?? e); }
    discovering = false;
  }

  function pick(d: Discovered) {
    newUri = d.uri;
    if (!newName) newName = d.label.split(/[._/:]/)[0].replace(/[^A-Za-z0-9_-]+/g, '_').slice(0, 40) || 'Printer';
  }

  async function add() {
    saving = true; error = '';
    try {
      await SystemBridge.invokeCommand('printers_add', { name: newName, uri: newUri, driver, makeDefault });
      adding = false; newName = ''; newUri = ''; discovered = [];
      notice = $t('printers.added');
    } catch (e: any) { error = String(e?.message ?? e); }
    saving = false; await load(true);
  }

  const stateLabel = (s: PrinterInfo['state']) => $t(`printers.state.${s}`);
</script>

{#if loading}
  <div class="flex items-center justify-center py-12"><RefreshCw size={20} class="animate-spin text-blue-400" /></div>
{:else}
  <div class="space-y-4">
    <div class="flex items-center justify-between gap-3">
      <p class="text-sm text-slate-400">{$t('printers.subtitle')}</p>
      {#if cupsAvailable}
        <button on:click={() => (adding = !adding)} class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm transition-colors shrink-0"><Plus size={13} /> {$t('printers.add')}</button>
      {/if}
    </div>

    {#if error}<div class="flex items-start gap-2 bg-red-500/10 border border-red-500/30 text-red-200 rounded-lg px-3 py-2 text-sm" role="alert"><AlertTriangle size={15} class="mt-0.5 shrink-0" /> <span class="break-words min-w-0">{error}</span></div>{/if}
    {#if notice}<div class="bg-green-500/10 border border-green-500/30 text-green-200 rounded-lg px-3 py-2 text-sm" role="status">{notice}</div>{/if}

    {#if !cupsAvailable}
      <div class="bg-slate-800 rounded-xl p-8 text-center border border-white/5">
        <PrinterIcon size={32} class="mx-auto text-slate-600 mb-3" />
        <p class="text-slate-300">{$t('printers.no_cups')}</p>
        <p class="text-xs text-slate-500 mt-1">{$t('printers.no_cups_hint')}</p>
      </div>
    {:else}
      {#if adding}
        <div class="bg-slate-800 rounded-xl p-4 border border-white/10 space-y-3">
          <div class="flex items-center justify-between"><h3 class="font-medium text-white text-sm">{$t('printers.add')}</h3><button on:click={() => (adding = false)} aria-label={$t('common.close')} class="text-slate-400 hover:text-white"><X size={15} /></button></div>
          <button on:click={discover} disabled={discovering} class="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs disabled:opacity-50">
            {#if discovering}<Loader2 size={13} class="animate-spin" />{:else}<Search size={13} />{/if} {$t('printers.search')}
          </button>
          {#if discovered.length}
            <div class="max-h-40 overflow-y-auto grid gap-1">
              {#each discovered as d (d.uri)}
                <button on:click={() => pick(d)} class="text-left px-3 py-1.5 rounded-lg text-xs {newUri === d.uri ? 'bg-blue-600/30 border border-blue-500/50' : 'bg-slate-900 hover:bg-slate-700 border border-transparent'}">
                  <span class="text-slate-400">{d.kind}</span> · <span class="text-white break-all">{d.label}</span>
                </button>
              {/each}
            </div>
          {/if}
          <div class="grid sm:grid-cols-2 gap-3 text-xs">
            <label class="grid gap-1 text-slate-400">{$t('printers.name')}<input bind:value={newName} placeholder="Office_Printer" class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white outline-none focus:border-blue-500" /></label>
            <label class="grid gap-1 text-slate-400">{$t('printers.driver')}
              <select bind:value={driver} class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white">
                <option value="everywhere">{$t('printers.driver_everywhere')}</option>
                <option value="raw">{$t('printers.driver_raw')}</option>
              </select>
            </label>
          </div>
          <label class="grid gap-1 text-xs text-slate-400">{$t('printers.uri')}<input bind:value={newUri} placeholder="ipp://printer.local/ipp/print" class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white font-mono outline-none focus:border-blue-500" /></label>
          {#if newName && !validName(newName)}<p class="text-xs text-amber-300">{$t('printers.name_rule')}</p>{/if}
          <label class="flex items-center gap-2 text-sm text-slate-300"><input type="checkbox" bind:checked={makeDefault} /> {$t('printers.make_default')}</label>
          <button on:click={add} disabled={!canAdd} class="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-40">{#if saving}<Loader2 size={14} class="animate-spin" />{/if} {$t('printers.add_confirm')}</button>
        </div>
      {/if}

      {#if printers.length === 0}
        <div class="bg-slate-800 rounded-xl p-8 text-center border border-white/5">
          <PrinterIcon size={32} class="mx-auto text-slate-600 mb-3" />
          <p class="text-slate-400">{$t('printers.none')}</p>
          <p class="text-xs text-slate-600 mt-1">{$t('printers.none_hint')}</p>
        </div>
      {:else}
        {#each printers as p (p.name)}
          <div class="bg-slate-800 rounded-xl p-4 border border-white/5">
            <div class="flex items-center gap-4">
              <div class="w-10 h-10 bg-white/5 rounded-xl flex items-center justify-center shrink-0"><PrinterIcon size={18} class={p.state === 'disabled' ? 'text-slate-600' : 'text-slate-300'} /></div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="font-medium text-white">{p.name}</span>
                  {#if p.isDefault}<span class="text-xs bg-green-600/20 text-green-300 px-1.5 py-0.5 rounded border border-green-500/20">{$t('printers.default')}</span>{/if}
                </div>
                <div class="text-xs text-slate-500">{stateLabel(p.state)}{p.reason ? ` — ${p.reason}` : ''}</div>
                {#if p.uri}<div class="text-[11px] text-slate-600 truncate font-mono">{p.uri}</div>{/if}
              </div>
              {#if busyName === p.name}<Loader2 size={15} class="animate-spin text-slate-400" />{/if}
            </div>
            <div class="flex flex-wrap gap-1.5 mt-3 text-xs">
              <button on:click={() => printFile(p.name)} disabled={p.state === 'disabled'} class="flex items-center gap-1 px-2.5 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg disabled:opacity-40"><FileText size={12} /> {$t('printers.print_file')}</button>
              <button on:click={() => testPage(p.name)} disabled={p.state === 'disabled'} class="flex items-center gap-1 px-2.5 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg disabled:opacity-40">{$t('printers.test_page')}</button>
              {#if !p.isDefault}<button on:click={() => setDefault(p.name)} class="flex items-center gap-1 px-2.5 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg"><Star size={12} /> {$t('printers.set_default')}</button>{/if}
              <button on:click={() => toggleEnabled(p)} class="flex items-center gap-1 px-2.5 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg"><Power size={12} /> {p.state === 'disabled' ? $t('printers.enable') : $t('printers.disable')}</button>
              <button on:click={() => remove(p.name)} class="ml-auto p-1.5 hover:text-red-400 text-slate-500" aria-label={$t('common.remove')}><Trash2 size={13} /></button>
            </div>
          </div>
        {/each}
      {/if}

      {#if jobs.length}
        <div class="bg-slate-800 rounded-xl p-4 border border-white/5">
          <h3 class="text-sm font-medium text-white flex items-center gap-2 mb-2"><ListOrdered size={14} /> {$t('printers.queue')}</h3>
          <div class="grid gap-1">
            {#each jobs as j (j.printer + j.id)}
              <div class="flex items-center gap-3 text-xs bg-slate-900 rounded-lg px-3 py-1.5">
                <span class="text-slate-300 w-28 truncate">{j.printer}-{j.id}</span><span class="text-slate-500 flex-1 truncate">{j.user} · {j.submitted}</span>
                <button on:click={() => cancelJob(j.id)} class="text-slate-400 hover:text-red-300" aria-label={$t('printers.cancel_job')}><X size={13} /></button>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
  </div>
{/if}
