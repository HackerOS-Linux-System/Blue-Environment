<script lang="ts">
  /**
   * Common frame of the alternative shells: theme styles, wallpaper layer and
   * the global overlays every shell needs (toasts, OSD, dialogs, file picker,
   * transfer progress, on-screen keyboard). The shell-specific UI goes in the
   * default slot.
   */
  import ShellThemeStyle from '../components/ShellThemeStyle.svelte';
  import SystemThemeStyle from '../components/SystemThemeStyle.svelte';
  import ToastContainer from '../components/ToastContainer.svelte';
  import OsdOverlay from '../components/OsdOverlay.svelte';
  import ContextMenu from '../components/ContextMenu.svelte';
  import DialogHost from '../components/DialogHost.svelte';
  import ConflictDialog from '../components/ConflictDialog.svelte';
  import TransferProgress from '../components/TransferProgress.svelte';
  import BlueFilePicker from '../components/BlueFilePicker.svelte';
  import OnscreenKeyboard from '../components/OnscreenKeyboard.svelte';
  import { resolveActiveShellTheme } from '../data/builtinThemes';
  import { SystemBridge, toAssetUrl } from '../utils/systemBridge';

  /** `phones` | `tv` — exposed as `data-alt-shell` for CSS. */
  export let variant: 'phones' | 'tv';
  export let wallpaper = '';
  export let shellThemeId: string | undefined = undefined;
  export let systemThemeId: string | null | undefined = undefined;
  /** Show the on-screen keyboard when a text field gets focus. */
  export let keyboardOnFocus = true;
  export let showTransferProgress = true;
  export let theme = 'dark';

  $: activeShellTheme = resolveActiveShellTheme(shellThemeId);
  $: effectiveWallpaper = activeShellTheme?.wallpaper || wallpaper;

  // Same two-step load as the desktop shell: the asset: protocol first, then a
  // data: URL read through the backend if the webview refuses the file.
  let wallpaperCss = '';
  let token = 0;
  async function resolveWallpaper(path: string) {
    const mine = ++token;
    if (!path) { wallpaperCss = ''; return; }
    const url = toAssetUrl(path);
    const ok = await new Promise<boolean>((res) => {
      const img = new Image();
      img.onload = () => res(true);
      img.onerror = () => res(false);
      img.src = url;
    });
    if (mine !== token) return;
    if (ok) { wallpaperCss = url; return; }
    const data = await SystemBridge.getWallpaperDataUrl(path).catch(() => null);
    if (mine !== token) return;
    wallpaperCss = data ?? '';
  }
  $: resolveWallpaper(effectiveWallpaper);

  let keyboardVisible = false;
  $: if (!keyboardOnFocus) keyboardVisible = false;
  function isTextTarget(el: Element | null): boolean {
    if (!el) return false;
    if (el.tagName === 'TEXTAREA') return true;
    if (el.tagName === 'INPUT') return ['text', 'search', 'email', 'url', 'tel', 'password', 'number'].includes((el as HTMLInputElement).type);
    return (el as HTMLElement).isContentEditable === true;
  }
  function onFocusIn(e: FocusEvent) {
    if (keyboardOnFocus) keyboardVisible = isTextTarget(e.target as Element | null);
  }
</script>

<svelte:window on:focusin={onFocusIn} />

<ShellThemeStyle {shellThemeId} />
<SystemThemeStyle {systemThemeId} />

<div class="fixed inset-0 overflow-hidden theme-bg-primary theme-text-primary" data-alt-shell={variant} data-theme={theme}>
  <div class="absolute pointer-events-none"
    style="inset:-40px; background-size:cover; background-position:center;
           background-image:{wallpaperCss ? `url('${wallpaperCss}'), ` : ''}linear-gradient(135deg, #0b1330 0%, #131d49 50%, #0b1330 100%);
           filter:blur(var(--wallpaper-blur, 0px)) brightness(var(--wallpaper-brightness, 1)) saturate(var(--wallpaper-saturate, 1));" />

  <slot />

  <ToastContainer />
  <OsdOverlay zIndex={900} bottomInset={0} />
  <ContextMenu />
  <DialogHost />
  <ConflictDialog />
  <TransferProgress enabled={showTransferProgress} />
  <BlueFilePicker />
  <OnscreenKeyboard bind:visible={keyboardVisible} />
</div>
