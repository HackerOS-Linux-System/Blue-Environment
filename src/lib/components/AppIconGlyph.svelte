<script lang="ts">
  export let icon: any;
  export let name: string;
  export let size = 32;

  let failed = false;
  $: isUrl = typeof icon === 'string' && (icon.startsWith('http') || icon.startsWith('file://'));
  $: hue = (name.charCodeAt(0) * 37) % 360;
</script>

{#if typeof icon === 'string' && isUrl && !failed}
  <img src={icon} alt={name} width={size} height={size} class="object-contain" on:error={() => (failed = true)} />
{:else if typeof icon !== 'string' && !failed}
  <svelte:component this={icon} {size} />
{:else}
  <div class="flex items-center justify-center rounded-lg font-bold text-white"
       style="width:{size}px; height:{size}px; background:hsl({hue},60%,40%); font-size:{size * 0.45}px;">
    {name.charAt(0).toUpperCase()}
  </div>
{/if}
