<script lang="ts">
  // Applies a filesystem theme package (`/usr/share/themes/<id>/`, see
  // src-tauri/src/themes.rs) globally, the same way ShellThemeStyle.svelte
  // applies a built-in `shellThemeId` theme via `[data-shell-theme]` — the
  // two systems are deliberately independent (a filesystem theme package
  // ships its own self-contained CSS scoped under
  // `[data-system-theme='<id>']`, see themes/HDE/styles.css's own doc, so
  // it doesn't need to know anything about `builtinThemes.ts`'s internal
  // CSS variable names). Mounted once near the app root (see App.svelte),
  // not per-window, since the injected `<style>` tag and the
  // `data-system-theme` attribute on `<html>` both need to be global for
  // every window/panel to pick them up.
  import { onMount } from 'svelte';
  import { SystemBridge } from '../utils/systemBridge';

  export let systemThemeId: string | null | undefined;

  let injectedStyleEl: HTMLStyleElement | null = null;
  let currentlyAppliedId: string | null = null;

  async function applyTheme(id: string | null | undefined) {
    if (!id) {
      clearTheme();
      return;
    }
    if (id === currentlyAppliedId) return; // no-op re-run (e.g. unrelated config change)

    const theme = await SystemBridge.loadSystemTheme(id);
    if (!theme) {
      // The theme package disappeared or failed to load (uninstalled,
      // corrupt config.hk, ...) — fall back to no system theme rather
      // than leaving a stale one applied silently.
      clearTheme();
      return;
    }

    if (!injectedStyleEl) {
      injectedStyleEl = document.createElement('style');
      injectedStyleEl.id = 'blue-system-theme-style';
      document.head.appendChild(injectedStyleEl);
    }
    injectedStyleEl.textContent = theme.css;
    document.documentElement.setAttribute('data-system-theme', theme.id);
    currentlyAppliedId = id;
  }

  function clearTheme() {
    document.documentElement.removeAttribute('data-system-theme');
    if (injectedStyleEl) {
      injectedStyleEl.textContent = '';
    }
    currentlyAppliedId = null;
  }

  $: applyTheme(systemThemeId);

  onMount(() => () => {
    // Cleanup on unmount (shell restart, hot-reload during
    // development) — an orphaned `<style>` tag with no owning
    // component would otherwise keep overriding styles forever.
    injectedStyleEl?.remove();
  });
</script>
