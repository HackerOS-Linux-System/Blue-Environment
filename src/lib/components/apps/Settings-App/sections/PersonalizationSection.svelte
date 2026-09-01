<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, Image as IconImageIcon, MousePointer2 } from 'lucide-svelte';
  import type { ThemeDefinition, UserConfig } from '../../../../types';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { dialogPrompt } from '../../../../stores/dialog';
  import { t as translate } from '../../../../stores/language';
  import { configStore } from '../../../../utils/configStore';

  let iconThemes: string[] = [];
  let selectedIconTheme = '';
  let cursorThemes: string[] = [];
  let iconSize = 48;
  let cursorTheme = '';

  onMount(async () => {
    iconThemes = await SystemBridge.invokeCommand<string[]>('list_icon_themes').catch(() => []);
    cursorThemes = await SystemBridge.invokeCommand<string[]>('list_cursor_themes').catch(() => []);
    const cfg = await configStore.init();
    iconSize = cfg.iconSize ?? 48;
    cursorTheme = cfg.cursorTheme ?? '';
  });

  async function applyIconTheme(theme: string) {
    selectedIconTheme = theme;
    await SystemBridge.invokeCommand('set_icon_theme', { theme: theme || null }).catch(() => {});
  }

  async function applyIconSize(size: number) {
    iconSize = size;
    await configStore.save({ iconSize: size });
  }

  async function applyCursorTheme(theme: string) {
    cursorTheme = theme;
    await configStore.save({ cursorTheme: theme });
    if (theme) await SystemBridge.invokeCommand('set_cursor_theme', { theme }).catch(() => {});
  }

  const ICON_SIZES = [24, 32, 48, 64, 96];

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
    const name = await dialogPrompt({ title: $translate('settings.personalization.new_theme_prompt_title'), label: $translate('settings.personalization.new_theme_prompt_label'), placeholder: 'My Theme', confirmLabel: $translate('settings.personalization.next') });
    if (!name) return;

    const list = PRESET_ACCENTS.map((p, i) => `${i + 1}. ${p.label}`).join('  ·  ');
    const choice = await dialogPrompt({ title: $translate('settings.personalization.accent_prompt_title'), label: $translate('settings.personalization.accent_prompt_label').replace('{list}', list), placeholder: '1', defaultValue: '1', confirmLabel: $translate('settings.personalization.create') });
    const idx = Math.min(Math.max(parseInt(choice || '1', 10) - 1, 0), PRESET_ACCENTS.length - 1);
    const preset = PRESET_ACCENTS[isNaN(idx) ? 0 : idx];

    const theme: ThemeDefinition = { id: `custom-${Date.now()}`, name, type: 'custom', colors: { accent: preset.accent, background: preset.background } };
    await handleSave(theme);
  }
</script>

<div class="p-4 space-y-6">
  <div>
    <h2 class="text-lg font-semibold text-white">{$translate('settings.icons.title')}</h2>
  </div>

  <!-- Icon theme -->
  <div>
    <div class="flex items-center gap-2 mb-2"><IconImageIcon size={14} class="text-slate-400" /><h3 class="text-sm font-medium text-white">{$translate('settings.personalization.icon_theme')}</h3></div>
    <p class="text-xs text-slate-500 mb-3">{$translate('settings.personalization.icon_theme_desc')}</p>
    {#if iconThemes.length === 0}
      <p class="text-xs text-slate-600">{$translate('settings.personalization.no_icon_themes')}</p>
    {:else}
      <div class="flex flex-wrap gap-1.5">
        <button on:click={() => applyIconTheme('')} class="px-2.5 py-1 rounded-lg text-xs transition-colors {selectedIconTheme === '' ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white'}">
          {$translate('settings.personalization.icon_theme_auto')}
        </button>
        {#each iconThemes as th (th)}
          <button on:click={() => applyIconTheme(th)} class="px-2.5 py-1 rounded-lg text-xs transition-colors {selectedIconTheme === th ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white'}">
            {th}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Icon size -->
  <div class="pt-4 border-t border-white/5">
    <h3 class="text-sm font-medium text-white mb-1">{$translate('settings.icons.icon_size')}</h3>
    <p class="text-xs text-slate-500 mb-3">{$translate('settings.icons.icon_size_desc')}</p>
    <div class="flex items-center gap-3">
      <div class="flex gap-1.5">
        {#each ICON_SIZES as size (size)}
          <button
            on:click={() => applyIconSize(size)}
            class="px-3 py-1.5 rounded-lg text-xs transition-colors {iconSize === size ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white'}"
          >
            {size}px
          </button>
        {/each}
      </div>
      <div class="flex items-center gap-2 ml-2" style="opacity: 0.9;">
        <span class="text-[10px] text-slate-600 uppercase tracking-wide">{$translate('settings.icons.preview')}</span>
        <div class="rounded-lg bg-slate-800 flex items-center justify-center border border-white/5" style="width:{iconSize + 16}px; height:{iconSize + 16}px;">
          <IconImageIcon size={iconSize} class="text-blue-400" />
        </div>
      </div>
    </div>
  </div>

  <!-- Cursor theme -->
  <div class="pt-4 border-t border-white/5">
    <div class="flex items-center gap-2 mb-2"><MousePointer2 size={14} class="text-slate-400" /><h3 class="text-sm font-medium text-white">{$translate('settings.icons.cursor_theme')}</h3></div>
    <p class="text-xs text-slate-500 mb-3">{$translate('settings.icons.cursor_theme_desc')}</p>
    {#if cursorThemes.length === 0}
      <p class="text-xs text-slate-600">{$translate('settings.icons.no_cursor_themes')}</p>
    {:else}
      <div class="flex flex-wrap gap-1.5">
        {#each cursorThemes as th (th)}
          <button on:click={() => applyCursorTheme(th)} class="px-2.5 py-1 rounded-lg text-xs transition-colors {cursorTheme === th ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white'}">
            {th}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Custom accent themes -->
  <div class="pt-4 border-t border-white/5">
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-sm font-medium text-white">{$translate('settings.icons.accent_themes_title')}</h3>
      <button on:click={createTheme} class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs text-white transition-colors"><Plus size={13} /> {$translate('settings.personalization.new_theme')}</button>
    </div>
    <div class="space-y-2">
      {#each themes as t (t.id)}
        <div class="flex items-center justify-between bg-slate-800 rounded-xl px-4 py-3 border border-white/5">
          <span class="flex items-center gap-2 text-sm text-white">
            {#if t.colors?.accent}<span class="w-3 h-3 rounded-full inline-block" style="background:{t.colors.accent};" />{/if}
            {t.name}
          </span>
          <div class="flex gap-2">
            <button on:click={() => handleSave(t)} class="px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded-lg">{$translate('settings.common.apply')}</button>
            <button on:click={() => handleDelete(t.id)} class="px-3 py-1 text-xs bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg">{$translate('settings.common.delete')}</button>
          </div>
        </div>
      {/each}
      {#if themes.length === 0}<p class="text-slate-500 text-sm">{$translate('settings.personalization.no_themes')}</p>{/if}
    </div>
  </div>
</div>
