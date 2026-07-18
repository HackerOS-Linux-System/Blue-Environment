<script lang="ts">
  import { Hash } from 'lucide-svelte';
  import type { DocFormat } from './types';

  export let content: string;
  export let format: DocFormat;

  $: headings = (() => {
    const list: { level: number; text: string }[] = [];
    if (format === 'rich') {
      const div = document.createElement('div');
      div.innerHTML = content;
      div.querySelectorAll('h1,h2,h3').forEach((h) => list.push({ level: parseInt(h.tagName[1]), text: h.textContent ?? '' }));
    } else if (format === 'markdown') {
      for (const line of content.split('\n')) {
        const m = line.match(/^(#{1,3})\s+(.+)/);
        if (m) list.push({ level: m[1].length, text: m[2] });
      }
    }
    return list;
  })();

  $: words = content.replace(/<[^>]+>/g, '').trim().split(/\s+/).filter(Boolean).length;
  $: chars = content.replace(/<[^>]+>/g, '').length;
  $: lines = content.split('\n').length;
</script>

<div class="w-52 border-l border-white/5 flex flex-col bg-slate-900/50 shrink-0">
  <div class="px-3 py-3 border-b border-white/5">
    <p class="text-xs text-slate-500 font-medium uppercase tracking-wider mb-2">Outline</p>
    {#if headings.length === 0}
      <p class="text-xs text-slate-700 italic">No headings</p>
    {:else}
      <div class="space-y-1">
        {#each headings as h, i (i)}
          <div class="flex items-center gap-1.5 cursor-pointer hover:text-blue-400 text-slate-400 transition-colors" style="padding-left:{(h.level - 1) * 10}px;">
            <Hash size={10} class="shrink-0 text-slate-600" />
            <span class="text-xs truncate">{h.text}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="px-3 py-3 mt-auto border-t border-white/5">
    <p class="text-xs text-slate-500 font-medium uppercase tracking-wider mb-2">Statistics</p>
    <div class="space-y-1 text-xs text-slate-600">
      <div class="flex justify-between"><span>Words</span><span class="text-slate-400">{words.toLocaleString()}</span></div>
      <div class="flex justify-between"><span>Chars</span><span class="text-slate-400">{chars.toLocaleString()}</span></div>
      <div class="flex justify-between"><span>Lines</span><span class="text-slate-400">{lines}</span></div>
    </div>
  </div>
</div>
