<script lang="ts">
  import { Search, Home, ZoomIn, ShieldCheck, Plus, X, Trash2 } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';
  import { SEARCH_ENGINES, ZOOM_LEVELS } from './types';
  import type { BlueWebSettings } from './types';
  import { BUILTIN_BLOCKLIST } from './webSettings';

  export let settings: BlueWebSettings;
  export let onUpdate: (patch: Partial<BlueWebSettings>) => void;
  export let onAddBlocked: (domain: string) => void;
  export let onRemoveBlocked: (domain: string) => void;

  const dispatch = createEventDispatcher<{ close: void }>();

  let newDomain = '';
  function addDomain() {
    if (!newDomain.trim()) return;
    onAddBlocked(newDomain);
    newDomain = '';
  }
</script>

<div class="h-full overflow-y-auto bg-slate-900 text-white px-8 py-6">
  <div class="max-w-xl mx-auto flex flex-col gap-8">
    <div class="flex items-center justify-between">
      <h1 class="text-xl font-bold">Blue Web Settings</h1>
      <button on:click={() => dispatch('close')} class="p-1.5 rounded-lg hover:bg-white/10"><X size={16} /></button>
    </div>

    <section class="flex flex-col gap-3">
      <div class="flex items-center gap-2 text-sm font-medium text-slate-300"><Search size={14} /> Search engine</div>
      <div class="grid grid-cols-2 gap-2">
        {#each SEARCH_ENGINES as engine}
          <button on:click={() => onUpdate({ searchEngine: engine.id })}
            class="px-3 py-2 rounded-lg text-sm text-left border transition-colors {settings.searchEngine === engine.id ? 'bg-blue-600/15 border-blue-500/40' : 'bg-slate-800 border-white/5 hover:bg-white/5'}">
            {engine.name}
          </button>
        {/each}
      </div>
    </section>

    <section class="flex flex-col gap-3">
      <div class="flex items-center gap-2 text-sm font-medium text-slate-300"><Home size={14} /> Homepage</div>
      <input value={settings.homepage} on:change={(e) => onUpdate({ homepage: e.currentTarget.value })}
        placeholder="https://…" class="bg-slate-800 rounded-lg px-3 py-2 text-sm focus:outline-none font-mono" />
    </section>

    <section class="flex flex-col gap-3">
      <div class="flex items-center gap-2 text-sm font-medium text-slate-300"><ZoomIn size={14} /> Default zoom for new tabs</div>
      <select value={settings.defaultZoom} on:change={(e) => onUpdate({ defaultZoom: Number(e.currentTarget.value) })}
        class="bg-slate-800 rounded-lg px-3 py-2 text-sm focus:outline-none w-40">
        {#each ZOOM_LEVELS as z}<option value={z}>{Math.round(z * 100)}%</option>{/each}
      </select>
    </section>

    <section class="flex flex-col gap-3">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2 text-sm font-medium text-slate-300"><ShieldCheck size={14} /> Content blocking</div>
        <button on:click={() => onUpdate({ contentBlockingEnabled: !settings.contentBlockingEnabled })}
          class="relative w-10 h-5.5 rounded-full transition-colors {settings.contentBlockingEnabled ? 'bg-blue-600' : 'bg-slate-700'}">
          <span class="absolute top-0.5 w-4.5 h-4.5 rounded-full bg-white transition-transform {settings.contentBlockingEnabled ? 'translate-x-[19px]' : 'translate-x-0.5'}" />
        </button>
      </div>
      <p class="text-xs text-slate-500 -mt-1">
        Blocks navigation to {BUILTIN_BLOCKLIST.length} known ad/tracker domains, plus any you add below.
        This blocks whole-page navigation to a blocked domain — it can't yet block individual ad/tracker
        resources embedded inside an otherwise allowed page.
      </p>
      {#if settings.contentBlockingEnabled}
        <div class="flex gap-2">
          <input bind:value={newDomain} on:keydown={(e) => e.key === 'Enter' && addDomain()}
            placeholder="example-ads.com" class="flex-1 bg-slate-800 rounded-lg px-3 py-1.5 text-xs focus:outline-none font-mono" />
          <button on:click={addDomain} class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-xs flex items-center gap-1"><Plus size={12} /> Add</button>
        </div>
        {#if settings.customBlockedDomains.length > 0}
          <div class="flex flex-col gap-1 mt-1">
            {#each settings.customBlockedDomains as domain}
              <div class="flex items-center justify-between px-3 py-1.5 rounded-lg bg-slate-800/60 text-xs font-mono">
                <span>{domain}</span>
                <button on:click={() => onRemoveBlocked(domain)} class="text-slate-500 hover:text-red-400"><Trash2 size={12} /></button>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    </section>

    <section class="flex items-center justify-between">
      <div>
        <div class="text-sm font-medium text-slate-300">Open links from other apps in a new tab</div>
        <p class="text-xs text-slate-500">When another app (like Blue Mail) opens a link, it lands in a new Blue Web tab instead of replacing what you're currently viewing.</p>
      </div>
      <button on:click={() => onUpdate({ openLinksFromOtherAppsInNewTab: !settings.openLinksFromOtherAppsInNewTab })}
        class="relative w-10 h-5.5 rounded-full transition-colors shrink-0 ml-4 {settings.openLinksFromOtherAppsInNewTab ? 'bg-blue-600' : 'bg-slate-700'}">
        <span class="absolute top-0.5 w-4.5 h-4.5 rounded-full bg-white transition-transform {settings.openLinksFromOtherAppsInNewTab ? 'translate-x-[19px]' : 'translate-x-0.5'}" />
      </button>
    </section>
  </div>
</div>
