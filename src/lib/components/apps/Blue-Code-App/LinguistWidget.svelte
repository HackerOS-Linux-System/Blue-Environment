<script lang="ts">
  import { RefreshCw, Minus, Plus, Settings2 } from 'lucide-svelte';
  import { groupShares, formatPercent, needsOutline, leadBrand, tooltip, formatBytes, type Linguist } from './linguist';
  import { brandIcon } from './linguistBrands';

  /**
   * GitHub-style "Languages" card, bottom-right corner of the editor.
   * Built-in custom languages (H#, HackerScript, Hacker Lang, hk) look exactly
   * like they do on bit.io; languages from .blue/languages.json get a "custom" tag.
   */
  interface Props {
    linguist: Linguist;
    /** Lift the card above the terminal panel when it is open. */
    lifted?: boolean;
    oncustomize?: () => void;
  }
  let { linguist, lifted = false, oncustomize }: Props = $props();

  const { report, status, error } = linguist;

  const KEY = 'blue-code-linguist-collapsed';
  function readCollapsed(): boolean { try { return localStorage.getItem(KEY) === '1'; } catch { return false; } }
  let collapsed = $state(readCollapsed());
  function toggle() { collapsed = !collapsed; try { localStorage.setItem(KEY, collapsed ? '1' : '0'); } catch { /* ignore */ } }

  let grouped = $derived($report ? groupShares($report.languages) : { visible: [], other: null });
  let lead = $derived($report ? leadBrand($report.languages) : null);
  let top = $derived($report?.languages[0]);
</script>

{#if $status !== 'unavailable' && ($report || $status === 'error')}
  <aside
    aria-label="Languages used in this project"
    class="absolute right-3 z-20 w-64 rounded-xl border border-white/10 bg-slate-800/95 shadow-xl backdrop-blur text-[11px] text-slate-300 {lifted ? 'bottom-[13.25rem]' : 'bottom-9'}"
  >
    <div class="flex items-center gap-1.5 px-3 py-2">
      {#if lead}
        <img src={brandIcon(lead.brand)} alt="" class="h-4 w-4 rounded-sm" />
        <h4 class="font-semibold text-slate-100 truncate" title="{lead.name} leads this project">{lead.name} leads this project</h4>
      {:else}
        <h4 class="font-semibold text-slate-100">Languages</h4>
      {/if}
      <div class="flex-1"></div>
      <button class="p-0.5 rounded hover:bg-white/10 hover:text-white" title="Rescan languages" aria-label="Rescan languages" onclick={() => linguist.scan()}>
        <RefreshCw size={12} class={$status === 'scanning' ? 'animate-spin' : ''} />
      </button>
      <button class="p-0.5 rounded hover:bg-white/10 hover:text-white" title="Custom languages (.blue/languages.json)" aria-label="Customize languages" onclick={() => oncustomize?.()}>
        <Settings2 size={12} />
      </button>
      <button class="p-0.5 rounded hover:bg-white/10 hover:text-white" aria-expanded={!collapsed} aria-label={collapsed ? 'Expand language bar' : 'Collapse language bar'} onclick={toggle}>
        {#if collapsed}<Plus size={12} />{:else}<Minus size={12} />{/if}
      </button>
    </div>

    {#if $status === 'error'}
      <p class="px-3 pb-3 text-red-300">Couldn't scan this folder: {$error}</p>
    {:else if $report}
      <!-- the stacked bar stays visible when collapsed -->
      <div class="mx-3 mb-2 flex h-2 overflow-hidden rounded-full bg-slate-700" role="img" aria-label="Language breakdown">
        {#each grouped.visible as s (s.name)}
          <div style="width:{s.percent}%;background:{s.color};min-width:2px" title={tooltip(s)}></div>
        {/each}
        {#if grouped.other}<div style="width:{grouped.other.percent}%;background:#ededed;min-width:2px" title="Other — {formatPercent(grouped.other.percent)}"></div>{/if}
      </div>

      {#if collapsed}
        {#if top}<p class="px-3 pb-2 text-slate-400"><span class="text-slate-200">{top.name}</span> {formatPercent(top.percent)}</p>{/if}
      {:else}
        {#if grouped.visible.length === 0}
          <p class="px-3 pb-3 text-slate-500">No known source languages found.</p>
        {:else}
          <ul class="max-h-56 overflow-y-auto px-3 pb-2 space-y-1">
            {#each grouped.visible as s (s.name)}
              <li class="flex items-center gap-2" title={tooltip(s)}>
                <span class="h-2.5 w-2.5 shrink-0 rounded-full {needsOutline(s.color) ? 'ring-1 ring-slate-400' : ''}" style="background:{s.color}"></span>
                <span class="truncate text-slate-100">{s.name}</span>
                {#if s.custom}<span class="rounded bg-blue-500/20 px-1 text-[9px] uppercase tracking-wide text-blue-300">custom</span>{/if}
                <span class="ml-auto tabular-nums text-slate-400">{formatPercent(s.percent)}</span>
              </li>
            {/each}
            {#if grouped.other}
              <li class="flex items-center gap-2" title="{grouped.other.count} more · {formatBytes(grouped.other.bytes)}">
                <span class="h-2.5 w-2.5 shrink-0 rounded-full" style="background:#ededed"></span>
                <span class="text-slate-100">Other</span>
                <span class="ml-auto tabular-nums text-slate-400">{formatPercent(grouped.other.percent)}</span>
              </li>
            {/if}
          </ul>
        {/if}
        <p class="border-t border-white/5 px-3 py-1.5 text-[10px] text-slate-500">
          {$report.countedFiles} files{$report.truncated ? ' (partial scan)' : ''} · {$report.source === 'git' ? 'git' : 'folder'} · {$report.elapsedMs} ms
        </p>
      {/if}
    {/if}
  </aside>
{/if}
