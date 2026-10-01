<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { ChevronRight, Check } from 'lucide-svelte';
  import { contextMenu, closeContextMenu, type MenuItem } from '../stores/contextMenu';

  let el: HTMLDivElement;
  let pos = { x: 0, y: 0 };
  let openSub: number | null = null;
  let subPos = { x: 0, y: 0, flip: false };
  let active = -1;

  $: if ($contextMenu) { pos = { x: $contextMenu.x, y: $contextMenu.y }; openSub = null; active = -1; tick().then(clamp); }

  // Keep the menu fully inside the viewport (flip up/left near the edges).
  function clamp() {
    if (!el || !$contextMenu) return;
    const r = el.getBoundingClientRect();
    let { x, y } = $contextMenu;
    if (x + r.width > window.innerWidth - 4) x = Math.max(4, window.innerWidth - r.width - 4);
    if (y + r.height > window.innerHeight - 4) y = Math.max(4, window.innerHeight - r.height - 4);
    pos = { x, y };
  }

  async function run(it: MenuItem) {
    if (it.disabled || it.separator || it.children) return;
    closeContextMenu();
    try { await it.action?.(); } catch (e) { console.error('Context menu action failed:', e); }
  }

  function hoverItem(i: number, it: MenuItem, e: MouseEvent) {
    active = i;
    if (it.children || it.emptyLabel) {
      openSub = i;
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const flip = r.right + 230 > window.innerWidth;
      subPos = { x: flip ? r.left : r.right, y: r.top, flip };
    } else openSub = null;
  }

  function onKey(e: KeyboardEvent) {
    const m = $contextMenu; if (!m) return;
    const sel = m.items.map((it, i) => (it.separator || it.disabled ? -1 : i)).filter((i) => i >= 0);
    if (e.key === 'Escape') { closeContextMenu(); e.preventDefault(); }
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const cur = sel.indexOf(active);
      const next = e.key === 'ArrowDown' ? (cur + 1) % sel.length : (cur <= 0 ? sel.length - 1 : cur - 1);
      active = sel[next];
    } else if (e.key === 'Enter' && active >= 0) { e.preventDefault(); run(m.items[active]); }
  }

  const close = () => closeContextMenu();
  onMount(() => {
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('blur', close);
    window.addEventListener('resize', close);
    window.addEventListener('wheel', close, { passive: true });
  });
  onDestroy(() => {
    window.removeEventListener('keydown', onKey, true);
    window.removeEventListener('blur', close);
    window.removeEventListener('resize', close);
    window.removeEventListener('wheel', close);
  });
</script>

{#if $contextMenu}
  <!-- Backdrop: any click (left OR right) outside closes the menu; a right-click
       there is swallowed so it doesn't pop up the native menu either. -->
  <div class="fixed inset-0 z-[99998]" on:mousedown={close} on:contextmenu|preventDefault={close} role="presentation" />

  <div bind:this={el} class="fixed z-[99999] min-w-[200px] max-w-[320px] bg-slate-800/95 backdrop-blur border border-white/10 rounded-xl shadow-2xl py-1 text-sm text-slate-100 select-none"
    style="left:{pos.x}px; top:{pos.y}px;" role="menu" on:contextmenu|preventDefault>
    {#each $contextMenu.items as it, i}
      {#if it.separator}
        <div class="h-px bg-white/10 my-1" />
      {:else}
        <button role="menuitem" disabled={it.disabled}
          on:mouseenter={(e) => hoverItem(i, it, e)} on:click={() => run(it)}
          class="w-full text-left px-3 py-1.5 flex items-center gap-2.5 disabled:opacity-40 disabled:cursor-default
                 {it.danger ? 'text-red-400' : ''} {active === i && !it.disabled ? (it.danger ? 'bg-red-500/20' : 'bg-white/10') : ''}">
          <span class="w-4 shrink-0 flex items-center justify-center">
            {#if it.checked}<Check size={13} />{:else if it.icon}<svelte:component this={it.icon} size={14} />{/if}
          </span>
          <span class="flex-1 truncate">{it.label}</span>
          {#if it.shortcut}<span class="text-xs text-slate-500 ml-4">{it.shortcut}</span>{/if}
          {#if it.children || it.emptyLabel}<ChevronRight size={12} class="text-slate-500" />{/if}
        </button>

        {#if openSub === i && (it.children || it.emptyLabel)}
          <div class="fixed z-[100000] min-w-[200px] max-w-[300px] max-h-[60vh] overflow-y-auto bg-slate-800/95 backdrop-blur border border-white/10 rounded-xl shadow-2xl py-1"
            style="top:{subPos.y}px; {subPos.flip ? `right:${window.innerWidth - subPos.x}px` : `left:${subPos.x}px`};" role="menu">
            {#if !it.children?.length}
              <div class="px-3 py-1.5 text-slate-500 text-xs">{it.emptyLabel}</div>
            {:else}
              {#each it.children as sub}
                {#if sub.separator}<div class="h-px bg-white/10 my-1" />{:else}
                  <button role="menuitem" disabled={sub.disabled} on:click={() => run(sub)}
                    class="w-full text-left px-3 py-1.5 flex items-center gap-2.5 hover:bg-white/10 disabled:opacity-40">
                    <span class="w-4 shrink-0 flex items-center justify-center">
                      {#if sub.checked}<Check size={13} />{:else if sub.icon}<svelte:component this={sub.icon} size={14} />{/if}
                    </span>
                    <span class="flex-1 truncate">{sub.label}</span>
                  </button>
                {/if}
              {/each}
            {/if}
          </div>
        {/if}
      {/if}
    {/each}
  </div>
{/if}
