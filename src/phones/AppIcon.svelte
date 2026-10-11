<script lang="ts">
  /** An app icon: either a Svelte (lucide) component or an image URL. */
  export let icon: any;
  export let size = 28;
  export let title = '';
</script>

{#if typeof icon === 'string'}
  <img src={icon} alt={title} width={size} height={size} class="object-contain" draggable="false" />
{:else if icon}
  <svelte:component this={icon} {size} aria-hidden="true" />
{/if}
