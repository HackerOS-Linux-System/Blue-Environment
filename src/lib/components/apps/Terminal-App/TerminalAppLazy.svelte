<script lang="ts">
  // See constants.ts: registered as Terminal's `component` instead of
  // TerminalApp.svelte directly, so its xterm-based wrapper only loads
  // as a separate chunk the first time a Terminal window is opened.
  let component: any = null;
  let failed = false;

  import('./TerminalApp.svelte')
    .then((mod) => (component = mod.default))
    .catch(() => (failed = true));
</script>

{#if failed}
  <div class="flex items-center justify-center h-full text-red-400 text-sm">Failed to load Terminal.</div>
{:else if component}
  <svelte:component this={component} {...$$restProps} />
{:else}
  <div class="flex items-center justify-center h-full bg-slate-900">
    <div class="w-6 h-6 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
  </div>
{/if}
