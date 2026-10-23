<script lang="ts">
  import type { HistPoint } from './types';
  import Sparkline from './Sparkline.svelte';
  import Bar from './Bar.svelte';

  interface Props {
    icon: any;
    label: string;
    value: string;
    sub?: string | undefined;
    pctVal: number;
    hist: HistPoint[];
    color: string;
  }

  let {
    icon,
    label,
    value,
    sub = undefined,
    pctVal,
    hist,
    color
  }: Props = $props();

  const SvelteComponent = $derived(icon);
</script>

<div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5 flex flex-col gap-3">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-2">
      <SvelteComponent size={15} style="color:{color};" />
      <span class="text-xs text-slate-400">{label}</span>
    </div>
    <span class="text-xs text-slate-600">{Math.round(pctVal)}%</span>
  </div>
  <div class="flex items-end justify-between gap-4">
    <div>
      <div class="text-2xl font-light text-white tabular-nums">{value}</div>
      {#if sub}<div class="text-xs text-slate-600 mt-0.5">{sub}</div>{/if}
    </div>
    <div style="flex:1; max-width:100px;">
      <Sparkline data={hist} {color} h={40} />
    </div>
  </div>
  <Bar value={pctVal} {color} />
</div>
