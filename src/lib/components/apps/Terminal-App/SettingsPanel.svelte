<script lang="ts">
  import { X } from 'lucide-svelte';
  import { THEMES, type ThemeName } from './themes';
  import type { TerminalSession } from './terminalSession';

  interface Props {
    session: TerminalSession;
    onClose: () => void;
  }

  let { session, onClose }: Props = $props();

  const { themeName, fontSize } = session;
</script>

<div class="shrink-0 flex items-center gap-4 px-4 py-2 bg-slate-900 border-b border-white/5 text-sm">
  <div class="flex items-center gap-2">
    <span class="text-slate-400 text-xs">Theme:</span>
    <select bind:value={$themeName} class="bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-white focus:outline-none">
      {#each Object.keys(THEMES) as t (t)}<option value={t}>{t}</option>{/each}
    </select>
  </div>
  <div class="flex items-center gap-2">
    <span class="text-slate-400 text-xs">Font size:</span>
    <input type="number" bind:value={$fontSize} min="10" max="24"
      class="bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-white w-14 focus:outline-none" />
  </div>
  <button onclick={onClose} class="ml-auto text-slate-500 hover:text-white"><X size={13} /></button>
</div>
