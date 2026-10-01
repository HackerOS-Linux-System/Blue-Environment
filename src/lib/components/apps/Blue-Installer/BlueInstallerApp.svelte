<script lang="ts">
  import { ChevronLeft, ChevronRight, Sparkles, Power, MonitorUp } from 'lucide-svelte';
  import { createInstallState } from './installState';
  import { switchToClassicDesktop } from '../../../utils/liveMode';
  import { SystemBridge } from '../../../utils/systemBridge';
  import PowerMenu from '../../PowerMenu.svelte';
  import type { PowerAction } from '../../../types';
  import type { InstallStep } from './types';

  import WelcomeStep from './steps/WelcomeStep.svelte';
  import LanguageStep from './steps/LanguageStep.svelte';
  import KeyboardStep from './steps/KeyboardStep.svelte';
  import TimezoneStep from './steps/TimezoneStep.svelte';
  import DiskStep from './steps/DiskStep.svelte';
  import AccountStep from './steps/AccountStep.svelte';
  import SummaryStep from './steps/SummaryStep.svelte';
  import InstallingStep from './steps/InstallingStep.svelte';
  import DoneStep from './steps/DoneStep.svelte';
  import ErrorStep from './steps/ErrorStep.svelte';

  /** Set by App.svelte while the person is looking at the classic desktop
   * (live mode): the installer stays mounted — so nothing they typed and no
   * running installation is lost — it is just not displayed. */
  export let hidden = false;

  const state = createInstallState();
  const { step, config } = state;

  const STEP_ORDER: InstallStep[] = ['welcome', 'language', 'keyboard', 'timezone', 'disk', 'account', 'summary'];
  const STEP_LABELS: Record<string, string> = {
    welcome: 'Welcome', language: 'Language', keyboard: 'Keyboard', timezone: 'Timezone', disk: 'Disk', account: 'Account', summary: 'Summary',
  };

  let showPowerMenu = false;

  // Never offer to leave / power off while the disk is being written —
  // pulling the plug halfway through an install leaves an unbootable disk.
  $: busy = $step === 'installing';

  function handlePower(e: CustomEvent<PowerAction>) {
    showPowerMenu = false;
    SystemBridge.powerAction(e.detail).catch(() => {});
  }

  $: currentIdx = STEP_ORDER.indexOf($step);
  $: showNav = currentIdx >= 0;
  $: canGoNext =
    $step === 'welcome' ? true :
    $step === 'language' ? true :
    $step === 'keyboard' ? true :
    $step === 'timezone' ? true :
    $step === 'disk' ? !!$config.disk && ($config.diskMode === 'erase' || !state.validatePartitionPlan($config.partitions)) :
    $step === 'account' ? !!$config.username && !!$config.password : false;
</script>

<div class="fixed inset-0 z-[999] flex flex-col bg-slate-950 text-white select-none" style="background:radial-gradient(ellipse at top, #0f1c3f 0%, #020617 70%);" style:display={hidden ? 'none' : null}>
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

    <!-- Live-session actions (this installer only ever exists in live mode). -->
    <div class="ml-auto flex items-center gap-2">
      <button on:click={switchToClassicDesktop} disabled={busy}
        title="Leave the installer and use the classic Blue desktop — you can come back from there"
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-slate-300 bg-white/5 hover:bg-white/10 disabled:opacity-30 disabled:cursor-not-allowed transition-colors">
        <MonitorUp size={14} /> Classic desktop
      </button>
      <button on:click={() => (showPowerMenu = true)} disabled={busy}
        title="Shut down or restart the computer"
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-red-300 bg-red-500/10 hover:bg-red-500/20 disabled:opacity-30 disabled:cursor-not-allowed transition-colors">
        <Power size={14} /> Power
      </button>
    </div>
  </div>

  <div class="flex-1 flex flex-col overflow-hidden">
    {#if $step === 'welcome'}<WelcomeStep on:next={state.next} />
    {:else if $step === 'language'}<LanguageStep {state} />
    {:else if $step === 'keyboard'}<KeyboardStep {state} />
    {:else if $step === 'timezone'}<TimezoneStep {state} />
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

{#if showPowerMenu && !hidden}
  <PowerMenu hideActions={['hibernate']} zIndex={1000} on:action={handlePower} on:close={() => (showPowerMenu = false)} />
{/if}
