<script lang="ts">
  import { ChevronLeft, ChevronRight, Sparkles } from 'lucide-svelte';
  import { createInstallState } from './installState';
  import type { InstallStep } from './types';

  import WelcomeStep from './steps/WelcomeStep.svelte';
  import LanguageStep from './steps/LanguageStep.svelte';
  import KeyboardStep from './steps/KeyboardStep.svelte';
  import DiskStep from './steps/DiskStep.svelte';
  import AccountStep from './steps/AccountStep.svelte';
  import SummaryStep from './steps/SummaryStep.svelte';
  import InstallingStep from './steps/InstallingStep.svelte';
  import DoneStep from './steps/DoneStep.svelte';
  import ErrorStep from './steps/ErrorStep.svelte';

  const state = createInstallState();
  const { step, config } = state;

  const STEP_ORDER: InstallStep[] = ['welcome', 'language', 'keyboard', 'disk', 'account', 'summary'];
  const STEP_LABELS: Record<string, string> = {
    welcome: 'Welcome', language: 'Language', keyboard: 'Keyboard', disk: 'Disk', account: 'Account', summary: 'Summary',
  };

  $: currentIdx = STEP_ORDER.indexOf($step);
  $: showNav = currentIdx >= 0;
  $: canGoNext =
    $step === 'welcome' ? true :
    $step === 'language' ? true :
    $step === 'keyboard' ? true :
    $step === 'disk' ? !!$config.disk :
    $step === 'account' ? !!$config.username && !!$config.password : false;
</script>

<div class="fixed inset-0 z-[999] flex flex-col bg-slate-950 text-white select-none" style="background:radial-gradient(ellipse at top, #0f1c3f 0%, #020617 70%);">
  <div class="shrink-0 flex items-center gap-2 px-6 py-4 border-b border-white/5">
    <div class="w-7 h-7 rounded-lg bg-gradient-to-br from-blue-500 to-indigo-700 flex items-center justify-center">
      <Sparkles size={14} class="text-white" />
    </div>
    <span class="text-sm font-medium text-slate-300">Blue Installer</span>

    {#if showNav}
      <div class="flex items-center gap-1.5 ml-8">
        {#each STEP_ORDER as s, i (s)}
          <div class="flex items-center gap-1.5">
            <div class="w-6 h-6 rounded-full flex items-center justify-center text-[10px] font-medium transition-colors
              {i < currentIdx ? 'bg-blue-600 text-white' : i === currentIdx ? 'bg-blue-600/20 text-blue-400 border border-blue-500/40' : 'bg-slate-800 text-slate-600'}">
              {i + 1}
            </div>
            <span class="text-[11px] {i === currentIdx ? 'text-slate-300' : 'text-slate-600'} hidden md:inline">{STEP_LABELS[s]}</span>
            {#if i < STEP_ORDER.length - 1}<div class="w-6 h-px bg-slate-800" />{/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="flex-1 flex flex-col overflow-hidden">
    {#if $step === 'welcome'}<WelcomeStep on:next={state.next} />
    {:else if $step === 'language'}<LanguageStep {state} />
    {:else if $step === 'keyboard'}<KeyboardStep {state} />
    {:else if $step === 'disk'}<DiskStep {state} />
    {:else if $step === 'account'}<AccountStep {state} />
    {:else if $step === 'summary'}<SummaryStep {state} />
    {:else if $step === 'installing'}<InstallingStep {state} />
    {:else if $step === 'done'}<DoneStep {state} />
    {:else if $step === 'error'}<ErrorStep {state} />
    {/if}
  </div>

  {#if showNav}
    <div class="shrink-0 flex items-center justify-between px-6 py-4 border-t border-white/5">
      <button on:click={state.back} disabled={currentIdx === 0}
        class="flex items-center gap-1.5 px-4 py-2 rounded-xl text-sm text-slate-400 hover:text-white disabled:opacity-0 transition-colors">
        <ChevronLeft size={15} /> Back
      </button>
      {#if $step !== 'summary'}
        <button on:click={state.next} disabled={!canGoNext}
          class="flex items-center gap-1.5 px-5 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded-xl text-sm text-white font-medium transition-colors">
          Next <ChevronRight size={15} />
        </button>
      {:else}
        <div />
      {/if}
    </div>
  {/if}
</div>
