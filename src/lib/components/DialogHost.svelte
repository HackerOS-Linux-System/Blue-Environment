<script lang="ts">
  import { activeDialog, closeDialog } from '../stores/dialog';
  import { tick } from 'svelte';

  let inputValue = '';
  let inputEl: HTMLInputElement;
  let dialogEl: HTMLDivElement;

  $: if ($activeDialog?.kind === 'prompt') {
    inputValue = $activeDialog.options.defaultValue ?? '';
    tick().then(() => { inputEl?.focus(); inputEl?.select(); });
  }

  function close(result: string | boolean | null) {
    if ($activeDialog) closeDialog($activeDialog, result);
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget && $activeDialog) {
      close($activeDialog.kind === 'prompt' ? null : false);
    }
  }

  // Focus trap: Tab/Shift+Tab previously could walk focus straight out
  // of the dialog into whatever's underneath it (a window's own
  // controls, desktop icons, ...) since nothing kept it contained. A
  // modal that lets keyboard focus leave isn't really modal — someone
  // tabbing through the page can end up interacting with background
  // content while the dialog is still visually up front.
  function getFocusable(): HTMLElement[] {
    if (!dialogEl) return [];
    return Array.from(
      dialogEl.querySelectorAll<HTMLElement>('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])')
    ).filter((el) => !el.hasAttribute('disabled') && el.offsetParent !== null);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (!$activeDialog) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      close($activeDialog.kind === 'prompt' ? null : false);
    } else if (e.key === 'Enter') {
      // Only treat Enter as "confirm" when it isn't already being
      // handled by something inside the dialog that wants it for its
      // own purposes (there's nothing like that today, but this keeps
      // the two concerns — submit vs. trap — clearly separate rather
      // than folding Enter-handling into the Tab-trap logic below).
      e.preventDefault();
      if ($activeDialog.kind === 'prompt') close(inputValue.trim());
      else if ($activeDialog.kind === 'alert') close(null);
    } else if (e.key === 'Tab') {
      const focusable = getFocusable();
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (e.shiftKey) {
        if (active === first || !focusable.includes(active as HTMLElement)) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (active === last || !focusable.includes(active as HTMLElement)) {
          e.preventDefault();
          first.focus();
        }
      }
    }
  }
</script>

<svelte:window on:keydown={handleKeyDown} />

{#if $activeDialog}
  {@const dialog = $activeDialog}
  <div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm" on:mousedown={handleBackdropClick}>
    <div bind:this={dialogEl} class="w-[400px] bg-slate-800 border border-white/10 rounded-2xl shadow-2xl p-5" role="dialog" aria-modal="true">
      <h3 class="text-white font-semibold text-base mb-1">{dialog.options.title}</h3>

      {#if dialog.kind === 'prompt'}
        {#if dialog.options.label}
          <p class="text-slate-400 text-xs mb-3">{dialog.options.label}</p>
        {/if}
        {#if dialog.options.inputType === 'password'}
          <input
            bind:this={inputEl}
            bind:value={inputValue}
            type="password"
            placeholder={dialog.options.placeholder}
            class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60 mb-4"
          />
        {:else}
          <input
            bind:this={inputEl}
            bind:value={inputValue}
            type="text"
            placeholder={dialog.options.placeholder}
            class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60 mb-4"
          />
        {/if}
      {/if}

      {#if dialog.kind === 'confirm' || dialog.kind === 'alert'}
        <p class="text-slate-300 text-sm mb-4 mt-2 leading-relaxed">{dialog.options.message}</p>
      {/if}

      <div class="flex justify-end gap-2">
        {#if dialog.kind !== 'alert'}
          <button on:click={() => close(dialog.kind === 'prompt' ? null : false)}
            class="px-3.5 py-1.5 text-sm bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors text-slate-200">
            {dialog.kind === 'confirm' ? dialog.options.cancelLabel || 'Cancel' : 'Cancel'}
          </button>
        {/if}
        <button on:click={() => close(dialog.kind === 'prompt' ? inputValue.trim() : true)}
          class="px-3.5 py-1.5 text-sm rounded-lg transition-colors text-white {dialog.kind === 'confirm' && dialog.options.danger ? 'bg-red-600 hover:bg-red-500' : 'bg-blue-600 hover:bg-blue-500'}">
          {dialog.kind === 'alert' ? dialog.options.confirmLabel || 'OK' : dialog.options.confirmLabel || (dialog.kind === 'prompt' ? 'Create' : 'Confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}
