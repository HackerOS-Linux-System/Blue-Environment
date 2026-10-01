<script lang="ts">
  import 'xterm/css/xterm.css';
  import { Plus } from 'lucide-svelte';
  import { onDestroy } from 'svelte';
  import { createTerminalSession } from './terminalSession';
  import { Copy, ClipboardPaste, TextSelect, Eraser } from 'lucide-svelte';
  import { showContextMenu } from '../../../stores/contextMenu';
  import TabBar from './TabBar.svelte';
  import SettingsPanel from './SettingsPanel.svelte';

  const session = createTerminalSession();
  const { tabs, activeTab } = session;
  let showSettings = false;

  onDestroy(() => session.dispose());

  function termMenu(e: MouseEvent, tabId: string) {
    const sel = session.getSelection(tabId);
    showContextMenu(e, [
      { label: 'Copy', icon: Copy, shortcut: 'Ctrl+Shift+C', disabled: !sel, action: () => navigator.clipboard.writeText(sel).catch(() => {}) },
      { label: 'Paste', icon: ClipboardPaste, shortcut: 'Ctrl+Shift+V', action: async () => { try { session.pasteText(tabId, await navigator.clipboard.readText()); } catch { /* clipboard denied */ } } },
      { separator: true },
      { label: 'Select all', icon: TextSelect, action: () => session.selectAll(tabId) },
      { label: 'Clear', icon: Eraser, action: () => session.clearTerm(tabId) },
      { separator: true },
      { label: 'New tab', action: session.newTab },
      { label: 'Close tab', danger: true, action: () => session.closeTab(tabId) },
    ]);
  }

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
        on:contextmenu={(e) => termMenu(e, tab.id)}
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
