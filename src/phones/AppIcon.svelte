<script lang="ts">
  
  interface Props {
    /** An app icon: either a Svelte (lucide) component or an image URL. */
    icon: any;
    size?: number;
    title?: string;
  }

  let { icon, size = 28, title = '' }: Props = $props();
</script>

{#if typeof icon === 'string'}
  <img src={icon} alt={title} width={size} height={size} class="object-contain" draggable="false" />
{:else if icon}
  {@const SvelteComponent = icon}
  <SvelteComponent {size} aria-hidden="true" />
{/if}
