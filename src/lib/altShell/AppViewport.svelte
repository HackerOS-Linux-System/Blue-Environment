<script lang="ts">
  /**
   * Full-screen host for running Blue apps, used by the phone and TV shells.
   * Every open (non-minimized) app of the current workspace stays mounted so
   * it keeps its state while another app is in front; only the active one is
   * visible. Nothing is positioned or resized — each app fills the viewport,
   * which is how phone/TV UIs present apps.
   */
  import { windows, activeWindowId, currentWorkspace } from '../stores/windowManager';
  import { APPS } from '../constants';
  import type { AppId } from '../types';
  import ErrorBoundary from '../components/ErrorBoundary.svelte';
  import CommunityAppHost from '../components/apps/Community-App-Host/CommunityAppHost.svelte';

  
  interface Props {
    /**
   * Replaces the component of an app for this shell (appId → component). The TV
   * shell uses it to swap the desktop Settings app for a remote-friendly one.
   * The replacement gets the same props (`windowId`, launch args).
   */
    overrides?: Record<string, any>;
  }

  let { overrides = {} }: Props = $props();

  function componentFor(appId: string) {
    if (overrides[appId]) return overrides[appId];
    if (appId.startsWith('community:')) return CommunityAppHost;
    return APPS[appId as AppId]?.component;
  }

  let mounted = $derived($windows.filter((w) => !w.isMinimized && !w.isExternal && w.workspace === $currentWorkspace));
</script>

{#each mounted as win (win.id)}
  {@const component = componentFor(win.appId)}
  <div
    class="absolute inset-0 overflow-auto theme-bg-primary theme-text-primary {win.id === $activeWindowId ? '' : 'hidden'}"
    data-app-viewport={win.id}
    aria-hidden={win.id === $activeWindowId ? 'false' : 'true'}
  >
    {#if component}
      <ErrorBoundary {component} appTitle={win.title} props={{ windowId: win.id, ...win.launchArgs }} />
    {/if}
  </div>
{/each}
