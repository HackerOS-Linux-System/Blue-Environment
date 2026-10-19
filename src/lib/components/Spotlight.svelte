<script lang="ts">
  import { createEventDispatcher, onMount, tick } from 'svelte';
  import { Search, AppWindow, Settings as SettingsIcon, File as FileIcon, Folder, CalendarDays, Mail, Calculator, Copy, Loader2 } from 'lucide-svelte';
  import { t } from '../stores/language';
  import { SystemBridge, shellQuote } from '../utils/systemBridge';
  import { APPS } from '../constants';
  import { AppId } from '../types';
  import { openApp } from '../stores/windowManager';
  import { rank, evalMath, formatNumber, buildSections, flatten, moveSelection, scoreText, tokens, type ResultItem, type ResultSection } from '../utils/spotlight';
  import { SETTINGS_PAGES } from '../utils/settingsPages';

  export let isOpen = false;
  export let zIndex = 9000;

  const dispatch = createEventDispatcher<{ close: void }>();

  interface DesktopApp { id: string; name: string; comment: string; exec: string; is_external: boolean }
  interface FileHit { name: string; path: string; isDir: boolean }
  interface EventHit { id: string; title: string; date: string; time: string | null }
  interface MailHit { accountId: string; folder: string; uid: string; from: string; subject: string; date: string }
  interface SearchResults { files: FileHit[]; events: EventHit[]; mails: MailHit[] }

  let query = '';
  let input: HTMLInputElement | undefined;
  let sections: ResultSection[] = [];
  let selected = 0;
  let loading = false;
  let systemApps: DesktopApp[] = [];
  let debounce: ReturnType<typeof setTimeout> | undefined;
  let token = 0;

  $: flat = flatten(sections);

  $: if (isOpen) { void onOpen(); }
  async function onOpen() {
    query = ''; sections = []; selected = 0;
    await tick(); input?.focus();
    if (!systemApps.length) systemApps = (await SystemBridge.getSystemApps(false).catch(() => [])) as unknown as DesktopApp[];
  }

  onMount(() => () => { if (debounce) clearTimeout(debounce); });

  const internalApps = () => Object.values(APPS).map((a) => ({ id: String(a.id), title: a.title }));

  function localResults(q: string) {
    const calc = evalMath(q);
    const parts: Partial<Record<ResultItem['kind'], ResultItem[]>> = {};
    if (calc !== null) parts.calc = [{ kind: 'calc', id: 'calc', title: `= ${formatNumber(calc)}`, subtitle: q.trim(), payload: formatNumber(calc) }];

    const internal = rank(internalApps(), q, (a) => a.title, (a) => a.id, 6).map<ResultItem>((a) => ({ kind: 'app', id: `i:${a.id}`, title: a.title, payload: { internal: a.id } }));
    const external = rank(systemApps.filter((a) => a.is_external !== false), q, (a) => a.name, (a) => a.comment ?? '', 6).map<ResultItem>((a) => ({ kind: 'app', id: `e:${a.id}`, title: a.name, subtitle: a.comment || undefined, payload: { exec: a.exec, appId: a.id } }));
    // internal apps first, but a better-matching external app can lead
    parts.app = [...internal, ...external].slice(0, 8);

    const toks = tokens(q);
    parts.setting = SETTINGS_PAGES
      .map((p) => ({ p, label: $t(p.labelKey) || p.en }))
      .filter(({ p, label }) => scoreText(label, toks, `${p.en} ${p.keywords}`) !== null)
      .slice(0, 4)
      .map<ResultItem>(({ p, label }) => ({ kind: 'setting', id: `s:${p.tab}`, title: label, subtitle: $t('spotlight.settings'), payload: { tab: p.tab } }));
    return parts;
  }

  function onInput() {
    selected = 0;
    if (debounce) clearTimeout(debounce);
    const q = query;
    if (q.trim().length < 1) { sections = []; loading = false; return; }
    const local = localResults(q);
    sections = buildSections(local);
    if (q.trim().length < 2) return;
    loading = true;
    const my = ++token;
    debounce = setTimeout(async () => {
      try {
        const r = await SystemBridge.invokeCommand<SearchResults>('global_search', { query: q, limit: 6 });
        if (my !== token) return;
        sections = buildSections({
          ...local,
          event: r.events.map<ResultItem>((e) => ({ kind: 'event', id: `ev:${e.id}`, title: e.title, subtitle: `${e.date}${e.time ? ' ' + e.time : ''}`, payload: e })),
          mail: r.mails.map<ResultItem>((m) => ({ kind: 'mail', id: `m:${m.accountId}:${m.uid}`, title: m.subject || '(no subject)', subtitle: m.from, payload: m })),
          file: r.files.map<ResultItem>((f) => ({ kind: 'file', id: `f:${f.path}`, title: f.name, subtitle: f.path.replace(/^\/home\/[^/]+/, '~'), payload: f })),
        });
      } catch { /* backend search is a bonus; local results stay */ }
      finally { if (my === token) loading = false; }
    }, 160);
  }

  function close() { dispatch('close'); }

  async function activate(item: ResultItem | undefined) {
    if (!item) return;
    const p = item.payload as any;
    close();
    switch (item.kind) {
      case 'calc': await navigator.clipboard?.writeText(String(p)).catch(() => {}); break;
      case 'app':
        if (p.internal) openApp(p.internal);
        else void SystemBridge.launchApp(p.exec, p.appId).catch(() => {});
        break;
      case 'setting': openApp(AppId.SETTINGS, false, undefined, { initialTab: p.tab }); break;
      case 'event': openApp(AppId.BLUE_CALENDAR); break;
      case 'mail': openApp(AppId.MAIL); break;
      case 'file':
        if (p.isDir) openApp(AppId.EXPLORER, false, undefined, { openPath: p.path });
        else void SystemBridge.executeCommand(`xdg-open ${shellQuote(p.path)} >/dev/null 2>&1 &`).catch(() => {});
        break;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.preventDefault(); close(); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); selected = moveSelection(selected, 1, flat.length); scrollSel(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); selected = moveSelection(selected, -1, flat.length); scrollSel(); }
    else if (e.key === 'Enter') { e.preventDefault(); void activate(flat[selected]); }
  }
  async function scrollSel() { await tick(); document.querySelector('[data-spot-selected="true"]')?.scrollIntoView({ block: 'nearest' }); }

  const iconFor = (item: ResultItem) => (item.kind === 'file' && (item.payload as { isDir?: boolean } | undefined)?.isDir ? Folder : icons[item.kind]);
  const icons = { calc: Calculator, app: AppWindow, setting: SettingsIcon, file: FileIcon, event: CalendarDays, mail: Mail, action: Copy } as const;
  const sectionLabel: Record<string, string> = { calc: 'spotlight.calc', app: 'spotlight.apps', setting: 'spotlight.settings', file: 'spotlight.files', event: 'spotlight.events', mail: 'spotlight.mail', action: 'spotlight.actions' };
</script>

{#if isOpen}
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
  <div class="fixed inset-0 flex items-start justify-center pt-[12vh] bg-black/45 backdrop-blur-sm" style="z-index:{zIndex}" on:click|self={close}>
    <div class="w-[min(680px,92vw)] rounded-2xl bg-slate-900/95 border border-white/10 shadow-2xl overflow-hidden" role="dialog" aria-modal="true" aria-label={$t('spotlight.title')}>
      <div class="flex items-center gap-3 px-4 py-3 border-b border-white/10">
        <Search size={18} class="text-slate-400 shrink-0" />
        <input bind:this={input} bind:value={query} on:input={onInput} on:keydown={onKey} placeholder={$t('spotlight.placeholder')}
          class="flex-1 bg-transparent outline-none text-lg text-white placeholder:text-slate-500" autocomplete="off" spellcheck="false" role="combobox" aria-expanded="true" aria-controls="spotlight-results" />
        {#if loading}<Loader2 size={16} class="animate-spin text-slate-400" />{/if}
      </div>
      <div id="spotlight-results" role="listbox" class="max-h-[52vh] overflow-y-auto py-1">
        {#if query.trim() && flat.length === 0 && !loading}
          <div class="px-5 py-6 text-sm text-slate-500">{$t('spotlight.none')}</div>
        {:else if !query.trim()}
          <div class="px-5 py-6 text-sm text-slate-500">{$t('spotlight.hint')}</div>
        {/if}
        {#each sections as sec (sec.kind)}
          <div class="px-4 pt-2 pb-1 text-[11px] uppercase tracking-wide text-slate-500">{$t(sectionLabel[sec.kind])}</div>
          {#each sec.items as item (item.id)}
            {@const i = flat.indexOf(item)}
            <button role="option" aria-selected={i === selected} data-spot-selected={i === selected}
              class="w-full flex items-center gap-3 px-4 py-2 text-left {i === selected ? 'bg-blue-600/30' : 'hover:bg-white/5'}"
              on:click={() => activate(item)} on:mousemove={() => (selected = i)}>
              <svelte:component this={iconFor(item)} size={16} class="text-slate-300 shrink-0" />
              <span class="min-w-0 flex-1"><span class="block text-sm text-white truncate">{item.title}</span>{#if item.subtitle}<span class="block text-[11px] text-slate-500 truncate">{item.subtitle}</span>{/if}</span>
              {#if item.kind === 'calc'}<span class="text-[11px] text-slate-500">{$t('spotlight.copy')}</span>{/if}
            </button>
          {/each}
        {/each}
      </div>
    </div>
  </div>
{/if}
