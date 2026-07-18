<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, Palette as PaletteIcon } from 'lucide-svelte';
  import type { ThemeDefinition } from '../../../../types';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { dialogPrompt } from '../../../../stores/dialog';

  let iconThemes: string[] = [];
  let selectedIconTheme = '';

  onMount(async () => {
    iconThemes = await SystemBridge.invokeCommand<string[]>('list_icon_themes').catch(() => []);
  });

  async function applyIconTheme(theme: string) {
    selectedIconTheme = theme;
    await SystemBridge.invokeCommand('set_icon_theme', { theme: theme || null }).catch(() => {});
  }

  const PRESET_ACCENTS = [
    { label: 'Blue', accent: '#3b82f6', background: '#0f172a' },
    { label: 'Violet', accent: '#8b5cf6', background: '#1e1b2e' },
    { label: 'Emerald', accent: '#10b981', background: '#0f1f1a' },
    { label: 'Rose', accent: '#f43f5e', background: '#1f0f17' },
    { label: 'Amber', accent: '#f59e0b', background: '#1f1709' },
  ];

  let themes: ThemeDefinition[] = [];

  onMount(() => {
    SystemBridge.getCustomThemes().then((ts: any[]) => (themes = ts));
  });

  async function handleSave(t: ThemeDefinition) {
    await SystemBridge.saveCustomTheme(t as any);
    const idx = themes.findIndex((x) => x.id === t.id);
    if (idx >= 0) { const n = [...themes]; n[idx] = t; themes = n; }
    else themes = [...themes, t];
  }

  async function handleDelete(id: string) {
    await SystemBridge.deleteCustomTheme(id);
    themes = themes.filter((t) => t.id !== id);
  }

  async function createTheme() {
    const name = await dialogPrompt({ title: 'New Theme', label: 'Give your custom theme a name.', placeholder: 'My Theme', confirmLabel: 'Next' });
    if (!name) return;

    const list = PRESET_ACCENTS.map((p, i) => `${i + 1}. ${p.label}`).join('  ·  ');
    const choice = await dialogPrompt({ title: 'Accent color', label: `Pick a palette: ${list}`, placeholder: '1', defaultValue: '1', confirmLabel: 'Create' });
    const idx = Math.min(Math.max(parseInt(choice || '1', 10) - 1, 0), PRESET_ACCENTS.length - 1);
    const preset = PRESET_ACCENTS[isNaN(idx) ? 0 : idx];

    const theme: ThemeDefinition = { id: `custom-${Date.now()}`, name, type: 'custom', colors: { accent: preset.accent, background: preset.background } };
    await handleSave(theme);
  }
</script>

<div class="p-4 space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-white">Personalization</h2>
    <button on:click={createTheme} class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs text-white transition-colors"><Plus size={13} /> New Theme</button>
  </div>
  <div class="space-y-2">
    {#each themes as t (t.id)}
      <div class="flex items-center justify-between bg-slate-800 rounded-xl px-4 py-3 border border-white/5">
        <span class="flex items-center gap-2 text-sm text-white">
          {#if t.colors?.accent}<span class="w-3 h-3 rounded-full inline-block" style="background:{t.colors.accent};" />{/if}
          {t.name}
        </span>
        <div class="flex gap-2">
          <button on:click={() => handleSave(t)} class="px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded-lg">Apply</button>
          <button on:click={() => handleDelete(t.id)} class="px-3 py-1 text-xs bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg">Delete</button>
        </div>
      </div>
    {/each}
    {#if themes.length === 0}<p class="text-slate-500 text-sm">No custom themes yet — create one above.</p>{/if}
  </div>

  <div class="pt-4 border-t border-white/5">
    <div class="flex items-center gap-2 mb-2"><PaletteIcon size={14} class="text-slate-400" /><h3 class="text-sm font-medium text-white">Icon Theme</h3></div>
    <p class="text-xs text-slate-500 mb-3">
      Applies to icons for external apps in the Start Menu (Blue Environment's own apps always use their built-in icons).
      Scanned from /usr/share/icons and ~/.local/share/icons.
    </p>
    {#if iconThemes.length === 0}
      <p class="text-xs text-slate-600">No installed icon themes with an index.theme found — Papirus is used as the built-in fallback.</p>
    {:else}
      <div class="flex flex-wrap gap-1.5">
        <button on:click={() => applyIconTheme('')} class="px-2.5 py-1 rounded-lg text-xs transition-colors {selectedIconTheme === '' ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white'}">
          Auto (system default)
        </button>
        {#each iconThemes as t (t)}
          <button on:click={() => applyIconTheme(t)} class="px-2.5 py-1 rounded-lg text-xs transition-colors {selectedIconTheme === t ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white'}">
            {t}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>
