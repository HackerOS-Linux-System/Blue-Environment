<script lang="ts">
  import { onMount } from 'svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import { dialogConfirm } from '../../stores/dialog';
  import { Plus, Trash2, Printer, RefreshCw } from 'lucide-svelte';

  interface PrinterInfo { name: string; status: string; isDefault: boolean; location?: string; }

  let printers: PrinterInfo[] = [];
  let loading = true;

  async function load() {
    loading = true;
    try {
      const result = await SystemBridge.executeCommand('lpstat -p 2>/dev/null || echo "No CUPS"');
      const out = typeof result === 'string' ? result : (result as any)?.stdout || '';
      if (out.includes('No CUPS') || !out.trim()) { printers = []; loading = false; return; }
      const defResult = await SystemBridge.executeCommand('lpstat -d 2>/dev/null | cut -d: -f2');
      const defOut = typeof defResult === 'string' ? defResult : (defResult as any)?.stdout || '';
      const defaultPrinter = defOut.trim();
      const ps: PrinterInfo[] = [];
      for (const line of out.split('\n')) {
        const m = line.match(/^printer\s+(\S+)\s+is\s+(.+)/);
        if (m) ps.push({ name: m[1], status: m[2].trim(), isDefault: m[1] === defaultPrinter });
      }
      printers = ps;
    } catch { printers = []; }
    loading = false;
  }

  onMount(load);

  function shq(s: string): string { return `'${s.replace(/'/g, "'\\''")}'`; }

  async function setDefault(name: string) {
    // `name` is enumerated from `lpstat -p`, so realistic risk is low,
    // but single-quoting instead of the previous unescaped double
    // quotes costs nothing and matches the pattern used elsewhere.
    await SystemBridge.executeCommand(`lpoptions -d ${shq(name)}`).catch(() => {});
    load();
  }

  async function removePrinter(name: string) {
    const ok = await dialogConfirm({ title: 'Remove printer', message: `Remove printer "${name}"? You can add it again later.`, confirmLabel: 'Remove', danger: true });
    if (!ok) return;
    await SystemBridge.executeCommand(`lpadmin -x ${shq(name)}`).catch(() => {});
    load();
  }
</script>

{#if loading}
  <div class="flex items-center justify-center py-12"><RefreshCw size={20} class="animate-spin text-blue-400" /></div>
{:else}
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <p class="text-sm text-slate-400">Manage printers via CUPS</p>
      <button on:click={() => SystemBridge.launchApp('system-config-printer &').catch(() => {})}
        class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm transition-colors">
        <Plus size={13} /> Add Printer
      </button>
    </div>
    {#if printers.length === 0}
      <div class="bg-slate-800 rounded-xl p-8 text-center border border-white/5">
        <Printer size={32} class="mx-auto text-slate-600 mb-3" />
        <p class="text-slate-400">No printers configured</p>
        <p class="text-xs text-slate-600 mt-1">Install CUPS and add a printer</p>
      </div>
    {:else}
      {#each printers as p (p.name)}
        <div class="bg-slate-800 rounded-xl p-4 border border-white/5 flex items-center gap-4">
          <div class="w-10 h-10 bg-white/5 rounded-xl flex items-center justify-center shrink-0">
            <Printer size={18} class="text-slate-400" />
          </div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="font-medium text-white">{p.name}</span>
              {#if p.isDefault}<span class="text-xs bg-green-600/20 text-green-300 px-1.5 py-0.5 rounded border border-green-500/20">Default</span>{/if}
            </div>
            <div class="text-xs text-slate-500 capitalize">{p.status}</div>
          </div>
          <div class="flex gap-1">
            {#if !p.isDefault}
              <button on:click={() => setDefault(p.name)} class="px-2 py-1 text-xs bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors">Set Default</button>
            {/if}
            <button on:click={() => removePrinter(p.name)} class="p-1.5 hover:text-red-400 text-slate-500 transition-colors"><Trash2 size={13} /></button>
          </div>
        </div>
      {/each}
    {/if}
  </div>
{/if}
