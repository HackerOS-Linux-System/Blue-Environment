<script lang="ts">
  // See constants.ts: registered as Blue Code's `component` instead of
  // BlueCodeApp.svelte directly, so BlueCodeApp's own code (not just the
  // monaco-editor npm package it already lazy-imports internally) only
  // ever loads as a separate chunk the first time a Blue Code window is
  // actually opened, not on every app startup.
  let component: any = null;
  let failed = false;

  import('./BlueCodeApp.svelte')
    .then((mod) => (component = mod.default))
    .catch(() => (failed = true));
</script>

{#if failed}
  <div class="flex items-center justify-center h-full text-red-400 text-sm">Failed to load Blue Code.</div>
{:else if component}
  <svelte:component this={component} {...$$restProps} />
{:else}
  <div class="flex items-center justify-center h-full bg-slate-900">
    <div class="w-6 h-6 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
  </div>
{/if}
