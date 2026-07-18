<script lang="ts">
  import { AlertTriangle, RefreshCw, X } from 'lucide-svelte';
  import { onMount, onDestroy } from 'svelte';
  import type { ComponentType } from 'svelte';

  export let component: ComponentType;
  export let props: Record<string, unknown> = {};
  export let appTitle = 'App';
  export let onClose: (() => void) | undefined = undefined;

  let hasError = false;
  let errorMessage = '';
  let mountKey = 0;

  function reportError(message: string) {
    hasError = true;
    errorMessage = message;
    window.dispatchEvent(
      new CustomEvent('blue:show-toast', {
        detail: { id: Date.now().toString(), title: `${appTitle} crashed`, message },
      })
    );
  }

  function onAppError(e: Event) {
    const detail = (e as CustomEvent).detail;
    reportError(detail?.message ?? 'Unknown error');
  }

  onMount(() => window.addEventListener('blue:app-error', onAppError));
  onDestroy(() => window.removeEventListener('blue:app-error', onAppError));

  function restart() {
    hasError = false;
    errorMessage = '';
    mountKey += 1; // forces <svelte:component> to remount via {#key}
  }
</script>

{#if hasError}
  <div class="flex flex-col items-center justify-center h-full bg-slate-900 text-white p-6">
    <div class="max-w-md w-full">
      <div class="flex items-center gap-3 mb-4">
        <div class="w-10 h-10 bg-red-500/20 rounded-full flex items-center justify-center">
          <AlertTriangle size={20} class="text-red-400" />
        </div>
        <div>
          <h2 class="font-bold">{appTitle} stopped working</h2>
          <p class="text-xs text-slate-400">The application crashed unexpectedly</p>
        </div>
      </div>
      <div class="bg-red-500/10 border border-red-500/20 rounded-xl p-3 mb-4">
        <p class="text-red-300 text-sm font-mono break-all">{errorMessage}</p>
      </div>
      <div class="flex gap-2 mb-4">
        <button on:click={restart} class="flex-1 flex items-center justify-center gap-2 bg-blue-600 hover:bg-blue-500 px-4 py-2 rounded-lg text-sm">
          <RefreshCw size={14} /> Restart App
        </button>
        {#if onClose}
          <button on:click={onClose} class="flex items-center gap-2 bg-slate-700 hover:bg-slate-600 px-4 py-2 rounded-lg text-sm">
            <X size={14} /> Close
          </button>
        {/if}
      </div>
      <p class="text-xs text-slate-600 text-center">Only this app crashed — your desktop is fine.</p>
    </div>
  </div>
{:else}
  {#key mountKey}
    <svelte:component this={component} {...props} on:error={(e) => reportError(e.detail?.message ?? 'Unknown error')} />
  {/key}
{/if}
