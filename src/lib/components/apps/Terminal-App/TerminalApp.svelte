<script lang="ts">
  import 'xterm/css/xterm.css';
  import { Plus } from 'lucide-svelte';
  import { onDestroy } from 'svelte';
  import { createTerminalSession } from './terminalSession';
  import TabBar from './TabBar.svelte';
  import SettingsPanel from './SettingsPanel.svelte';

  const session = createTerminalSession();
  const { tabs, activeTab } = session;
  let showSettings = false;

  onDestroy(() => session.dispose());

  function bindTerminal(el: HTMLDivElement, tabId: string) {
    session.initTerminal(tabId, el);
    return {
      destroy() {},
    };
  }
</script>

<div class="flex flex-col h-full bg-slate-950 text-white overflow-hidden select-none">
  <TabBar {session} {showSettings} onToggleSettings={() => (showSettings = !showSettings)} />

  {#if showSettings}
    <SettingsPanel {session} onClose={() => (showSettings = false)} />
  {/if}

  <div class="flex-1 relative overflow-hidden">
    {#each $tabs as tab (tab.id)}
      <div
        class="absolute inset-0 {$activeTab === tab.id ? '' : 'invisible pointer-events-none'}"
        style="padding:4px;"
        use:bindTerminal={tab.id}
      />
    {/each}
    {#if $tabs.length === 0}
      <div class="flex items-center justify-center h-full text-slate-600">
        <button on:click={session.newTab} class="flex items-center gap-2 hover:text-white transition-colors">
          <Plus size={16} /> New Terminal
        </button>
      </div>
    {/if}
  </div>
</div>
