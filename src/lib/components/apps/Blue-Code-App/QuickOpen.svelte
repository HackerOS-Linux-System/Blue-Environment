<script lang="ts">
  import { createEventDispatcher, tick } from 'svelte';
  import { FileCode, Search } from 'lucide-svelte';
  import type { FileNode } from './types';

  export let visible = false;
  export let fileTree: FileNode[] = [];

  const dispatch = createEventDispatcher<{ open: string; close: void }>();

  let query = '';
  let selectedIdx = 0;
  let inputEl: HTMLInputElement;

  /** Flattens the tree into files only (no directories — nothing to
   * "open" there) once per render rather than keeping a second store in
   * sync with `fileTree`'s own mutations; the tree is rarely more than a
   * few thousand nodes for a project someone's actively editing, so a
   * full walk on every open is unnoticeable and much simpler than
   * incrementally maintaining a parallel flat index. */
  function flatten(nodes: FileNode[], acc: { path: string; name: string }[] = []): { path: string; name: string }[] {
    for (const n of nodes) {
      if (n.type === 'file') acc.push({ path: n.path, name: n.name });
      else if (n.children) flatten(n.children, acc);
    }
    return acc;
  }

  $: allFiles = flatten(fileTree);

  /** Classic VS Code-style subsequence fuzzy match: every character of
   * `query` must appear in `name`, in order, but not necessarily
   * adjacent ("bcapp" matches "BlueCodeApp.svelte"). Score rewards
   * consecutive-character runs and an early match start, so "app" ranks
   * "BlueCodeApp.svelte" above "AppendOnlyLog.rs" despite both matching. */
  function fuzzyScore(name: string, q: string): number | null {
    if (!q) return 0;
    const lowerName = name.toLowerCase();
    const lowerQ = q.toLowerCase();
    let score = 0;
    let nameIdx = 0;
    let consecutive = 0;
    for (let qIdx = 0; qIdx < lowerQ.length; qIdx++) {
      const ch = lowerQ[qIdx];
      const foundAt = lowerName.indexOf(ch, nameIdx);
      if (foundAt === -1) return null; // query char not found in remaining name — no match
      consecutive = foundAt === nameIdx ? consecutive + 1 : 0;
      score += 10 - Math.min(foundAt - nameIdx, 9) + consecutive * 3;
      nameIdx = foundAt + 1;
    }
    score += Math.max(0, 20 - nameIdx); // reward an overall-earlier match
    return score;
  }

  $: results = query
    ? allFiles
        .map((f) => ({ ...f, score: fuzzyScore(f.name, query) }))
        .filter((f): f is typeof f & { score: number } => f.score !== null)
        .sort((a, b) => b.score - a.score)
        .slice(0, 50)
    : allFiles.slice(0, 50);

  $: if (visible) { selectedIdx = 0; tick().then(() => inputEl?.focus()); }
  $: if (query) selectedIdx = 0;

  function openSelected() {
    const target = results[selectedIdx];
    if (target) { dispatch('open', target.path); close(); }
  }
  function close() { query = ''; dispatch('close'); }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.preventDefault(); close(); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); selectedIdx = Math.min(selectedIdx + 1, results.length - 1); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); selectedIdx = Math.max(selectedIdx - 1, 0); }
    else if (e.key === 'Enter') { e.preventDefault(); openSelected(); }
  }
</script>

{#if visible}
  <div class="fixed inset-0 bg-black/40 z-50 flex items-start justify-center pt-24" on:click={close} role="presentation">
    <div class="w-[480px] max-w-[90vw] bg-slate-800 rounded-xl border border-white/10 shadow-2xl overflow-hidden" on:click|stopPropagation role="presentation">
      <div class="flex items-center gap-2 px-3 py-2 border-b border-white/5">
        <Search size={14} class="text-slate-500 shrink-0" />
        <input bind:this={inputEl} bind:value={query} on:keydown={onKeydown}
          placeholder="Go to file… (type to fuzzy search)"
          class="flex-1 bg-transparent text-sm text-white focus:outline-none placeholder-slate-600" />
      </div>
      <div class="max-h-80 overflow-y-auto py-1">
        {#each results as f, i (f.path)}
          <button on:click={() => { selectedIdx = i; openSelected(); }}
            class="w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm {i === selectedIdx ? 'bg-blue-600/30 text-white' : 'text-slate-300 hover:bg-white/5'}">
            <FileCode size={13} class="text-yellow-400 shrink-0" />
            <span class="truncate">{f.name}</span>
            <span class="text-[10px] text-slate-500 truncate ml-auto">{f.path}</span>
          </button>
        {:else}
          <div class="px-3 py-6 text-center text-xs text-slate-600">No matching files.</div>
        {/each}
      </div>
    </div>
  </div>
{/if}
