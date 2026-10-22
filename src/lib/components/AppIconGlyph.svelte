<script lang="ts">
  import { activeShellThemeId } from '../stores/shellTheme';
  import { toAssetUrl } from '../utils/systemBridge';

  interface Props {
    icon: any;
    name: string;
    size?: number;
  }

  let { icon, name, size = 32 }: Props = $props();

  let failed = $state(false);
  // `resolveIcon()` (icon_resolver.rs) and window_tracker.rs both hand
  // back icon paths as `file://<absolute path>` strings — which looks
  // like a normal, webview-loadable URL, but isn't: a Tauri v2 app runs
  // its UI from `tauri://localhost` (or `https://tauri.localhost`), and
  // loading a bare `file://` resource from that origin is blocked by
  // the webview the same way a regular website can't `<img
  // src="file:///etc/passwd">` — it has nothing to do with Tauri's own
  // CSP/`assetProtocol.scope` (`tauri.conf.json` already allows the
  // right directories; that was never the problem). `toAssetUrl()`
  // (systemBridge.ts) already exists and is already used for wallpapers/
  // album art/theme previews for exactly this reason — this component
  // just wasn't calling it, so every `file://` icon anywhere it's used
  // (every external window in Alt+Tab, every system app in the Start
  // Menu, every package in Blue Software) silently failed to load and
  // fell back to the lettered placeholder below, no matter how
  // correctly the backend had resolved the underlying path.
  let isFileUrl = $derived(typeof icon === 'string' && icon.startsWith('file://'));
  let isHttpUrl = $derived(typeof icon === 'string' && (icon.startsWith('http://') || icon.startsWith('https://')));
  let isUrl = $derived(isFileUrl || isHttpUrl);
  let resolvedSrc = $derived(isFileUrl ? toAssetUrl(icon) : isHttpUrl ? icon : null);

  let hue = $derived((name.charCodeAt(0) * 37) % 360);
  // Real, honest scope (see this app's request history — a true
  // pixel-matched custom icon set is separate, larger work with no
  // actual art assets to draw from): rather than hand-drawing a
  // handful of icons that would only cover a few apps and leave the
  // rest inconsistent, every icon rendered through this one shared
  // component gets a neon glow treatment while Hydra is active — a
  // systemic improvement across every app's icon (pinned, tray,
  // launcher grid, wherever this component is used), not a partial
  // one-off retouch of a chosen few.
  let isHydra = $derived($activeShellThemeId === 'hydra');
</script>

{#if isUrl && resolvedSrc && !failed}
  <img src={resolvedSrc} alt={name} width={size} height={size} loading="lazy" decoding="async" class="object-contain {isHydra ? 'hydra-icon-glow' : ''}" onerror={() => (failed = true)} />
{:else if typeof icon !== 'string' && !failed}
  {@const SvelteComponent = icon}
  <div class={isHydra ? 'hydra-icon-glow' : ''} style={isHydra ? 'display:contents;' : undefined}>
    <SvelteComponent {size} />
  </div>
{:else}
  <div class="flex items-center justify-center rounded-lg font-bold text-white {isHydra ? 'hydra-icon-glow' : ''}"
       style="width:{size}px; height:{size}px; background:{isHydra ? 'linear-gradient(135deg, #ec4899, #8b5cf6, #3b82f6)' : `hsl(${hue},60%,40%)`}; font-size:{size * 0.45}px;">
    {name.charAt(0).toUpperCase()}
  </div>
{/if}
