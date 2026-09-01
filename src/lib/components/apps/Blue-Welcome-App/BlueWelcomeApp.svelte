<script lang="ts">
  // Blue Welcome — new app, first-run wizard (the "something like KDE
  // has" ask). Auto-opened once by App.svelte on first boot (see
  // welcome.ts + App.svelte's onMount); also stays normally launchable
  // from the Start Menu afterwards, which doubles as "show the welcome
  // tour again" without needing a dedicated Settings toggle.
  import { Check, Globe, Rss, ListChecks, CalendarDays, Sparkles, ArrowRight, ArrowLeft, Sun, Moon } from 'lucide-svelte';
  import { language, t, setLanguage, SUPPORTED_LANGUAGES } from '../../../stores/language';
  import { configStore } from '../../../utils/configStore';
  import { closeWindow } from '../../../stores/windowManager';
  import { markWelcomeCompleted } from './welcome';

  export let windowId: string;

  let step = 0;
  const totalSteps = 4;
  let theme: 'dark' | 'light' = (configStore.get().theme as 'dark' | 'light') ?? 'dark';

  const features = [
    { icon: Globe, color: 'text-blue-400', bg: 'bg-blue-500/15', titleKey: 'welcome.feature.web.title', descKey: 'welcome.feature.web.desc' },
    { icon: ListChecks, color: 'text-emerald-400', bg: 'bg-emerald-500/15', titleKey: 'welcome.feature.tasks.title', descKey: 'welcome.feature.tasks.desc' },
    { icon: Rss, color: 'text-orange-400', bg: 'bg-orange-500/15', titleKey: 'welcome.feature.notif.title', descKey: 'welcome.feature.notif.desc' },
    { icon: CalendarDays, color: 'text-purple-400', bg: 'bg-purple-500/15', titleKey: 'welcome.feature.calendar.title', descKey: 'welcome.feature.calendar.desc' },
  ];

  async function selectTheme(t: 'dark' | 'light') {
    theme = t;
    await configStore.save({ theme: t });
  }

  function next() { if (step < totalSteps - 1) step += 1; }
  function back() { if (step > 0) step -= 1; }

  function finish() {
    markWelcomeCompleted();
    closeWindow(windowId);
  }
</script>

<div class="flex flex-col h-full bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950 text-white overflow-hidden">
  <!-- Step dots -->
  <div class="flex items-center justify-center gap-2 pt-6 shrink-0">
    {#each Array(totalSteps) as _, i}
      <div class="h-1.5 rounded-full transition-all {i === step ? 'w-6 bg-blue-500' : i < step ? 'w-1.5 bg-blue-500/60' : 'w-1.5 bg-white/10'}" />
    {/each}
  </div>

  <div class="flex-1 overflow-y-auto flex flex-col items-center justify-center px-10 py-8">
    {#if step === 0}
      <!-- Welcome hero -->
      <div class="flex flex-col items-center text-center gap-5 max-w-md">
        <div class="w-20 h-20 rounded-3xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-2xl shadow-blue-500/30">
          <Sparkles size={36} class="text-white" />
        </div>
        <div>
          <h1 class="text-3xl font-bold mb-2">{$t('welcome.title')}</h1>
          <p class="text-slate-400">{$t('welcome.subtitle')}</p>
        </div>
      </div>
    {:else if step === 1}
      <!-- Language -->
      <div class="w-full max-w-lg flex flex-col gap-4">
        <div class="text-center mb-2">
          <h2 class="text-xl font-bold">{$t('welcome.choose_language')}</h2>
          <p class="text-slate-400 text-sm mt-1">{$t('welcome.choose_language_desc')}</p>
        </div>
        <div class="grid grid-cols-2 gap-2 max-h-64 overflow-y-auto pr-1">
          {#each SUPPORTED_LANGUAGES as lang (lang.code)}
            <button on:click={() => setLanguage(lang.code)}
              class="flex items-center justify-between px-4 py-3 rounded-xl border transition-all {$language === lang.code ? 'bg-blue-600/15 border-blue-500/40' : 'bg-slate-900/40 border-white/5 hover:bg-white/5'}">
              <span class="flex items-center gap-3">
                <span class="text-xs font-mono bg-slate-700 px-1.5 py-0.5 rounded min-w-[28px] text-center">{lang.flag}</span>
                <span class="text-sm text-left">{lang.nativeName}</span>
              </span>
              {#if $language === lang.code}<Check size={14} class="text-blue-400" />{/if}
            </button>
          {/each}
        </div>
      </div>
    {:else if step === 2}
      <!-- Theme -->
      <div class="w-full max-w-md flex flex-col gap-4">
        <div class="text-center mb-2">
          <h2 class="text-xl font-bold">{$t('welcome.choose_theme')}</h2>
          <p class="text-slate-400 text-sm mt-1">{$t('welcome.choose_theme_desc')}</p>
        </div>
        <div class="grid grid-cols-2 gap-4">
          <button on:click={() => selectTheme('dark')} class="flex flex-col items-center gap-3 p-5 rounded-2xl border transition-all {theme === 'dark' ? 'border-blue-500/50 bg-blue-500/10' : 'border-white/5 bg-slate-900/40 hover:bg-white/5'}">
            <div class="w-full h-16 rounded-lg bg-slate-950 border border-white/10 flex items-center justify-center"><Moon size={20} class="text-blue-300" /></div>
            <span class="text-sm font-medium">{$t('welcome.theme_dark')}</span>
          </button>
          <button on:click={() => selectTheme('light')} class="flex flex-col items-center gap-3 p-5 rounded-2xl border transition-all {theme === 'light' ? 'border-blue-500/50 bg-blue-500/10' : 'border-white/5 bg-slate-900/40 hover:bg-white/5'}">
            <div class="w-full h-16 rounded-lg bg-slate-100 border border-white/10 flex items-center justify-center"><Sun size={20} class="text-amber-500" /></div>
            <span class="text-sm font-medium">{$t('welcome.theme_light')}</span>
          </button>
        </div>
      </div>
    {:else}
      <!-- Feature tour -->
      <div class="w-full max-w-lg flex flex-col gap-4">
        <div class="text-center mb-2">
          <h2 class="text-xl font-bold">{$t('welcome.tour_title')}</h2>
          <p class="text-slate-400 text-sm mt-1">{$t('welcome.tour_desc')}</p>
        </div>
        <div class="grid grid-cols-2 gap-3">
          {#each features as f}
            <div class="flex flex-col gap-2 p-4 rounded-xl bg-slate-900/40 border border-white/5">
              <div class="w-9 h-9 rounded-lg {f.bg} flex items-center justify-center">
                <svelte:component this={f.icon} size={16} class={f.color} />
              </div>
              <div class="text-sm font-medium">{$t(f.titleKey)}</div>
              <div class="text-xs text-slate-500">{$t(f.descKey)}</div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="flex items-center justify-between px-8 py-5 border-t border-white/5 shrink-0">
    {#if step > 0}
      <button on:click={back} class="flex items-center gap-1.5 text-sm text-slate-400 hover:text-white px-3 py-2"><ArrowLeft size={14} /> {$t('welcome.back')}</button>
    {:else}
      <div />
    {/if}
    {#if step < totalSteps - 1}
      <button on:click={next} class="flex items-center gap-1.5 text-sm px-5 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-500 font-medium">{$t('welcome.next')} <ArrowRight size={14} /></button>
    {:else}
      <button on:click={finish} class="flex items-center gap-1.5 text-sm px-5 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-500 font-medium">{$t('welcome.finish')} <Check size={14} /></button>
    {/if}
  </div>
</div>
