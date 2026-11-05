<script lang="ts">
  /**
   * Break screen of "Eye & health care" (see stores/eyeCare.ts).
   *  • 20-20-20 look-away  → a small card in the corner (full-screen in strict mode)
   *  • long break          → full-screen, dimmed, with a breathing circle and exercises
   * Skipping depends on the mode: gentle = always, firm = after 5 s, strict = never
   * (emergency exit: Escape ×3, handled by the engine).
   */
  import { fly, fade } from 'svelte/transition';
  import { Eye, Coffee, X, Clock } from 'lucide-svelte';
  import { onDestroy } from 'svelte';
  import { t } from '../stores/language';
  import { activeBreak, eyeCareConfig, finishBreak, postponeBreak } from '../stores/eyeCare';

  interface Props { zIndex?: number; bottomInset?: number }
  let { zIndex = 9500, bottomInset = 0 }: Props = $props();

  const reduceMotion = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  let now = $state(Date.now());
  const timer = setInterval(() => (now = Date.now()), 250);
  onDestroy(() => clearInterval(timer));

  const TIPS = ['eyecare.tip.1', 'eyecare.tip.2', 'eyecare.tip.3', 'eyecare.tip.4', 'eyecare.tip.5', 'eyecare.tip.6'];

  let b = $derived($activeBreak);
  let left = $derived(b ? Math.max(0, Math.ceil((b.endsAt - now) / 1000)) : 0);
  let progress = $derived(b ? Math.min(100, Math.max(0, ((now - b.startedAt) / (b.totalSec * 1000)) * 100)) : 0);
  let canSkip = $derived(!!b && b.skipAfter !== null && now >= b.skipAfter);
  let strict = $derived($eyeCareConfig.mode === 'strict');
  let tip = $derived(b ? TIPS[Math.floor((now - b.startedAt) / 12000) % TIPS.length] : TIPS[0]);
  const fmt = (s: number) => (s >= 60 ? `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}` : `${s}`);
</script>

{#if b && b.fullscreen}
  <div
    class="eyecare-full"
    role="alertdialog"
    aria-live="polite"
    aria-label={$t(b.kind === 'longBreak' ? 'eyecare.break.long_title' : 'eyecare.break.look_title')}
    data-shell-occluder="eyecare"
    style="z-index:{zIndex};"
    in:fade={{ duration: reduceMotion ? 0 : 400 }}
    out:fade={{ duration: reduceMotion ? 0 : 300 }}
  >
    <div class="inner">
      <div class="circle" class:still={reduceMotion} aria-hidden="true">
        {#if b.kind === 'longBreak'}<Coffee size={44} strokeWidth={1.5} />{:else}<Eye size={44} strokeWidth={1.5} />{/if}
      </div>
      <h2>{$t(b.kind === 'longBreak' ? 'eyecare.break.long_title' : 'eyecare.break.look_title')}</h2>
      <p class="lead">{$t(b.kind === 'longBreak' ? 'eyecare.break.long_text' : 'eyecare.break.look_text')}</p>
      <div class="time" aria-label={$t('eyecare.break.remaining', { n: left })}>{fmt(left)}</div>
      <div class="bar" aria-hidden="true"><span style="width:{progress}%"></span></div>
      <p class="tip">{$t(tip)}</p>
      <div class="actions">
        {#if canSkip}
          <button class="btn" onclick={() => postponeBreak(Date.now(), 5)}><Clock size={14} /> {$t('eyecare.break.postpone')}</button>
          <button class="btn ghost" onclick={() => finishBreak(false)}><X size={14} /> {$t('eyecare.break.skip')}</button>
        {:else if strict}
          <span class="hint">{$t('eyecare.break.strict_hint')}</span>
        {:else}
          <span class="hint">{$t('eyecare.break.skip_wait')}</span>
        {/if}
      </div>
    </div>
  </div>
{:else if b}
  <div
    class="eyecare-card"
    role="status"
    aria-live="polite"
    data-shell-occluder="eyecare"
    style="z-index:{zIndex}; bottom:calc({bottomInset}px + 20px);"
    in:fly={{ y: reduceMotion ? 0 : 16, duration: reduceMotion ? 0 : 200 }}
    out:fade={{ duration: reduceMotion ? 0 : 200 }}
  >
    <div class="row">
      <span class="ico" aria-hidden="true"><Eye size={20} /></span>
      <div class="txt">
        <strong>{$t('eyecare.break.look_title')}</strong>
        <span>{$t('eyecare.break.look_text')}</span>
      </div>
      <span class="count">{left}</span>
    </div>
    <div class="bar" aria-hidden="true"><span style="width:{progress}%"></span></div>
    <div class="actions">
      {#if canSkip}
        <button class="btn" onclick={() => postponeBreak(Date.now(), 5)}>{$t('eyecare.break.postpone')}</button>
        <button class="btn ghost" onclick={() => finishBreak(false)}>{$t('eyecare.break.skip')}</button>
      {:else}
        <span class="hint">{$t('eyecare.break.skip_wait')}</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .eyecare-full {
    position: fixed; inset: 0; display: grid; place-items: center; color: #e2e8f0; text-align: center;
    background: radial-gradient(circle at 50% 40%, rgba(15, 23, 42, 0.93), rgba(2, 6, 23, 0.98));
    backdrop-filter: blur(14px) brightness(0.5);
  }
  .inner { max-width: 32rem; padding: 2rem; display: flex; flex-direction: column; align-items: center; gap: 0.9rem; }
  h2 { font-size: 1.6rem; font-weight: 600; margin: 0; }
  .lead { color: #94a3b8; margin: 0; font-size: 0.95rem; }
  .time { font-size: 3.2rem; font-weight: 300; font-variant-numeric: tabular-nums; letter-spacing: 0.02em; }
  .tip { color: #7dd3fc; font-size: 0.9rem; min-height: 2.4em; margin: 0; }
  .circle {
    width: 120px; height: 120px; border-radius: 50%; display: grid; place-items: center; color: #7dd3fc;
    background: radial-gradient(circle, rgba(56, 189, 248, 0.25), rgba(56, 189, 248, 0.05) 70%);
    border: 1px solid rgba(125, 211, 252, 0.35); animation: breathe 8s ease-in-out infinite;
  }
  .circle.still { animation: none; }
  @keyframes breathe { 0%, 100% { transform: scale(0.82); opacity: 0.7; } 50% { transform: scale(1.12); opacity: 1; } }
  .bar { width: 100%; height: 4px; border-radius: 2px; background: rgba(148, 163, 184, 0.25); overflow: hidden; }
  .bar span { display: block; height: 100%; background: #38bdf8; transition: width 0.25s linear; }
  .actions { display: flex; gap: 0.6rem; align-items: center; min-height: 2.2rem; flex-wrap: wrap; justify-content: center; }
  .hint { font-size: 0.75rem; color: #64748b; }
  .btn {
    display: inline-flex; align-items: center; gap: 0.35rem; padding: 0.4rem 0.9rem; border-radius: 0.6rem; font-size: 0.8rem;
    background: #1d4ed8; color: #fff; border: 0; cursor: pointer;
  }
  .btn:hover { background: #2563eb; }
  .btn.ghost { background: rgba(148, 163, 184, 0.18); color: #cbd5e1; }
  .btn.ghost:hover { background: rgba(148, 163, 184, 0.3); }
  .eyecare-card {
    position: fixed; right: 1rem; width: 20rem; padding: 0.9rem; border-radius: 1rem; color: #e2e8f0;
    background: rgba(15, 23, 42, 0.96); border: 1px solid rgba(125, 211, 252, 0.3); box-shadow: 0 18px 40px rgba(0, 0, 0, 0.45);
    display: flex; flex-direction: column; gap: 0.6rem;
  }
  .row { display: flex; align-items: center; gap: 0.7rem; }
  .ico { color: #7dd3fc; }
  .txt { display: flex; flex-direction: column; flex: 1; font-size: 0.78rem; color: #94a3b8; gap: 0.1rem; }
  .txt strong { color: #f1f5f9; font-size: 0.88rem; font-weight: 600; }
  .count { font-size: 1.6rem; font-weight: 300; font-variant-numeric: tabular-nums; color: #7dd3fc; }
</style>
