<script lang="ts">
  /**
   * Running-apps strip in the top bar's left section (it replaced the old
   * "Search apps…" pill, which only duplicated the Start button).
   *
   *  • shows the windows open on the current workspace — `max` of them
   *    (Settings → Panel → Bar, default 4), the rest collapse into a "+N" chip;
   *  • a click on an entry focuses it, or minimizes it when it already has focus;
   *  • a DOUBLE click anywhere on the strip (or one click on "+N") opens the list
   *    of ALL open apps, across every workspace, with a close button per window.
   *    A double click never also toggles a window: the second click of the pair
   *    (`detail > 1`) is ignored by the entries.
   */
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import { X, ChevronsUpDown, AppWindow, MinusSquare } from 'lucide-svelte';
  import AppIconGlyph from './AppIconGlyph.svelte';
  import { APPS } from '../constants';
  import { t } from '../stores/language';
  import { showContextMenu, type MenuItem } from '../stores/contextMenu';
  import { CLOSE_POPUPS_EVENT } from '../utils/popups';
  import { pickVisible, groupByWorkspace, type RunningItem } from '../utils/runningApps';

  /** Every open window, all workspaces. */
  export let items: RunningItem[] = [];
  export let currentWorkspace = 0;
  export let max = 4;
  export let showLabels = true;
  export let position: 'top' | 'bottom' = 'top';

  const dispatch = createEventDispatcher<{ activate: RunningItem; close: RunningItem }>();

  let listOpen = false;

  $: onWorkspace = items.filter((i) => i.workspace === currentWorkspace);
  $: ({ visible, hidden } = pickVisible(onWorkspace, max));
  $: groups = groupByWorkspace(items, currentWorkspace);
  // The list can outlive its content: close it when the last window goes away.
  $: if (listOpen && items.length === 0) listOpen = false;

  function iconFor(item: RunningItem): unknown {
    if (item.iconPath) return item.iconPath;
    const app = item.appId ? APPS[item.appId as keyof typeof APPS] : undefined;
    return app?.icon ?? '';
  }

  function titleFor(item: RunningItem): string {
    if (item.title) return item.title;
    const app = item.appId ? APPS[item.appId as keyof typeof APPS] : undefined;
    return app?.title ?? $t('running.untitled');
  }

  function activate(item: RunningItem, e?: MouseEvent) {
    // The second click of a double click must not undo the first one.
    if (e && e.detail > 1) return;
    dispatch('activate', item);
  }

  function pick(item: RunningItem) {
    listOpen = false;
    dispatch('activate', item);
  }

  function openList() {
    listOpen = true;
  }

  function entryMenu(e: MouseEvent, item: RunningItem) {
    e.stopPropagation();
    const entries: MenuItem[] = [
      { label: item.isActive && !item.isMinimized ? $t('running.minimize') : $t('running.show'), action: () => dispatch('activate', item) },
      { separator: true },
      { label: $t('running.close'), danger: true, action: () => dispatch('close', item) },
    ];
    showContextMenu(e, entries);
  }

  /** Middle click closes, like on a browser tab. */
  function auxClick(e: MouseEvent, item: RunningItem) {
    if (e.button !== 1) return;
    e.preventDefault();
    dispatch('close', item);
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (listOpen && e.key === 'Escape') {
      listOpen = false;
      e.stopPropagation();
    }
  }

  const closeList = () => (listOpen = false);
  onMount(() => window.addEventListener(CLOSE_POPUPS_EVENT, closeList));
  onDestroy(() => window.removeEventListener(CLOSE_POPUPS_EVENT, closeList));
</script>

<svelte:window on:keydown={onWindowKeydown} />

<div class="relative hidden md:flex items-center min-w-0 flex-1 max-w-full">
  <div
    class="flex items-center gap-0.5 min-w-0 max-w-full rounded-full bg-slate-800/60 border border-white/5 px-1 py-0.5 transition-colors hover:bg-slate-800/80"
    title={$t('running.hint')}
    on:dblclick={openList}
    role="group"
    aria-label={$t('running.label')}
  >
    {#if visible.length === 0}
      <span class="px-2.5 py-1 text-xs text-slate-500 truncate select-none">{$t('running.none')}</span>
    {:else}
      {#each visible as item (item.id)}
        {@const dim = item.isMinimized}
        <button
          type="button"
          on:click={(e) => activate(item, e)}
          on:auxclick={(e) => auxClick(e, item)}
          on:contextmenu={(e) => entryMenu(e, item)}
          title={titleFor(item)}
          class="relative flex items-center gap-1.5 min-w-0 rounded-full pl-1.5 pr-1.5 xl:pr-2.5 py-1 text-xs transition-colors {item.isActive && !dim ? 'bg-blue-600/25 text-white' : 'text-slate-300 hover:bg-white/10 hover:text-white'} {dim ? 'opacity-60' : ''}"
        >
          <span class="shrink-0 flex items-center justify-center w-4 h-4 overflow-hidden"><AppIconGlyph icon={iconFor(item)} name={titleFor(item)} size={16} /></span>
          {#if showLabels}
            <span class="hidden xl:block truncate max-w-[104px]">{titleFor(item)}</span>
          {/if}
          {#if item.isActive && !dim}
            <span class="absolute -bottom-px left-1/2 -translate-x-1/2 h-0.5 w-3 rounded-full bg-blue-400" />
          {/if}
        </button>
      {/each}
      {#if hidden > 0}
        <button
          type="button"
          on:click={openList}
          title={$t('running.show_all', { n: items.length })}
          class="shrink-0 flex items-center gap-0.5 rounded-full px-2 py-1 text-[11px] font-semibold text-slate-300 hover:bg-white/10 hover:text-white transition-colors"
        >+{hidden}</button>
      {/if}
    {/if}
  </div>

  {#if listOpen}
    <div class="fixed inset-0 z-40" on:click={closeList} role="presentation" />
    <div
      data-shell-occluder="running-apps"
      class="absolute left-0 w-80 max-h-[60vh] flex flex-col bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl z-50 overflow-hidden {position === 'top' ? 'top-full mt-2' : 'bottom-full mb-2'}"
      role="dialog"
      aria-label={$t('running.all_title', { n: items.length })}
    >
      <div class="flex items-center justify-between px-4 py-3 border-b border-white/5 shrink-0">
        <div class="flex items-center gap-2 text-sm font-semibold text-slate-100">
          <ChevronsUpDown size={14} class="text-blue-400" />
          {$t('running.all_title', { n: items.length })}
        </div>
        <button type="button" on:click={closeList} class="p-1 rounded-lg text-slate-400 hover:text-white hover:bg-white/10" aria-label={$t('running.close_list')}><X size={14} /></button>
      </div>
      <div class="overflow-y-auto p-2 flex flex-col gap-1">
        {#if items.length === 0}
          <div class="flex flex-col items-center gap-2 py-8 text-slate-500 text-xs"><AppWindow size={26} class="opacity-50" />{$t('running.none')}</div>
        {/if}
        {#each groups as group (group.workspace)}
          {#if groups.length > 1}
            <div class="px-2 pt-1.5 pb-0.5 text-[10px] font-semibold uppercase tracking-wider text-slate-500">
              {$t('running.workspace', { n: group.workspace + 1 })}{group.workspace === currentWorkspace ? ` · ${$t('running.current')}` : ''}
            </div>
          {/if}
          {#each group.items as item (item.id)}
            <div class="group flex items-center gap-1 rounded-xl hover:bg-white/5 {item.isActive && !item.isMinimized ? 'bg-blue-600/15' : ''}">
              <button type="button" on:click={() => pick(item)} on:contextmenu={(e) => entryMenu(e, item)} class="flex-1 min-w-0 flex items-center gap-3 px-2 py-1.5 text-left">
                <span class="w-8 h-8 shrink-0 rounded-xl bg-slate-800 border border-white/5 flex items-center justify-center overflow-hidden"><AppIconGlyph icon={iconFor(item)} name={titleFor(item)} size={20} /></span>
                <span class="min-w-0 flex-1">
                  <span class="block text-sm text-slate-200 group-hover:text-white truncate">{titleFor(item)}</span>
                  <span class="flex items-center gap-1 text-[10px] text-slate-500">
                    {#if item.isMinimized}<MinusSquare size={9} />{$t('running.minimized')}{:else if item.isActive}{$t('running.focused')}{:else}{$t('running.open')}{/if}
                  </span>
                </span>
              </button>
              <button type="button" on:click={() => dispatch('close', item)} title={$t('running.close')} aria-label={$t('running.close')}
                class="mr-1 p-1.5 rounded-lg text-slate-500 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:text-red-400 hover:bg-red-500/10 transition-opacity"><X size={13} /></button>
            </div>
          {/each}
        {/each}
      </div>
    </div>
  {/if}
</div>
