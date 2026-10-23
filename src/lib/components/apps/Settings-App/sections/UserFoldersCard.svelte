<script lang="ts">
  import { run } from 'svelte/legacy';

  /**
   * Settings → Language → "Personal folders": renames an existing English
   * `~/Downloads`, `~/Documents`… to the folder names of the language the
   * person selected (`~/Pobrane`, `~/Dokumenty`…) — for systems installed
   * before the installer started creating localized folders, or after
   * switching the system language. Backed by `plan_user_dirs_migration` /
   * `apply_user_dirs_migration` (src-tauri/src/user_dirs.rs): files are MOVED
   * (a rename, never a copy), folders that exist under both names are left
   * alone, and the file manager updates live when it is done.
   */
  import { onMount } from 'svelte';
  import { FolderSync } from 'lucide-svelte';
  import { language, t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { ensureUserDirsLoaded } from '../../../../utils/userDirs';

  interface Item { key: string; from: string; to: string; action: 'keep' | 'rename' | 'create' | 'conflict' | 'external' }
  interface Plan { lang: string; items: Item[] }
  interface Result { renamed: number; created: number; skipped: number; errors: string[] }

  let plan = $state<Plan | null>(null);
  let loading = $state(false);
  let confirming = $state(false);
  let result = $state<Result | null>(null);
  let error = $state('');

  let work = $derived(plan ? plan.items.filter((i) => i.action === 'rename' || i.action === 'create') : []);

  async function check() {
    loading = true; error = ''; result = null; confirming = false;
    try { plan = await SystemBridge.invokeCommand<Plan>('plan_user_dirs_migration', { lang: $language }); }
    catch (e) { plan = null; error = String(e); }
    loading = false;
  }

  async function apply() {
    loading = true; error = ''; confirming = false;
    try {
      result = await SystemBridge.invokeCommand<Result>('apply_user_dirs_migration', { lang: $language });
      await ensureUserDirsLoaded(true);
      await check_after();
    } catch (e) { error = String(e); }
    loading = false;
  }
  async function check_after() {
    try { plan = await SystemBridge.invokeCommand<Plan>('plan_user_dirs_migration', { lang: $language }); } catch { /* keep */ }
  }

  const base = (p: string) => p.split('/').filter(Boolean).pop() ?? p;
  onMount(check);
  // Re-check when the person switches the UI language.
  let lastLang = $state('');
  run(() => {
    if ($language !== lastLang) { if (lastLang) check(); lastLang = $language; }
  });
</script>

<div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
  <div class="flex items-center gap-3 text-slate-400">
    <FolderSync size={16} class="text-blue-400" />
    <span class="text-sm">{$t('ufold.title')}</span>
  </div>
  <p class="text-xs text-slate-500">{$t('ufold.desc')}</p>

  {#if plan}
    <ul class="space-y-1">
      {#each plan.items as item (item.key)}
        <li class="flex items-center justify-between text-xs px-3 py-1.5 rounded-lg bg-slate-900/40">
          <span class="text-slate-300 truncate">{base(item.from)}{#if item.action === 'rename' || item.action === 'create'} → <span class="text-blue-300">{base(item.to)}</span>{/if}</span>
          <span class="shrink-0 ml-3 {item.action === 'rename' || item.action === 'create' ? 'text-blue-400' : 'text-slate-500'}">{$t(`ufold.act_${item.action}`)}</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if error}<p class="text-xs text-red-400">{$t('ufold.error', { error })}</p>{/if}
  {#if result}
    <p class="text-xs text-emerald-400">{$t('ufold.done', { renamed: result.renamed, created: result.created, skipped: result.skipped })}</p>
    {#each result.errors as e}<p class="text-xs text-red-400">{e}</p>{/each}
  {:else if plan && work.length === 0}
    <p class="text-xs text-slate-500">{$t('ufold.nothing')}</p>
  {/if}

  <div class="flex items-center gap-2">
    <button class="px-3 py-1.5 rounded-lg text-sm bg-slate-700 hover:bg-slate-600 text-slate-200 disabled:opacity-40" disabled={loading} onclick={check}>{$t('ufold.check')}</button>
    {#if work.length > 0 && !confirming}
      <button class="px-3 py-1.5 rounded-lg text-sm bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-40" disabled={loading} onclick={() => (confirming = true)}>{$t('ufold.apply')}</button>
    {/if}
  </div>
  {#if confirming}
    <div class="flex items-center gap-2 flex-wrap rounded-xl bg-amber-500/10 border border-amber-500/30 px-3 py-2">
      <span class="text-xs text-amber-200 flex-1 min-w-[12rem]">{$t('ufold.confirm', { n: work.length })}</span>
      <button class="px-3 py-1 rounded-lg text-xs bg-amber-500 text-slate-900 font-medium" onclick={apply}>{$t('ufold.confirm_yes')}</button>
      <button class="px-3 py-1 rounded-lg text-xs bg-slate-700 text-slate-200" onclick={() => (confirming = false)}>{$t('ufold.confirm_no')}</button>
    </div>
  {/if}
</div>
