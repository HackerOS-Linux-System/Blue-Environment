<script lang="ts">
  import { onMount } from 'svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { pushHistory, sparklinePoints, type WidgetInstance } from './widgetModel';
  import { t } from '../stores/language';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { }: Props = $props();

  let cpu = $state<number[]>([]);
  let ram = $state<number[]>([]);
  let net = $state({ rx: 0, tx: 0 });
  let disk = $state({ r: 0, w: 0 });

  async function sample() {
    if (document.hidden) return;
    try {
      const s = await SystemBridge.getSystemStats();
      cpu = pushHistory(cpu, s.cpu);
      ram = pushHistory(ram, s.ram);
      net = { rx: s.netRx, tx: s.netTx };
      disk = { r: s.diskRead, w: s.diskWrite };
    } catch { /* keep last values */ }
  }

  onMount(() => {
    sample();
    const id = setInterval(sample, 2000);
    return () => clearInterval(id);
  });

  const last = (a: number[]) => (a.length ? Math.round(a[a.length - 1]) : 0);
  const rate = (v: number) => (v >= 1 ? `${v.toFixed(1)} MB/s` : `${Math.round(v * 1024)} KB/s`);
</script>

<div class="h-full flex flex-col justify-center gap-2 px-4 text-xs">
  {#each [{ label: 'CPU', data: cpu, color: '#60a5fa' }, { label: 'RAM', data: ram, color: '#34d399' }] as m (m.label)}
    <div>
      <div class="flex justify-between text-slate-300"><span>{m.label}</span><span class="tabular-nums">{last(m.data)}%</span></div>
      <svg viewBox="0 0 100 20" preserveAspectRatio="none" class="w-full h-6">
        <polyline points={sparklinePoints(m.data, 100, 20)} fill="none" stroke={m.color} stroke-width="1.5" vector-effect="non-scaling-stroke" />
      </svg>
    </div>
  {/each}
  <div class="flex justify-between text-[11px] text-slate-400">
    <span>↓ {rate(net.rx)} ↑ {rate(net.tx)}</span>
    <span>{$t('widgets.disk')} {rate(disk.r)}/{rate(disk.w)}</span>
  </div>
</div>
