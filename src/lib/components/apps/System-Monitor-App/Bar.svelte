<script lang="ts">
  interface Props {
    value: number;
    max?: number;
    color?: string;
    height?: number;
  }

  let {
    value,
    max = 100,
    color = '#3b82f6',
    height = 5
  }: Props = $props();

  let pctVal = $derived(Math.min(100, Math.max(0, (value / max) * 100)));
  let c = $derived(pctVal > 90 ? '#ef4444' : pctVal > 70 ? '#f59e0b' : color);
</script>

<div style="height:{height}px; background:rgba(255,255,255,0.06); border-radius:{height / 2}px; overflow:hidden;">
  <div style="width:{pctVal}%; height:100%; background:{c}; border-radius:{height / 2}px; transition:width 0.4s ease;"></div>
</div>
