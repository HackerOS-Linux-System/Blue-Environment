<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    Cpu, MemoryStick, Network, HardDrive, Activity,
    Thermometer, Monitor, ChevronUp, ChevronDown,
    Search, X, RefreshCw, Zap, Wifi, Server,
  } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import Sparkline from './Sparkline.svelte';
  import Bar from './Bar.svelte';
  import StatCard from './StatCard.svelte';
  import CoreGrid from './CoreGrid.svelte';
  import {
    MAX_HISTORY, push, fmtBytes, fmtBps, pct,
    type HistPoint, type MonitorTab, type SortKey,
    type ProcessEntry, type CpuInfo, type MemInfo, type DiskEntry, type NetInterface, type GpuInfo, type TempSensor,
  } from './types';

  function invoke<T>(cmd: string, args?: any): Promise<T> { return SystemBridge.invokeCommand<T>(cmd, args); }

  let tab: MonitorTab = 'overview';

  let cpuHist: HistPoint[] = [];
  let ramHist: HistPoint[] = [];
  let netHist: HistPoint[] = [];
  let diskHist: HistPoint[] = [];
  let gpuHist: HistPoint[] = [];

  let cpu: CpuInfo | null = null;
  let mem: MemInfo | null = null;
  let disks: DiskEntry[] = [];
  let nets: NetInterface[] = [];
  let gpus: GpuInfo[] = [];
  let temps: TempSensor[] = [];
  let procs: ProcessEntry[] = [];

  let sort: SortKey = 'cpu';
  let asc = false;
  let query = '';
  let selPid: number | null = null;
  let killing: number | null = null;
  let interval: ReturnType<typeof setInterval>;

  async function tick() {
    try {
      const [cpuData, memData, diskData, netData, gpuData, tempData, procData] = await Promise.allSettled([
        invoke<CpuInfo>('get_cpu_metrics'),
        invoke<MemInfo>('get_memory_metrics'),
        invoke<DiskEntry[]>('get_disk_metrics'),
        invoke<NetInterface[]>('get_network_metrics'),
        invoke<GpuInfo[]>('get_gpu_metrics'),
        invoke<TempSensor[]>('get_temp_sensors'),
        invoke<ProcessEntry[]>('get_processes'),
      ]);

      if (cpuData.status === 'fulfilled') { cpu = cpuData.value; cpuHist = push(cpuHist, cpuData.value.total_usage); }
      if (memData.status === 'fulfilled') { mem = memData.value; ramHist = push(ramHist, pct(memData.value.used, memData.value.total)); }
      if (diskData.status === 'fulfilled') disks = diskData.value;
      if (netData.status === 'fulfilled') {
        nets = netData.value;
        const totalRx = netData.value.reduce((s, n) => s + n.rx_bps, 0);
        netHist = push(netHist, totalRx / 1024);
      }
      if (gpuData.status === 'fulfilled') {
        gpus = gpuData.value;
        const g = gpuData.value[0];
        if (g?.usage_pct != null) gpuHist = push(gpuHist, g.usage_pct);
      }
      if (tempData.status === 'fulfilled') temps = tempData.value;
      if (procData.status === 'fulfilled') procs = procData.value;
    } catch {}
  }

  onMount(() => { tick(); interval = setInterval(tick, 2000); });
  onDestroy(() => clearInterval(interval));

  async function killProc(pid: number) {
    killing = pid;
    try { await invoke('kill_process', { pid, signal: 15 }); } catch {}
    procs = procs.filter((x) => x.pid !== pid);
    if (selPid === pid) selPid = null;
    killing = null;
  }

  $: sorted = [...procs]
    .filter((p) => !query || p.name.toLowerCase().includes(query.toLowerCase()) || String(p.pid).includes(query))
    .sort((a, b) => {
      let v = 0;
      if (sort === 'cpu') v = a.cpu - b.cpu;
      if (sort === 'memory') v = a.memory - b.memory;
      if (sort === 'name') v = a.name.localeCompare(b.name);
      if (sort === 'pid') v = a.pid - b.pid;
      if (sort === 'user') v = a.user.localeCompare(b.user);
      return asc ? v : -v;
    });

  function sortBy(k: SortKey) {
    if (sort === k) asc = !asc;
    else { sort = k; asc = false; }
  }

  const TABS: { id: MonitorTab; label: string; icon: any }[] = [
    { id: 'overview', label: 'Overview', icon: Activity },
    { id: 'cpu', label: 'CPU', icon: Cpu },
    { id: 'memory', label: 'Memory', icon: MemoryStick },
    { id: 'gpu', label: 'GPU', icon: Monitor },
    { id: 'network', label: 'Network', icon: Network },
    { id: 'disk', label: 'Disk', icon: HardDrive },
    { id: 'temperatures', label: 'Temps', icon: Thermometer },
    { id: 'processes', label: 'Processes', icon: Server },
  ];

  const THS: { k: SortKey; label: string; cls: string }[] = [
    { k: 'pid', label: 'PID', cls: 'w-16' },
    { k: 'name', label: 'Name', cls: '' },
    { k: 'user', label: 'User', cls: 'hidden md:table-cell' },
    { k: 'cpu', label: 'CPU%', cls: 'w-16 text-right' },
    { k: 'memory', label: 'Memory', cls: 'w-24 text-right' },
  ];

  $: memRows = mem ? [
    ['Used', mem.used, '#a855f7'], ['Available', mem.available, '#10b981'], ['Cached', mem.cached, '#3b82f6'],
    ['Buffers', mem.buffers, '#0ea5e9'], ['Free', mem.free, '#22c55e'],
  ] as [string, number, string][] : [];
</script>

<div class="flex flex-col h-full bg-slate-950 text-white overflow-hidden">
  <div class="shrink-0 flex bg-slate-900 border-b border-white/5 overflow-x-auto scrollbar-hide">
    {#each TABS as t (t.id)}
      <button on:click={() => (tab = t.id)} class="flex items-center gap-1.5 px-4 py-3 text-sm shrink-0 border-b-2 transition-colors {tab === t.id ? 'border-blue-500 text-white' : 'border-transparent text-slate-500 hover:text-slate-300'}">
        <svelte:component this={t.icon} size={13} /> {t.label}
      </button>
    {/each}
    <button on:click={tick} class="ml-auto p-3 text-slate-600 hover:text-slate-400" title="Refresh"><RefreshCw size={14} /></button>
  </div>

  <div class="flex-1 overflow-y-auto p-4">
    {#if tab === 'overview'}
      <div class="grid grid-cols-2 gap-3">
        <StatCard icon={Cpu} label="CPU" value="{cpu?.total_usage.toFixed(1) ?? '0'}%"
          sub={cpu ? `${cpu.cores.length} cores · load ${cpu.load_avg_1.toFixed(2)}` : undefined}
          pctVal={cpu?.total_usage ?? 0} hist={cpuHist} color="#3b82f6" />
        <StatCard icon={MemoryStick} label="Memory" value={mem ? fmtBytes(mem.used) : '—'}
          sub={mem ? `${fmtBytes(mem.total)} total` : undefined}
          pctVal={mem ? pct(mem.used, mem.total) : 0} hist={ramHist} color="#a855f7" />
        <StatCard icon={Network} label="Network (RX)" value={fmtBps(nets.reduce((s, n) => s + n.rx_bps, 0))}
          sub={nets.filter((n) => n.connected).map((n) => n.name).join(', ')}
          pctVal={Math.min(100, (nets.reduce((s, n) => s + n.rx_bps, 0) / 1e6) * 5)} hist={netHist} color="#10b981" />
        <StatCard icon={HardDrive} label="Disk" value={disks.length ? fmtBytes(disks[0].used) : '—'}
          sub={disks.length ? `of ${fmtBytes(disks[0].total)} (${disks[0].mount})` : undefined}
          pctVal={disks.length ? pct(disks[0].used, disks[0].total) : 0} hist={diskHist} color="#f59e0b" />
        {#if gpus.length > 0}
          <StatCard icon={Zap} label="GPU — {gpus[0].name.slice(0, 24)}"
            value={gpus[0].usage_pct != null ? `${gpus[0].usage_pct.toFixed(0)}%` : 'N/A'}
            sub={gpus[0].vram_used != null ? `VRAM ${fmtBytes(gpus[0].vram_used)}` : gpus[0].driver}
            pctVal={gpus[0].usage_pct ?? 0} hist={gpuHist} color="#ec4899" />
        {/if}
        {#if temps.length > 0}
          <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
            <div class="flex items-center gap-2 mb-3"><Thermometer size={14} class="text-orange-400" /><span class="text-xs text-slate-400">Temperatures</span></div>
            <div class="space-y-2">
              {#each temps.slice(0, 5) as t, i (i)}
                <div class="flex items-center justify-between">
                  <span class="text-xs text-slate-500 truncate mr-2">{t.label}</span>
                  <span class="text-xs tabular-nums font-mono" style="color:{t.crit && t.input >= t.crit ? '#ef4444' : t.high && t.input >= t.high ? '#f59e0b' : '#94a3b8'};">{t.input.toFixed(0)}°C</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/if}

    {#if tab === 'cpu' && cpu}
      <div class="space-y-4">
        <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
          <div class="flex justify-between items-center mb-3">
            <div>
              <div class="text-white font-medium text-sm">{cpu.model}</div>
              <div class="text-slate-500 text-xs mt-0.5">{cpu.cores.length} cores</div>
            </div>
            <div class="text-right">
              <div class="text-2xl font-light tabular-nums" style="color:#3b82f6;">{cpu.total_usage.toFixed(1)}%</div>
              <div class="text-xs text-slate-600">total usage</div>
            </div>
          </div>
          <div class="mb-3"><Sparkline data={cpuHist} color="#3b82f6" h={60} /></div>
          <Bar value={cpu.total_usage} color="#3b82f6" height={6} />
        </div>
        <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
          <div class="text-xs text-slate-500 mb-3">Load Average</div>
          <div class="grid grid-cols-3 gap-3">
            {#each [['1 min', cpu.load_avg_1], ['5 min', cpu.load_avg_5], ['15 min', cpu.load_avg_15]] as [l, v] (l)}
              <div class="text-center">
                <div class="text-xl font-light text-white tabular-nums">{Number(v).toFixed(2)}</div>
                <div class="text-xs text-slate-600">{l}</div>
              </div>
            {/each}
          </div>
        </div>
        <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
          <div class="text-xs text-slate-500 mb-3">Per-Core Usage</div>
          <CoreGrid {cpu} />
        </div>
      </div>
    {/if}

    {#if tab === 'memory' && mem}
      <div class="space-y-3">
        <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
          <div class="flex justify-between items-center mb-3">
            <div class="text-sm text-slate-400">RAM Usage</div>
            <div class="text-xl font-light tabular-nums text-white">{fmtBytes(mem.used)} / {fmtBytes(mem.total)}</div>
          </div>
          <div class="mb-3"><Sparkline data={ramHist} color="#a855f7" h={60} /></div>
          <Bar value={pct(mem.used, mem.total)} color="#a855f7" height={8} />
        </div>
        <div class="grid grid-cols-2 gap-3">
          {#each memRows as [l, v, c] (l)}
            <div class="bg-slate-800/60 rounded-xl p-3 border border-white/5">
              <div class="text-xs text-slate-500 mb-1">{l}</div>
              <div class="text-lg font-light tabular-nums" style="color:{c};">{fmtBytes(v)}</div>
              <div class="mt-2"><Bar value={pct(v, mem.total)} color={c} height={3} /></div>
            </div>
          {/each}
        </div>
        {#if mem.swap_total > 0}
          <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
            <div class="flex justify-between items-center mb-2">
              <div class="text-sm text-slate-400">Swap</div>
              <div class="text-slate-300 tabular-nums text-sm">{fmtBytes(mem.swap_used)} / {fmtBytes(mem.swap_total)}</div>
            </div>
            <Bar value={pct(mem.swap_used, mem.swap_total)} color="#f59e0b" height={6} />
          </div>
        {/if}
      </div>
    {/if}

    {#if tab === 'gpu'}
      <div class="space-y-3">
        {#if gpus.length === 0}
          <div class="text-slate-600 text-sm text-center py-12">No GPU detected or nvidia-smi/radeontop not installed</div>
        {:else}
          {#each gpus as g, i (i)}
            <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5 space-y-3">
              <div class="flex justify-between">
                <div><div class="text-white font-medium">{g.name}</div><div class="text-xs text-slate-500">{g.driver}</div></div>
                {#if g.temp_c != null}
                  <div class="text-right"><div class="text-xl tabular-nums" style="color:{g.temp_c > 85 ? '#ef4444' : g.temp_c > 70 ? '#f59e0b' : '#22c55e'};">{g.temp_c.toFixed(0)}°C</div></div>
                {/if}
              </div>
              {#if g.usage_pct != null}
                <div>
                  <div class="flex justify-between text-xs mb-1"><span class="text-slate-500">GPU Usage</span><span class="text-pink-400 tabular-nums">{g.usage_pct.toFixed(0)}%</span></div>
                  <div class="mb-2"><Sparkline data={gpuHist} color="#ec4899" h={40} /></div>
                  <Bar value={g.usage_pct} color="#ec4899" height={6} />
                </div>
              {/if}
              {#if g.vram_total != null}
                <div>
                  <div class="flex justify-between text-xs mb-1"><span class="text-slate-500">VRAM</span><span class="text-slate-300 tabular-nums">{fmtBytes(g.vram_used ?? 0)} / {fmtBytes(g.vram_total)}</span></div>
                  <Bar value={pct(g.vram_used ?? 0, g.vram_total)} color="#8b5cf6" height={6} />
                </div>
              {/if}
              {#if g.power_w != null}
                <div class="flex items-center gap-2 text-xs text-slate-500"><Zap size={12} class="text-yellow-400" /><span>{g.power_w.toFixed(1)} W</span></div>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/if}

    {#if tab === 'network'}
      <div class="space-y-3">
        <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
          <div class="text-xs text-slate-500 mb-3">Total throughput</div>
          <Sparkline data={netHist} color="#10b981" h={60} />
        </div>
        {#each nets as n (n.name)}
          <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
            <div class="flex items-center justify-between mb-3">
              <div class="flex items-center gap-2">
                <Wifi size={14} class={n.connected ? 'text-green-400' : 'text-slate-600'} />
                <span class="text-sm font-medium text-white font-mono">{n.name}</span>
                {#if n.ip4}<span class="text-xs text-slate-500">{n.ip4}</span>{/if}
              </div>
              <div class="w-2 h-2 rounded-full {n.connected ? 'bg-green-400' : 'bg-slate-700'}" />
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div class="text-center"><div class="text-xs text-slate-500 mb-1">Download</div><div class="text-lg font-light tabular-nums text-emerald-400">{fmtBps(n.rx_bps)}</div><div class="text-xs text-slate-600">{fmtBytes(n.rx_bytes)} total</div></div>
              <div class="text-center"><div class="text-xs text-slate-500 mb-1">Upload</div><div class="text-lg font-light tabular-nums text-blue-400">{fmtBps(n.tx_bps)}</div><div class="text-xs text-slate-600">{fmtBytes(n.tx_bytes)} total</div></div>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if tab === 'disk'}
      <div class="space-y-3">
        {#each disks as d, i (i)}
          <div class="bg-slate-800/60 rounded-2xl p-4 border border-white/5">
            <div class="flex items-center justify-between mb-3">
              <div><div class="text-sm font-medium text-white font-mono">{d.mount}</div><div class="text-xs text-slate-500">{d.name} · {d.fs}</div></div>
              <div class="text-right"><div class="text-sm tabular-nums text-slate-300">{fmtBytes(d.used)} / {fmtBytes(d.total)}</div><div class="text-xs text-slate-600">{fmtBytes(d.free)} free</div></div>
            </div>
            <Bar value={pct(d.used, d.total)} color="#f59e0b" height={6} />
          </div>
        {/each}
      </div>
    {/if}

    {#if tab === 'temperatures'}
      <div class="space-y-2">
        {#if temps.length === 0}
          <div class="text-slate-600 text-sm text-center py-12">No temperature sensors found.<br /><span class="text-xs">Install lm-sensors: sudo sensors-detect</span></div>
        {:else}
          {#each temps as t, i (i)}
            {@const crit = t.crit && t.input >= t.crit}
            {@const high = t.high && t.input >= t.high}
            {@const color = crit ? '#ef4444' : high ? '#f59e0b' : '#22c55e'}
            {@const max = t.crit ?? t.high ?? 100}
            <div class="bg-slate-800/60 rounded-xl p-3 border border-white/5">
              <div class="flex items-center justify-between mb-2">
                <span class="text-sm text-slate-300 truncate mr-4">{t.label}</span>
                <div class="flex items-center gap-2 shrink-0">
                  {#if high}<span class="text-xs text-yellow-500">HIGH</span>{/if}
                  {#if crit}<span class="text-xs text-red-500 font-bold">CRIT</span>{/if}
                  <span class="text-sm font-mono tabular-nums" style="color:{color};">{t.input.toFixed(1)}°C</span>
                </div>
              </div>
              <Bar value={t.input} {max} {color} height={4} />
              <div class="flex justify-between text-[10px] text-slate-700 mt-0.5">
                <span>0°C</span>
                {#if t.high}<span class="text-yellow-800">High {t.high}°C</span>{/if}
                {#if t.crit}<span class="text-red-800">Crit {t.crit}°C</span>{/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {/if}

    {#if tab === 'processes'}
      <div class="flex flex-col gap-3">
        <div class="relative">
          <Search size={13} class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
          <input bind:value={query} placeholder="Filter by name or PID…" class="w-full bg-slate-800 border border-white/10 rounded-xl pl-8 pr-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
          {#if query}<button on:click={() => (query = '')} class="absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white"><X size={13} /></button>{/if}
        </div>
        <div class="bg-slate-800/60 rounded-2xl border border-white/5 overflow-hidden">
          <table class="w-full text-xs">
            <thead>
              <tr class="border-b border-white/5 text-left">
                {#each THS as th (th.k)}
                  <th on:click={() => sortBy(th.k)} class="py-2 px-3 text-left cursor-pointer hover:text-white select-none text-slate-500 text-xs font-medium {th.cls}">
                    <span class="inline-flex items-center gap-0.5">
                      {th.label}
                      {#if sort === th.k}{#if asc}<ChevronUp size={11} />{:else}<ChevronDown size={11} />{/if}{/if}
                    </span>
                  </th>
                {/each}
                <th class="w-16" />
              </tr>
            </thead>
            <tbody>
              {#each sorted.slice(0, 120) as p (p.pid)}
                {@const isSel = selPid === p.pid}
                <tr on:click={() => (selPid = isSel ? null : p.pid)} class="border-b border-white/[0.03] cursor-pointer transition-colors {isSel ? 'bg-blue-900/20' : 'hover:bg-white/[0.03]'}">
                  <td class="px-3 py-1.5 font-mono text-slate-500">{p.pid}</td>
                  <td class="px-3 py-1.5">
                    <div class="text-slate-200 font-medium truncate max-w-[160px]">{p.name}</div>
                    {#if isSel && p.cmd}<div class="text-slate-600 text-[10px] font-mono truncate max-w-[240px]">{p.cmd}</div>{/if}
                  </td>
                  <td class="px-3 py-1.5 text-slate-500 hidden md:table-cell">{p.user}</td>
                  <td class="px-3 py-1.5 text-right tabular-nums" style="color:{p.cpu > 50 ? '#f87171' : p.cpu > 20 ? '#fb923c' : '#94a3b8'};">{p.cpu.toFixed(1)}</td>
                  <td class="px-3 py-1.5 text-right text-slate-400 tabular-nums">{fmtBytes(p.memory)}</td>
                  <td class="px-3 py-1.5 text-right">
                    {#if isSel}
                      <button on:click|stopPropagation={() => killProc(p.pid)} disabled={killing === p.pid} class="px-2 py-0.5 bg-red-600/20 hover:bg-red-600/40 text-red-400 rounded text-[10px] transition-colors">
                        {killing === p.pid ? '…' : 'Kill'}
                      </button>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
          <div class="px-3 py-2 border-t border-white/5 text-xs text-slate-600">{sorted.length} processes</div>
        </div>
      </div>
    {/if}
  </div>
</div>
