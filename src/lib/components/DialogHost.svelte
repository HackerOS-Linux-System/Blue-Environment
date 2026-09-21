<script lang="ts">
  import { activeDialog, closeDialog } from '../stores/dialog';
  import type { PendingDialog } from '../stores/dialog';
  import { tick } from 'svelte';
  import { Eye, EyeOff } from 'lucide-svelte';
  import { SystemBridge } from '../utils/systemBridge';

  let inputValue = '';
  let inputEl: HTMLInputElement;
  let dialogEl: HTMLDivElement;
  let showSecret = false;
  let openedFor: PendingDialog | null = null;

  // ── Why typing into this prompt could silently do nothing ───────────
  // The blinking caret only proves the *DOM* has focus in the field. On a
  // Wayland compositor, key events go to whichever *client* has keyboard
  // focus — and after clicking a panel/Control Center entry that can still
  // be a native app window (or the shell without the compositor knowing
  // it should take focus back). The caret shows, keys go elsewhere.
  // Three defences, all cheap:
  //  1. ask the toolkit + compositor to give the shell keyboard focus
  //     (`focus_shell`), then re-assert DOM focus afterwards;
  //  2. keep re-asserting DOM focus for the first second (a poll/re-render
  //     elsewhere in the shell must not be able to steal it);
  //  3. the field is a plain text input masked with CSS instead of
  //     `type="password"` — WebKitGTK gives password fields special input
  //     hints/IM handling that some compositors' text-input implementations
  //     handle badly. (A Show/Hide toggle comes for free.)
  // NOTE: deliberately *not* referencing `inputEl` inside the reactive
  // statement — it used to, which re-ran the block (resetting the typed
  // value and re-selecting it) whenever `bind:this` assigned the element.
  $: if ($activeDialog?.kind === 'prompt') openPrompt($activeDialog);
  $: if (!$activeDialog) openedFor = null;

  function focusInput(selectAll: boolean) {
    if (!inputEl) return;
    inputEl.focus({ preventScroll: true });
    if (selectAll) inputEl.select();
  }

  function openPrompt(dialog: PendingDialog) {
    if (openedFor === dialog) return;
    openedFor = dialog;
    inputValue = (dialog as any).options.defaultValue ?? '';
    showSecret = false;
    tick().then(() => focusInput(true));
    SystemBridge.focusShell().finally(() => setTimeout(() => focusInput(false), 60));
    for (const ms of [150, 400, 900]) {
      setTimeout(() => {
        if (openedFor === dialog && inputEl && document.activeElement !== inputEl) focusInput(false);
      }, ms);
    }
  }

  function handleWindowFocus() {
    if ($activeDialog?.kind === 'prompt') focusInput(false);
  }

  // Ordinary typing must never be visible to window-level listeners of
  // whatever app happens to be open behind the dialog (file manager
  // shortcuts, game controls, …). Enter/Escape/Tab still bubble up to this
  // component's own window handler below.
  function handleInputKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === 'Escape' || e.key === 'Tab') return;
    e.stopPropagation();
  }

  $: isSecret = $activeDialog?.kind === 'prompt' && ($activeDialog as any).options.inputType === 'password';

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

<svelte:window on:keydown={handleKeyDown} on:focus={handleWindowFocus} />

{#if $activeDialog}
  {@const dialog = $activeDialog}
  <div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/60" on:mousedown={handleBackdropClick}>
    <div bind:this={dialogEl} class="w-[400px] bg-slate-800 border border-white/10 rounded-2xl shadow-2xl p-5" role="dialog" aria-modal="true">
      <h3 class="text-white font-semibold text-base mb-1">{dialog.options.title}</h3>

      {#if dialog.kind === 'prompt'}
        {#if dialog.options.label}
          <p class="text-slate-400 text-xs mb-3">{dialog.options.label}</p>
        {/if}
        <div class="relative mb-4">
          <input
            bind:this={inputEl}
            bind:value={inputValue}
            type="text"
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            placeholder={dialog.options.placeholder}
            on:keydown={handleInputKeyDown}
            style={isSecret && !showSecret ? '-webkit-text-security: disc;' : ''}
            class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 {isSecret ? 'pr-10' : ''} text-sm text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60"
          />
          {#if isSecret}
            <button type="button" tabindex="-1" on:mousedown|preventDefault on:click={() => { showSecret = !showSecret; focusInput(false); }}
              class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-slate-400 hover:text-white rounded"
              title={showSecret ? 'Hide' : 'Show'}>
              {#if showSecret}<EyeOff size={15} />{:else}<Eye size={15} />{/if}
            </button>
          {/if}
        </div>
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
