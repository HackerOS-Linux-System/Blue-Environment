<script lang="ts">
  /**
   * On-screen ("virtual") keyboard, toggled by Settings > "On-screen
   * Keyboard" (`config.onscreenKeyboardEnabled`, opt-in, default off —
   * see UserConfig's doc in systemBridge.ts).
   *
   * IMPORTANT SCOPE LIMITATION, read before wiring this up further:
   * this component types into whatever `<input>`/`<textarea>`/
   * `contenteditable` element currently has DOM focus **inside
   * Blue-Environment's own webview** — it works for the shell's own
   * search boxes, Notepad, Blue Docs, address bars, etc. It still
   * CANNOT type into a separate native application window (an
   * Xwayland game, a terminal emulator running outside this webview, a
   * native GTK/Qt app).
   *
   * Status as of this comment: the *compositor* side of real injection
   * now exists — HackerOS-Comp implements `zwp_virtual_keyboard_manager_v1`,
   * gated by the same `is_trusted_client()` check used for
   * screencopy/foreign-toplevel/ext-workspace (see
   * `state/mod.rs`'s `virtual_keyboard_manager_state` and its doc
   * comment). What's still missing is the *shell* side: Blue-Environment
   * itself has no `wayland-client` dependency and isn't a Wayland
   * client capable of speaking that protocol yet — it's a Tauri/webview
   * app, not something that opens its own raw socket to the
   * compositor. Closing that gap needs, roughly:
   *   1. A `wayland-client` + `wayland-protocols-misc` (or a small
   *      hand-rolled protocol binding) + `xkbcommon` dependency added
   *      to src-tauri.
   *   2. A minimal client that connects to `$WAYLAND_DISPLAY`, binds
   *      `zwp_virtual_keyboard_manager_v1` (this will only succeed if
   *      the shell's own binary is on the compositor's trusted-basename
   *      allowlist — it already is, "blue-environment" is in
   *      `TRUSTED_CLIENT_BASENAMES`), uploads a minimal keymap once,
   *      and exposes a Tauri command such as
   *      `compositor_send_key(keysym: u32, pressed: bool)`.
   *   3. This component's `pressKey()`/`backspace()`/`enter()` calling
   *      that command as the primary path, falling back to the current
   *      DOM-focus approach when not running under a trusted native
   *      session (e.g. a browser-only preview build).
   * Until that lands, this stays a DOM-only keyboard.
   */
  import { onMount, onDestroy } from 'svelte';
  import { ChevronDown, Delete, ArrowBigUp, Globe } from 'lucide-svelte';

  export let visible = false;

  let shift = false;
  let capsLock = false;

  const ROWS: string[][] = [
    ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
    ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
    ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
    ['z', 'x', 'c', 'v', 'b', 'n', 'm'],
  ];

  function isTextEditable(el: Element | null): el is HTMLInputElement | HTMLTextAreaElement | HTMLElement {
    if (!el) return false;
    const tag = el.tagName;
    if (tag === 'TEXTAREA') return true;
    if (tag === 'INPUT') {
      const type = (el as HTMLInputElement).type;
      return ['text', 'search', 'email', 'url', 'tel', 'password', 'number'].includes(type);
    }
    return (el as HTMLElement).isContentEditable === true;
  }

  // Tracks the last text-editable element that had focus, since a
  // pointerdown on one of this component's own on-screen buttons steals
  // DOM focus away from it before the click handler runs — without
  // this, every key press would insert into nothing.
  let target: HTMLInputElement | HTMLTextAreaElement | HTMLElement | null = null;

  function onFocusIn(e: FocusEvent) {
    const el = e.target as Element | null;
    if (isTextEditable(el)) target = el as any;
  }

  function insertIntoTarget(text: string) {
    if (!target) return;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      const start = target.selectionStart ?? target.value.length;
      const end = target.selectionEnd ?? target.value.length;
      target.value = target.value.slice(0, start) + text + target.value.slice(end);
      target.selectionStart = target.selectionEnd = start + text.length;
      // Real `input` events, not just a value mutation — Svelte
      // `bind:value` and any framework listening for typed input both
      // depend on this event actually firing, since setting `.value`
      // directly bypasses whatever native event would normally follow
      // a keypress.
      target.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (target.isContentEditable) {
      // Best-effort: execCommand is deprecated but still the only
      // widely-supported way to insert text at the caret in a
      // contenteditable while preserving undo history — a manual
      // Range/Node splice would need to be reimplemented per editor
      // (Blue Docs, Notepad's rich mode, etc.) to behave the same way.
      document.execCommand('insertText', false, text);
    }
    target.focus();
  }

  function backspace() {
    if (!target) return;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      const start = target.selectionStart ?? target.value.length;
      const end = target.selectionEnd ?? target.value.length;
      if (start === end && start > 0) {
        target.value = target.value.slice(0, start - 1) + target.value.slice(start);
        target.selectionStart = target.selectionEnd = start - 1;
      } else {
        target.value = target.value.slice(0, start) + target.value.slice(end);
        target.selectionStart = target.selectionEnd = start;
      }
      target.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (target.isContentEditable) {
      document.execCommand('delete', false);
    }
    target.focus();
  }

  function enter() {
    if (!target) return;
    if (target instanceof HTMLTextAreaElement) {
      insertIntoTarget('\n');
    } else {
      // Plain <input> and most contenteditable single-line fields treat
      // Enter as "submit"/"confirm", not a literal newline — dispatch a
      // real Enter keydown/keyup pair so a form's own submit handler
      // (which listens for the key, not for inserted text) still fires.
      for (const type of ['keydown', 'keyup'] as const) {
        target.dispatchEvent(new KeyboardEvent(type, { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }));
      }
    }
  }

  function keyLabel(k: string): string {
    return shift || capsLock ? k.toUpperCase() : k;
  }

  function pressKey(k: string) {
    insertIntoTarget(keyLabel(k));
    if (shift && !capsLock) shift = false; // one-shot shift, like a phone keyboard
  }

  onMount(() => document.addEventListener('focusin', onFocusIn));
  onDestroy(() => document.removeEventListener('focusin', onFocusIn));
</script>

{#if visible}
  <div class="fixed bottom-0 left-0 right-0 z-[9999] bg-slate-900/95 backdrop-blur-xl border-t border-white/10 px-3 pt-2 pb-3 select-none" data-testid="onscreen-keyboard">
    <div class="flex justify-end mb-1">
      <button class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400" on:click={() => (visible = false)} title="Hide keyboard" aria-label="Hide keyboard">
        <ChevronDown size={16} />
      </button>
    </div>
    <div class="max-w-3xl mx-auto space-y-1.5">
      {#each ROWS as row, i}
        <div class="flex gap-1.5 justify-center" style={i === 3 ? 'padding: 0 2.5rem;' : ''}>
          {#each row as k}
            <button
              class="flex-1 min-w-0 h-10 rounded-lg bg-white/5 hover:bg-white/10 active:bg-blue-600 text-white text-sm font-medium transition-colors"
              on:mousedown|preventDefault={() => pressKey(k)}>
              {keyLabel(k)}
            </button>
          {/each}
          {#if i === 3}
            <button
              class="w-16 h-10 rounded-lg bg-white/5 hover:bg-white/10 active:bg-blue-600 flex items-center justify-center transition-colors"
              on:mousedown|preventDefault={backspace} title="Backspace" aria-label="Backspace">
              <Delete size={16} class="text-white" />
            </button>
          {/if}
        </div>
      {/each}
      <div class="flex gap-1.5">
        <button
          class="w-16 h-10 rounded-lg flex items-center justify-center transition-colors {shift || capsLock ? 'bg-blue-600' : 'bg-white/5 hover:bg-white/10'}"
          on:mousedown|preventDefault={() => (shift = !shift)}
          on:dblclick|preventDefault={() => { capsLock = !capsLock; shift = false; }}
          title="Shift (double-tap for Caps Lock)" aria-label="Shift">
          <ArrowBigUp size={16} class="text-white" />
        </button>
        <button
          class="flex-1 h-10 rounded-lg bg-white/5 hover:bg-white/10 active:bg-blue-600 text-white text-sm transition-colors"
          on:mousedown|preventDefault={() => pressKey(' ')}>
          {' '}
        </button>
        <button
          class="w-20 h-10 rounded-lg bg-white/5 hover:bg-white/10 active:bg-blue-600 text-white text-sm font-medium transition-colors"
          on:mousedown|preventDefault={enter}>
          ↵
        </button>
      </div>
    </div>
  </div>
{/if}
