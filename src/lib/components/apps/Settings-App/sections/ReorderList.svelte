<script lang="ts">
  /**
   * A list the person can reorder by dragging (mouse/touch via HTML5 drag and
   * drop) or with the up/down buttons (keyboard and screen-reader friendly),
   * where every row can also be switched on/off. Used by Settings → Bar for
   * the left and right widget groups. Purely presentational: it emits the new
   * order / the toggled id and the parent saves.
   */
  import { createEventDispatcher } from 'svelte';
  import { ChevronUp, ChevronDown, GripVertical } from 'lucide-svelte';
  import { moveItemTo } from '../../../../utils/topBarConfig';
  import { t } from '../../../../stores/language';

  export let items: string[] = [];
  export let labelFor: (id: string) => string;
  /** Whether a row is currently shown on the bar (rows without a switch: return `true` and set `canToggle` false). */
  export let isOn: (id: string) => boolean = () => true;
  export let canToggle: (id: string) => boolean = () => true;

  const dispatch = createEventDispatcher<{ reorder: string[]; toggle: string }>();

  let dragFrom = -1;
  let dragOver = -1;

  function start(e: DragEvent, i: number) {
    dragFrom = i;
    if (e.dataTransfer) { e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', items[i]); }
  }
  function over(e: DragEvent, i: number) {
    if (dragFrom < 0) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    dragOver = i;
  }
  function drop(e: DragEvent, i: number) {
    e.preventDefault();
    if (dragFrom >= 0) dispatch('reorder', moveItemTo(items, dragFrom, i));
    end();
  }
  function end() { dragFrom = -1; dragOver = -1; }
  const move = (i: number, d: number) => dispatch('reorder', moveItemTo(items, i, i + d));
</script>

<div class="space-y-1.5">
  {#each items as id, i (id)}
    <div role="presentation"
      class="flex items-center gap-2 text-sm text-slate-200 rounded-lg px-2 py-1.5 border transition-colors
             {dragOver === i && dragFrom !== i ? 'border-blue-400 bg-blue-500/10' : 'border-transparent bg-slate-700/40'}
             {dragFrom === i ? 'opacity-50' : ''}"
      on:dragover={(e) => over(e, i)} on:drop={(e) => drop(e, i)}>
      <button type="button" draggable="true" class="p-1 cursor-grab active:cursor-grabbing text-slate-400 hover:text-slate-200"
        title={$t('sbar.order_drag')} aria-label={$t('sbar.order_drag')}
        on:dragstart={(e) => start(e, i)} on:dragend={end}>
        <GripVertical size={14} />
      </button>
      <span class="flex-1 truncate {isOn(id) ? '' : 'opacity-50 line-through'}">{labelFor(id)}</span>
      {#if canToggle(id)}
        <input type="checkbox" checked={isOn(id)} aria-label={labelFor(id)} on:change={() => dispatch('toggle', id)} />
      {/if}
      <span class="flex gap-0.5">
        <button class="p-1 rounded hover:bg-white/10 disabled:opacity-30" disabled={i === 0} title={$t('sbar.order_up')} aria-label={$t('sbar.order_up')} on:click={() => move(i, -1)}><ChevronUp size={14} /></button>
        <button class="p-1 rounded hover:bg-white/10 disabled:opacity-30" disabled={i === items.length - 1} title={$t('sbar.order_down')} aria-label={$t('sbar.order_down')} on:click={() => move(i, 1)}><ChevronDown size={14} /></button>
      </span>
    </div>
  {/each}
</div>
