<script lang="ts">
  export let value: number;
  export let max = 100;
  export let color = '#3b82f6';
  export let height = 5;

  $: pctVal = Math.min(100, Math.max(0, (value / max) * 100));
  $: c = pctVal > 90 ? '#ef4444' : pctVal > 70 ? '#f59e0b' : color;
</script>

<div style="height:{height}px; background:rgba(255,255,255,0.06); border-radius:{height / 2}px; overflow:hidden;">
  <div style="width:{pctVal}%; height:100%; background:{c}; border-radius:{height / 2}px; transition:width 0.4s ease;" />
</div>
