<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { t } from '../lib/stores/language';
  import type { LauncherApp, RecentItem } from '../lib/altShell/apps';
  import TvStatus from './TvStatus.svelte';

  interface Props {
    apps?: LauncherApp[];
    running?: RecentItem[];
    now: Date;
    use24h?: boolean;
  }

  let {
    apps = [],
    running = [],
    now,
    use24h = true
  }: Props = $props();

  const dispatch = createEventDispatcher<{ launch: string; resume: string; menu: void }>();
</script>

<div class="relative z-10 h-full flex flex-col text-white overflow-y-auto" data-tv-home>
  <TvStatus {now} {use24h} />

  {#if running.length}
    <section class="px-12 pt-8">
      <h2 class="text-xl font-semibold text-white/80 mb-4">{$t('altui.running')}</h2>
      <div class="flex gap-6 overflow-x-auto py-4 -my-4 px-2 -mx-2">
        {#each running as item (item.id)}
          <button data-tv-focusable data-tv-key={`run:${item.id}`} class="tv-tile shrink-0 w-64 h-36 rounded-3xl bg-white/12 backdrop-blur-md flex flex-col items-center justify-center gap-3"
            onclick={() => dispatch('resume', item.id)}>
            <item.icon size={44} />
            <span class="text-lg truncate max-w-[90%]">{item.title}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  <section class="px-12 pt-8 pb-28">
    <h2 class="text-xl font-semibold text-white/80 mb-4">{$t('altui.apps')}</h2>
    <div class="grid grid-cols-5 gap-6 p-2 -m-2">
      {#each apps as app (app.id)}
        <button data-tv-focusable data-tv-key={`app:${app.id}`} class="tv-tile h-40 rounded-3xl bg-white/10 backdrop-blur-md flex flex-col items-center justify-center gap-3 min-w-0"
          onclick={() => dispatch('launch', app.id)}>
          {#if typeof app.icon === 'string'}
            <img src={app.icon} alt="" width="48" height="48" class="object-contain" draggable="false" />
          {:else}
            <app.icon size={48} />
          {/if}
          <span class="text-lg truncate max-w-[90%]">{app.title}</span>
        </button>
      {/each}
    </div>
  </section>
</div>
