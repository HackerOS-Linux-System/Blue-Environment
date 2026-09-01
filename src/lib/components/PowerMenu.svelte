<script lang="ts">
  import { Power, RefreshCcw, Moon, HardDrive, X } from 'lucide-svelte';
  import type { PowerAction } from '../types';
  import { createEventDispatcher, onDestroy } from 'svelte';

  const dispatch = createEventDispatcher<{ action: PowerAction; close: void }>();

  /** Same convention as StartMenu/ControlCenter's `shellThemeId` prop —
   * this menu is a centered modal (not taskbar-anchored), so it doesn't
   * need their `panelPosition` fix, just the Hydra glow/border accent. */
  export let shellThemeId: string | undefined = undefined;

  const ACTIONS = [
    { action: 'shutdown' as PowerAction, label: 'Shut Down', Icon: Power, color: '#ef4444', glow: 'rgba(239,68,68,0.3)', description: 'Power off the system' },
    { action: 'reboot' as PowerAction, label: 'Restart', Icon: RefreshCcw, color: '#f59e0b', glow: 'rgba(245,158,11,0.3)', description: 'Reboot the system' },
    { action: 'suspend' as PowerAction, label: 'Suspend', Icon: Moon, color: '#3b82f6', glow: 'rgba(59,130,246,0.3)', description: 'Sleep — resume quickly' },
    { action: 'hibernate' as PowerAction, label: 'Hibernate', Icon: HardDrive, color: '#8b5cf6', glow: 'rgba(139,92,246,0.3)', description: 'Save state to disk' },
  ] as const;

  let confirming: PowerAction | null = null;
  let countdown = 5;
  let timer: ReturnType<typeof setInterval>;

  function handleSelect(action: PowerAction) {
    confirming = action;
    countdown = 5;
    clearInterval(timer);
    timer = setInterval(() => {
      countdown -= 1;
      if (countdown <= 0) {
        clearInterval(timer);
        dispatch('action', action);
      }
    }, 1000);
  }

  function cancel() {
    clearInterval(timer);
    confirming = null;
  }

  onDestroy(() => clearInterval(timer));

  $: selected = ACTIONS.find((a) => a.action === confirming);

  function hoverIn(e: MouseEvent, color: string, glow: string) {
    const el = e.currentTarget as HTMLElement;
    const rgb = color.slice(1).match(/../g)!.map((x) => parseInt(x, 16)).join(',');
    el.style.background = `rgba(${rgb},0.08)`;
    el.style.borderColor = `${color}40`;
    el.style.boxShadow = `0 0 24px ${glow}`;
  }
  function hoverOut(e: MouseEvent) {
    const el = e.currentTarget as HTMLElement;
    el.style.background = 'rgba(8,20,45,0.6)';
    el.style.borderColor = 'rgba(255,255,255,0.06)';
    el.style.boxShadow = 'none';
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center"
  style="background:rgba(2,8,18,0.8); backdrop-filter:blur(20px);"
  on:click={() => dispatch('close')} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => dispatch('close'))(); } }}
>
  <div
    class="relative glass-card rounded-3xl p-8 w-96 animate-scale-in"
    on:click|stopPropagation
    style="border:1px solid {shellThemeId === 'hydra' ? 'rgba(236,72,153,0.3)' : 'rgba(59,130,246,0.2)'}; {shellThemeId === 'hydra' ? 'box-shadow: 0 0 40px rgba(236,72,153,0.25);' : ''}"
  >
    <button on:click={() => dispatch('close')} class="absolute top-4 right-4 p-2 bedm-btn-ghost rounded-xl">
      <X size={16} />
    </button>

    {#if confirming && selected}
      <div class="text-center">
        <div class="w-20 h-20 rounded-full flex items-center justify-center mx-auto mb-6"
             style="background:radial-gradient(circle, {selected.glow} 0%, transparent 70%); border:2px solid {selected.color}30;">
          <svelte:component this={selected.Icon} size={36} color={selected.color}
            style="filter:drop-shadow(0 0 8px {selected.color});" />
        </div>
        <div class="text-6xl font-light tabular-nums mb-2" style="font-family:'Oxanium', monospace; color:{selected.color};">
          {countdown}
        </div>
        <p class="text-slate-300 mb-1 text-lg">{selected.label}ing…</p>
        <p class="text-slate-500 text-sm mb-8">{selected.description}</p>
        <button on:click={cancel} class="bedm-btn-ghost px-8 py-3 rounded-xl text-sm font-medium">
          Cancel
        </button>
      </div>
    {:else}
      <h2 class="text-center text-white mb-2" style="font-family:'Oxanium', monospace; font-size:1.25rem; font-weight:400;">
        Power Options
      </h2>
      <p class="text-center text-slate-500 text-sm mb-8">Choose an action</p>
      <div class="grid grid-cols-2 gap-3">
        {#each ACTIONS as { action, label, Icon, color, glow, description } (action)}
          <button
            on:click={() => handleSelect(action)}
            class="group flex flex-col items-center gap-3 p-5 rounded-2xl transition-all duration-200"
            style="background:rgba(8,20,45,0.6); border:1px solid rgba(255,255,255,0.06);"
            on:mouseenter={(e) => hoverIn(e, color, glow)}
            on:mouseleave={hoverOut}
          >
            <svelte:component this={Icon} size={28} class="text-slate-500 group-hover:scale-110 transition-all" />
            <div>
              <div class="text-slate-300 text-sm font-medium group-hover:text-white transition-colors">{label}</div>
              <div class="text-slate-600 text-xs mt-0.5">{description}</div>
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>
