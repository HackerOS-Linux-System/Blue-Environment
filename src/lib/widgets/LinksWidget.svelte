<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { ExternalLink } from 'lucide-svelte';
  import { parseLinks } from './widgetModel';
  import type { WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  let links = $derived(parseLinks(String(widget.config.links ?? '')));

  async function open(url: string) {
    try { await invoke('plugin:opener|open_url', { url }); } catch { /* not running inside Tauri */ }
  }
  const onLinks = (e: Event) => patchWidget(widget.id, { config: { links: (e.currentTarget as HTMLTextAreaElement).value } });
</script>

{#if editing}
  <textarea
    value={String(widget.config.links ?? '')}
    onchange={onLinks}
    onmousedown={(e) => e.stopPropagation()}
    spellcheck="false" placeholder={'Label|https://example.org\nOne link per line'}
    class="mt-6 w-[calc(100%-1rem)] h-[calc(100%-2rem)] mx-2 resize-none rounded bg-black/30 px-2 py-1 text-xs text-slate-200 outline-none"
  ></textarea>
{:else if links.length === 0}
  <p class="h-full flex items-center justify-center px-3 text-center text-xs text-slate-400">Add links with “Edit widgets” (Label|https://…).</p>
{:else}
  <div class="h-full overflow-y-auto p-2 flex flex-col gap-1">
    {#each links as l (l.url + l.label)}
      <button class="flex items-center justify-between gap-2 rounded-lg px-2.5 py-1.5 text-left text-sm text-slate-100 bg-white/5 hover:bg-purple-500/30 transition-colors" onclick={() => open(l.url)} title={l.url}>
        <span class="truncate">{l.label}</span><ExternalLink size={12} class="shrink-0 text-slate-400" />
      </button>
    {/each}
  </div>
{/if}
