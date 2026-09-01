<script lang="ts">
  // Same rationale as Blue-Calendar-App/BlueCalendarAppLazy.svelte: keeps
  // BlueTasksApp.svelte's code out of the main startup bundle, loaded
  // only the first time a Blue Tasks window actually opens.
  let component: any = null;
  let failed = false;

  import('./BlueTasksApp.svelte')
    .then((mod) => (component = mod.default))
    .catch(() => (failed = true));
</script>

{#if failed}
  <div class="flex items-center justify-center h-full text-red-400 text-sm">Failed to load Blue Tasks.</div>
{:else if component}
  <svelte:component this={component} {...$$restProps} />
{:else}
  <div class="flex items-center justify-center h-full bg-slate-900">
    <div class="w-6 h-6 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
  </div>
{/if}
