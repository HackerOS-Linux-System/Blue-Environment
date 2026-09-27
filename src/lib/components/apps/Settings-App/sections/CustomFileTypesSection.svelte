<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, Trash2, Pencil, X, Check, AlertCircle } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import {
    fileTypeAssociations, ensureFileTypeAssociationsLoaded,
    saveFileTypeAssociation, removeFileTypeAssociation,
    newAssociationId, ICON_COMPONENTS, ICON_NAMES,
  } from '../../../../utils/fileTypeAssociations';
  import type { FileTypeAssociation, MatchKind } from '../../../../utils/fileTypeAssociations';

  let editing: FileTypeAssociation | null = null;
  // Typed here rather than as `'extension' as MatchKind` inline in the
  // template below — svelte-check's template-expression parser doesn't
  // accept a TS `as` cast inside a Svelte markup `{...}` expression
  // (only inside `<script>`); see ThemesSection.svelte's COLOR_FIELDS
  // for the same fix applied to a different inline-cast case.
  const MATCH_KIND_EXTENSION: MatchKind = 'extension';
  const MATCH_KIND_MIME: MatchKind = 'mime';
  let error: string | null = null;
  let saving = false;

  onMount(() => { ensureFileTypeAssociationsLoaded(); });

  function blank(): FileTypeAssociation {
    return {
      id: newAssociationId(), kind: 'extension', pattern: '',
      icon: 'Puzzle', color: '#38bdf8', label: '', open_with_command: null,
    };
  }

  function startCreate() { error = null; editing = blank(); }
  function startEdit(a: FileTypeAssociation) { error = null; editing = { ...a }; }
  function cancel() { editing = null; error = null; }

  async function submit() {
    if (!editing) return;
    saving = true; error = null;
    const result = await saveFileTypeAssociation({
      ...editing,
      pattern: editing.pattern.trim().replace(/^\./, ''),
      open_with_command: editing.open_with_command?.trim() || null,
    });
    saving = false;
    if (result.ok) editing = null; else error = result.error;
  }

  async function remove(id: string) {
    await removeFileTypeAssociation(id);
  }
</script>

<div class="space-y-6">
  <div>
    <h2 class="text-2xl font-bold text-white">{$t('settings.filetypes.title')}</h2>
    <p class="text-slate-500 text-sm mt-1">{$t('settings.filetypes.desc')}</p>
  </div>

  {#if !editing}
    <button on:click={startCreate}
      class="flex items-center gap-2 px-4 py-2.5 bg-blue-600 hover:bg-blue-500 text-white rounded-xl text-sm font-medium transition-colors">
      <Plus size={15} /> {$t('settings.filetypes.add')}
    </button>
  {/if}

  {#if editing}
    <div class="bg-slate-800 rounded-2xl border border-white/5 p-5 space-y-4">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-semibold text-white">{$t('settings.filetypes.editor_title')}</h3>
        <button on:click={cancel} class="text-slate-500 hover:text-white"><X size={16} /></button>
      </div>

      <div class="flex gap-2">
        <label class="flex items-center gap-1.5 text-xs text-slate-400 cursor-pointer">
          <input type="radio" bind:group={editing.kind} value={MATCH_KIND_EXTENSION} class="accent-blue-500" />
          {$t('settings.filetypes.match_extension')}
        </label>
        <label class="flex items-center gap-1.5 text-xs text-slate-400 cursor-pointer ml-4">
          <input type="radio" bind:group={editing.kind} value={MATCH_KIND_MIME} class="accent-blue-500" />
          {$t('settings.filetypes.match_mime')}
        </label>
      </div>

      <div>
        <label class="block text-xs text-slate-500 mb-1">
          {editing.kind === 'extension' ? $t('settings.filetypes.pattern_ext_label') : $t('settings.filetypes.pattern_mime_label')}
        </label>
        <input bind:value={editing.pattern}
          placeholder={editing.kind === 'extension' ? 'blueproj' : 'application/x-mymod  (or application/*)'}
          class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white font-mono focus:outline-none focus:border-blue-500/50" />
      </div>

      <div>
        <label class="block text-xs text-slate-500 mb-1">{$t('settings.filetypes.label_label')}</label>
        <input bind:value={editing.label} placeholder={$t('settings.filetypes.label_placeholder')}
          class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
      </div>

      <div class="flex gap-4">
        <div class="flex-1">
          <label class="block text-xs text-slate-500 mb-1">{$t('settings.filetypes.icon_label')}</label>
          <div class="grid grid-cols-8 gap-1.5 max-h-32 overflow-y-auto bg-slate-900 rounded-lg p-2 border border-white/10">
            {#each ICON_NAMES as name (name)}
              <button on:click={() => editing && (editing.icon = name)}
                title={name}
                class="aspect-square flex items-center justify-center rounded-lg transition-colors {editing.icon === name ? 'bg-blue-500/30 ring-1 ring-blue-400' : 'hover:bg-white/10'}"
                style="color: {editing.icon === name ? editing.color : '#94a3b8'}">
                <svelte:component this={ICON_COMPONENTS[name]} size={16} />
              </button>
            {/each}
          </div>
        </div>
        <div class="w-28">
          <label class="block text-xs text-slate-500 mb-1">{$t('settings.filetypes.color_label')}</label>
          <input type="color" bind:value={editing.color} class="w-full h-9 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
          <div class="mt-2 flex items-center justify-center bg-slate-900 rounded-lg py-3 border border-white/10">
            <svelte:component this={ICON_COMPONENTS[editing.icon]} size={28} style="color: {editing.color}" />
          </div>
        </div>
      </div>

      <div>
        <label class="block text-xs text-slate-500 mb-1">{$t('settings.filetypes.open_with_label')}</label>
        <input bind:value={editing.open_with_command} placeholder="my-app --open {'{path}'}"
          class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white font-mono focus:outline-none focus:border-blue-500/50" />
        <p class="text-xs text-slate-600 mt-1">{$t('settings.filetypes.open_with_hint')}</p>
      </div>

      {#if error}
        <div class="flex items-center gap-2 text-red-400 text-sm bg-red-500/10 rounded-lg px-3 py-2">
          <AlertCircle size={14} /> {error}
        </div>
      {/if}

      <div class="flex gap-2 pt-1">
        <button on:click={submit} disabled={saving || !editing.pattern.trim()}
          class="flex items-center gap-1.5 px-4 py-2 bg-blue-600 hover:bg-blue-500 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg text-sm font-medium transition-colors">
          <Check size={14} /> {$t('settings.filetypes.save')}
        </button>
        <button on:click={cancel} class="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-lg text-sm transition-colors">
          {$t('settings.filetypes.cancel')}
        </button>
      </div>
    </div>
  {/if}

  <div class="space-y-2">
    {#each $fileTypeAssociations as assoc (assoc.id)}
      <div class="flex items-center gap-3 bg-slate-800 rounded-xl border border-white/5 px-4 py-3">
        <div class="w-9 h-9 rounded-lg bg-slate-900 flex items-center justify-center shrink-0" style="color: {assoc.color}">
          <svelte:component this={ICON_COMPONENTS[assoc.icon] ?? ICON_COMPONENTS.File} size={18} />
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-sm text-white font-medium truncate">{assoc.label || assoc.pattern}</div>
          <div class="text-xs text-slate-500 font-mono truncate">
            {assoc.kind === 'extension' ? `*.${assoc.pattern}` : assoc.pattern}
            {#if assoc.open_with_command} · {$t('settings.filetypes.opens_with')} {assoc.open_with_command}{/if}
          </div>
        </div>
        <button on:click={() => startEdit(assoc)} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white"><Pencil size={14} /></button>
        <button on:click={() => remove(assoc.id)} class="p-1.5 hover:bg-red-500/20 rounded-lg text-slate-400 hover:text-red-400"><Trash2 size={14} /></button>
      </div>
    {:else}
      {#if !editing}
        <p class="text-sm text-slate-600 italic">{$t('settings.filetypes.empty')}</p>
      {/if}
    {/each}
  </div>
</div>
