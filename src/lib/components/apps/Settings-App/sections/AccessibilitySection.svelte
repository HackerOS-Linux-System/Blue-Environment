<script lang="ts">
  import { onMount } from 'svelte';
  import { Eye, Type, Zap, ZoomIn, Ear, Keyboard, RotateCcw, Loader2 } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { a11y, updateA11y, resetA11y, announce, TEXT_SCALE_MIN, TEXT_SCALE_MAX, MAGNIFIER_MIN, MAGNIFIER_MAX } from '../../../../stores/accessibility';

  interface ScreenReaderStatus { installed: boolean; running: boolean; speechAvailable: boolean }

  let sr: ScreenReaderStatus | null = null;
  let srBusy = false;
  let srError = '';

  const call = <T,>(cmd: string, args: Record<string, unknown> = {}) => SystemBridge.invokeCommand<T>(cmd, args);

  onMount(refresh);
  async function refresh() { sr = await call<ScreenReaderStatus>('a11y_screen_reader_status').catch(() => null); }

  async function toggleReader(on: boolean) {
    srBusy = true; srError = '';
    try { await call('a11y_screen_reader_set', { enabled: on }); await new Promise((r) => setTimeout(r, 700)); }
    catch (e: any) { srError = String(e?.message ?? e); }
    srBusy = false; await refresh();
  }

  const pct = (n: number) => `${Math.round(n * 100)}%`;
  function testAnnounce() {
    const msg = $t('a11y.test_message');
    announce(msg);
    if (!sr?.running && $a11y.speakAnnouncements) void call('a11y_speak', { text: msg }).catch(() => {});
  }
</script>

<div class="space-y-5 max-w-2xl">
  <p class="text-sm text-slate-400">{$t('a11y.subtitle')}</p>

  <!-- Vision -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-4">
    <h3 class="flex items-center gap-2 text-white font-medium"><Eye size={16} class="text-blue-400" /> {$t('a11y.vision')}</h3>

    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('a11y.high_contrast')}<span class="block text-xs text-slate-500">{$t('a11y.high_contrast_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={$a11y.highContrast} on:change={(e) => updateA11y({ highContrast: e.currentTarget.checked })} />
    </label>

    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('a11y.strong_focus')}<span class="block text-xs text-slate-500">{$t('a11y.strong_focus_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={$a11y.strongFocus} on:change={(e) => updateA11y({ strongFocus: e.currentTarget.checked })} />
    </label>

    <div>
      <div class="flex items-center justify-between text-sm text-slate-200 mb-1"><span class="flex items-center gap-2"><Type size={14} /> {$t('a11y.text_size')}</span><span class="tabular-nums text-slate-400">{pct($a11y.textScale)}</span></div>
      <input type="range" class="w-full" min={TEXT_SCALE_MIN} max={TEXT_SCALE_MAX} step="0.05" value={$a11y.textScale} aria-label={$t('a11y.text_size')}
        on:input={(e) => updateA11y({ textScale: Number(e.currentTarget.value) })} />
      <p class="mt-2 text-slate-300" style="font-size:1rem">{$t('a11y.text_preview')}</p>
    </div>

    <div class="border-t border-white/5 pt-4">
      <label class="flex items-center justify-between gap-4">
        <span class="text-sm text-slate-200 flex items-center gap-2"><ZoomIn size={14} /> {$t('a11y.magnifier')}<span class="block text-xs text-slate-500">{$t('a11y.magnifier_hint')}</span></span>
        <input type="checkbox" class="w-5 h-5" checked={$a11y.magnifierEnabled} on:change={(e) => updateA11y({ magnifierEnabled: e.currentTarget.checked })} />
      </label>
      {#if $a11y.magnifierEnabled}
        <div class="mt-3">
          <div class="flex justify-between text-xs text-slate-400 mb-1"><span>{$t('a11y.magnifier_zoom')}</span><span class="tabular-nums">{$a11y.magnifierZoom.toFixed(1)}×</span></div>
          <input type="range" class="w-full" min={MAGNIFIER_MIN} max={MAGNIFIER_MAX} step="0.5" value={$a11y.magnifierZoom} aria-label={$t('a11y.magnifier_zoom')} on:input={(e) => updateA11y({ magnifierZoom: Number(e.currentTarget.value) })} />
        </div>
      {/if}
      <p class="mt-2 text-[11px] text-slate-500 flex items-center gap-1.5"><Keyboard size={12} /> {$t('a11y.magnifier_keys')}</p>
    </div>
  </section>

  <!-- Motion -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5">
    <h3 class="flex items-center gap-2 text-white font-medium mb-3"><Zap size={16} class="text-blue-400" /> {$t('a11y.motion')}</h3>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('a11y.reduce_motion')}<span class="block text-xs text-slate-500">{$t('a11y.reduce_motion_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={$a11y.reduceMotion} on:change={(e) => updateA11y({ reduceMotion: e.currentTarget.checked })} />
    </label>
  </section>

  <!-- Screen reader -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="flex items-center gap-2 text-white font-medium"><Ear size={16} class="text-blue-400" /> {$t('a11y.screen_reader')}</h3>
    {#if !sr}
      <Loader2 size={16} class="animate-spin text-slate-400" />
    {:else if !sr.installed}
      <p class="text-sm text-amber-300">{$t('a11y.orca_missing')}</p>
      <code class="block bg-black/40 rounded px-3 py-2 text-xs text-slate-200">sudo apt install orca</code>
    {:else}
      <label class="flex items-center justify-between gap-4">
        <span class="text-sm text-slate-200">{$t('a11y.orca')}<span class="block text-xs text-slate-500">{sr.running ? $t('a11y.orca_running') : $t('a11y.orca_stopped')}</span></span>
        <span class="flex items-center gap-2">{#if srBusy}<Loader2 size={14} class="animate-spin" />{/if}<input type="checkbox" class="w-5 h-5" checked={sr.running} disabled={srBusy} on:change={(e) => toggleReader(e.currentTarget.checked)} /></span>
      </label>
    {/if}
    {#if srError}<p class="text-sm text-red-400 break-words" role="alert">{srError}</p>{/if}

    <label class="flex items-center justify-between gap-4 {sr?.speechAvailable ? '' : 'opacity-50'}">
      <span class="text-sm text-slate-200">{$t('a11y.speak')}<span class="block text-xs text-slate-500">{sr?.speechAvailable ? $t('a11y.speak_hint') : $t('a11y.speak_missing')}</span></span>
      <input type="checkbox" class="w-5 h-5" disabled={!sr?.speechAvailable} checked={$a11y.speakAnnouncements} on:change={(e) => updateA11y({ speakAnnouncements: e.currentTarget.checked })} />
    </label>
    <button on:click={testAnnounce} class="text-xs px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600">{$t('a11y.test')}</button>
    <p class="text-[11px] text-slate-500">{$t('a11y.orca_note')}</p>
  </section>

  <button on:click={resetA11y} class="flex items-center gap-2 text-xs px-3 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300"><RotateCcw size={13} /> {$t('a11y.reset')}</button>
</div>
