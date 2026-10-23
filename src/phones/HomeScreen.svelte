<script lang="ts">
  import { run } from 'svelte/legacy';

  import { createEventDispatcher } from 'svelte';
  import { Search, X } from 'lucide-svelte';
  import { t } from '../lib/stores/language';
  import { filterApps, paginate, type LauncherApp } from '../lib/altShell/apps';
  import { pageAfterDrag } from '../lib/altShell/gestures';
  import { formatClock, formatDate } from '../lib/altShell/shellUtils';
  import AppIcon from './AppIcon.svelte';

  interface Props {
    apps?: LauncherApp[];
    dock?: LauncherApp[];
    now: Date;
    use24h?: boolean;
  }

  let {
    apps = [],
    dock = [],
    now,
    use24h = true
  }: Props = $props();

  const dispatch = createEventDispatcher<{ launch: string }>();

  const PAGE_SIZE = 20; // 4 columns × 5 rows
  let query = $state('');
  let results = $derived(filterApps(apps, query));
  let pages = $derived(paginate(apps, PAGE_SIZE));
  let page = $state(0);
  run(() => {
    if (page > Math.max(0, pages.length - 1)) page = Math.max(0, pages.length - 1);
  });

  // ── pager drag ─────────────────────────────────────────────────────────
  let pageWidth = $state(0);
  let dragX = $state(0);
  let dragging = $state(false);
  let moved = false;
  let startX = 0; let startY = 0; let startT = 0; let pointerId = -1;

  function down(e: PointerEvent) {
    startX = e.clientX; startY = e.clientY; startT = performance.now();
    pointerId = e.pointerId; moved = false; dragging = false; dragX = 0;
  }
  function move(e: PointerEvent) {
    if (e.pointerId !== pointerId) return;
    const dx = e.clientX - startX; const dy = e.clientY - startY;
    if (!dragging && Math.abs(dx) > 10 && Math.abs(dx) > Math.abs(dy)) {
      dragging = true; moved = true;
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    }
    if (dragging) dragX = dx;
  }
  function end(e: PointerEvent) {
    if (e.pointerId !== pointerId) return;
    if (dragging) page = pageAfterDrag(page, pages.length, dragX, pageWidth, performance.now() - startT);
    dragging = false; dragX = 0; pointerId = -1;
  }
  function launch(id: string) { if (!moved) dispatch('launch', id); moved = false; }
</script>

<div class="relative z-10 flex-1 min-h-0 flex flex-col text-white">
  <div class="px-6 pt-6 pb-2 shrink-0">
    <div class="text-5xl font-light tabular-nums">{formatClock(now, use24h)}</div>
    <div class="text-sm text-white/70 mt-1 capitalize">{formatDate(now)}</div>
  </div>

  <div class="px-4 py-2 shrink-0">
    <label class="flex items-center gap-2 bg-white/10 backdrop-blur-md rounded-full px-4 h-11">
      <Search size={16} class="text-white/60" />
      <input bind:value={query} placeholder={$t('topbar.search')} class="flex-1 bg-transparent outline-none text-sm placeholder-white/50" />
      {#if query}<button aria-label={$t('startmenu.close')} onclick={() => (query = '')}><X size={16} /></button>{/if}
    </label>
  </div>

  {#if query}
    <div class="flex-1 min-h-0 overflow-y-auto px-4 pb-2">
      {#each results as app (app.id)}
        <button class="w-full flex items-center gap-4 px-3 h-14 rounded-xl active:bg-white/10 text-left" onclick={() => dispatch('launch', app.id)}>
          <AppIcon icon={app.icon} size={26} title={app.title} /><span class="text-base">{app.title}</span>
        </button>
      {:else}
        <p class="text-center text-white/60 text-sm mt-8">{$t('startmenu.no_results', { q: query })}</p>
      {/each}
    </div>
  {:else}
    <div class="phone-pager flex-1 min-h-0 overflow-hidden relative" bind:clientWidth={pageWidth}
      onpointerdown={down} onpointermove={move} onpointerup={end} onpointercancel={end} role="presentation">
      <div class="flex h-full" style="width:{Math.max(1, pages.length) * 100}%;
           transform:translateX(calc(-{page * (100 / Math.max(1, pages.length))}% + {dragX}px));
           transition:{dragging ? 'none' : 'transform 0.25s ease'};">
        {#each pages as pageApps, i (i)}
          <div class="h-full px-3 pt-2 grid grid-cols-4 content-start gap-y-5" style="width:{100 / pages.length}%;">
            {#each pageApps as app (app.id)}
              <button class="flex flex-col items-center gap-1.5 active:scale-95 transition-transform min-w-0" onclick={() => launch(app.id)}>
                <span class="w-14 h-14 rounded-2xl bg-white/10 backdrop-blur-md flex items-center justify-center"><AppIcon icon={app.icon} size={28} title={app.title} /></span>
                <span class="text-[11px] leading-tight text-center w-full truncate">{app.title}</span>
              </button>
            {/each}
          </div>
        {/each}
      </div>
    </div>

    {#if pages.length > 1}
      <div class="flex justify-center gap-1.5 py-2 shrink-0">
        {#each pages as _, i (i)}
          <button class="h-1.5 rounded-full transition-all {i === page ? 'w-4 bg-white' : 'w-1.5 bg-white/40'}" aria-label={$t('altui.page', { n: i + 1 })} onclick={() => (page = i)}></button>
        {/each}
      </div>
    {/if}
  {/if}

  {#if dock.length}
    <div class="shrink-0 mx-4 mb-3 mt-1 px-3 py-2 rounded-3xl bg-white/10 backdrop-blur-md flex justify-around">
      {#each dock as app (app.id)}
        <button class="w-14 h-14 rounded-2xl flex items-center justify-center active:scale-95 transition-transform" aria-label={app.title} onclick={() => dispatch('launch', app.id)}>
          <AppIcon icon={app.icon} size={30} title={app.title} />
        </button>
      {/each}
    </div>
  {/if}
</div>
