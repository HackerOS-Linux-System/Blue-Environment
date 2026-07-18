<script lang="ts">
  import { onMount } from 'svelte';
  import { SystemBridge, toAssetUrl } from '../../utils/systemBridge';
  import { RefreshCw, Cpu, CircuitBoard, HardDrive, Monitor, Server, Globe, Bug, ExternalLink } from 'lucide-svelte';

  interface DistroInfo { name: string; version: string; logoPath?: string; website?: string; copyright?: string; bugReportUrl?: string; variantName?: string; description?: string; supportUrl?: string; }
  interface HwInfo { kernel: string; cpu: number; ram: number; hostname: string; username: string; uptime: string; cpuModel: string; totalRam: string; gpuInfo: string; diskInfo: string; arch: string; }

  async function readDistroInfo(): Promise<DistroInfo> {
    try {
      const raw = await SystemBridge.readFile('/etc/xdg/kcm-about-distrorc');
      if (raw?.trim()) {
        const info: DistroInfo = { name: 'LegendaryOS', version: '0.6' };
        for (const line of raw.split('\n')) {
          const eq = line.indexOf('='); if (eq < 0) continue;
          const k = line.slice(0, eq).trim(), v = line.slice(eq + 1).trim();
          if (k === 'Name') info.name = v;
          else if (k === 'Version') info.version = v;
          else if (k === 'LogoPath') info.logoPath = v;
          else if (k === 'Website') info.website = v;
          else if (k === 'Copyright') info.copyright = v;
          else if (k === 'BugReportUrl') info.bugReportUrl = v;
          else if (k === 'VariantName') info.variantName = v;
          else if (k === 'Description') info.description = v;
          else if (k === 'SupportUrl') info.supportUrl = v;
        }
        return info;
      }
    } catch {}
    try {
      const raw = await SystemBridge.readFile('/etc/os-release');
      const info: DistroInfo = { name: 'Linux', version: 'Unknown' };
      for (const line of raw.split('\n')) {
        const eq = line.indexOf('='); if (eq < 0) continue;
        const k = line.slice(0, eq).trim(), v = line.slice(eq + 1).replace(/^["']|["']$/g, '').trim();
        if (k === 'PRETTY_NAME') info.name = v;
        else if (k === 'VERSION_ID') info.version = v;
        else if (k === 'HOME_URL') info.website = v;
        else if (k === 'BUG_REPORT_URL') info.bugReportUrl = v;
        else if (k === 'SUPPORT_URL') info.supportUrl = v;
      }
      return info;
    } catch {}
    return { name: 'LegendaryOS Linux', version: '0.6', copyright: '© 2026 LegendaryOS Team' };
  }

  async function readHwInfo(): Promise<HwInfo> {
    const stats = await SystemBridge.getSystemStats();
    const s = stats as any;

    let cpuModel = 'Unknown CPU', totalRam = '—', gpuInfo = '—', diskInfo = '—', arch = '—';
    try {
      const r = await SystemBridge.executeCommand("grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2");
      cpuModel = (typeof r === 'string' ? r : (r as any)?.stdout || '').trim() || 'Unknown CPU';
    } catch {}
    try {
      const r = await SystemBridge.executeCommand("grep MemTotal /proc/meminfo | awk '{printf \"%.1f GB\", $2/1024/1024}'");
      totalRam = (typeof r === 'string' ? r : (r as any)?.stdout || '').trim() || '—';
    } catch {}
    try {
      const r = await SystemBridge.executeCommand("lspci | grep -i 'vga\\|3d\\|display' | head -1 | cut -d: -f3");
      gpuInfo = (typeof r === 'string' ? r : (r as any)?.stdout || '').trim() || '—';
    } catch {}
    try {
      const r = await SystemBridge.executeCommand("df -h / | tail -1 | awk '{print $2 \" total, \" $3 \" used (\" $5 \")\"}'");
      diskInfo = (typeof r === 'string' ? r : (r as any)?.stdout || '').trim() || '—';
    } catch {}
    try {
      const r = await SystemBridge.executeCommand('uname -m');
      arch = (typeof r === 'string' ? r : (r as any)?.stdout || '').trim() || '—';
    } catch {}

    const up = (s.uptime as number) || 0;
    const days = Math.floor(up / 86400);
    const hrs = Math.floor((up % 86400) / 3600);
    const mins = Math.floor((up % 3600) / 60);
    const uptime = days > 0 ? `${days}d ${hrs}h ${mins}m` : hrs > 0 ? `${hrs}h ${mins}m` : `${mins}m`;

    let hostname = 'localhost';
    try { hostname = await SystemBridge.getHostname(); } catch {}

    return { kernel: stats.kernel || 'Unknown', cpu: Math.round(stats.cpu), ram: Math.round(stats.ram), hostname, username: SystemBridge.getUsername(), uptime, cpuModel, totalRam, gpuInfo, diskInfo, arch };
  }

  let distro: DistroInfo | null = null;
  let hw: HwInfo | null = null;
  let loading = true;

  async function load() {
    loading = true;
    const [d, h] = await Promise.all([readDistroInfo(), readHwInfo()]);
    distro = d; hw = h; loading = false;
  }

  function hideBrokenLogo(e: Event) {
    (e.currentTarget as HTMLImageElement).style.display = 'none';
  }

  onMount(load);

  function openUrl(url?: string) { if (url) SystemBridge.launchApp(`xdg-open "${url}"`); }

  $: hwRows = [
    { icon: Cpu, label: 'Processor', value: hw?.cpuModel, sub: hw ? `${hw.cpu}% usage` : undefined },
    { icon: CircuitBoard, label: 'Memory', value: hw?.totalRam, sub: hw ? `${hw.ram}% used` : undefined },
    { icon: Monitor, label: 'Graphics', value: hw?.gpuInfo },
    { icon: HardDrive, label: 'Storage', value: hw?.diskInfo },
  ];
  $: sysRows = [
    { icon: Server, label: 'Kernel', value: hw?.kernel },
    { icon: Server, label: 'Hostname', value: hw?.hostname },
    { icon: Server, label: 'Uptime', value: hw?.uptime },
  ];
</script>

{#if loading}
  <div class="h-full flex items-center justify-center bg-slate-900"><RefreshCw size={24} class="animate-spin text-blue-400" /></div>
{:else}
  <div class="flex flex-col h-full bg-slate-900 text-white overflow-y-auto">
    <div class="relative bg-gradient-to-br from-blue-900/50 via-slate-900 to-slate-900 px-8 py-8 border-b border-white/5 overflow-hidden">
      <div class="absolute inset-0" style="background:radial-gradient(circle at 30% 50%, rgba(59,130,246,0.15), transparent 60%);" />
      <div class="relative flex items-center gap-6">
        {#if distro?.logoPath}
          <img src={toAssetUrl(distro.logoPath)} alt="Logo" class="w-20 h-20 object-contain shrink-0"
               on:error={hideBrokenLogo} />
        {:else}
          <div class="w-20 h-20 rounded-2xl bg-gradient-to-br from-blue-500 to-blue-700 flex items-center justify-center shadow-2xl shadow-blue-500/40 shrink-0">
            <span class="text-4xl font-black text-white">{(distro?.name || 'L').charAt(0).toUpperCase()}</span>
          </div>
        {/if}
        <div>
          <h1 class="text-2xl font-bold text-white">{distro?.name}</h1>
          {#if distro?.variantName}<p class="text-blue-300 text-sm mt-0.5">{distro.variantName}</p>{/if}
          {#if distro?.description}<p class="text-slate-400 text-sm mt-1">{distro.description}</p>{/if}
          <div class="flex items-center gap-3 mt-2">
            <span class="bg-blue-600/20 text-blue-300 text-xs px-2.5 py-0.5 rounded-full border border-blue-500/20">v{distro?.version}</span>
            <span class="bg-white/5 text-slate-300 text-xs px-2.5 py-0.5 rounded-full border border-white/10">{hw?.arch}</span>
          </div>
          <div class="flex gap-2 mt-2">
            {#if distro?.website}
              <button on:click={() => openUrl(distro?.website)} class="flex items-center gap-1 text-xs text-blue-400 hover:text-blue-300 transition-colors"><Globe size={11} /> Website</button>
            {/if}
            {#if distro?.supportUrl}
              <button on:click={() => openUrl(distro?.supportUrl)} class="flex items-center gap-1 text-xs text-slate-400 hover:text-white transition-colors"><ExternalLink size={11} /> Support</button>
            {/if}
            {#if distro?.bugReportUrl}
              <button on:click={() => openUrl(distro?.bugReportUrl)} class="flex items-center gap-1 text-xs text-slate-400 hover:text-white transition-colors"><Bug size={11} /> Report Bug</button>
            {/if}
          </div>
        </div>
      </div>
    </div>

    <div class="px-6 py-4 border-b border-white/5">
      <h2 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-3">Desktop Environment</h2>
      <div class="bg-slate-800 rounded-xl p-4 flex items-center gap-4 border border-white/5">
        <div class="w-10 h-10 bg-blue-600/20 rounded-xl flex items-center justify-center shrink-0"><Monitor size={20} class="text-blue-400" /></div>
        <div class="flex-1">
          <div class="font-semibold text-white">Blue Environment</div>
          <div class="text-sm text-slate-400">Version 0.6.0 - Smithay Wayland Compositor (Svelte UI)</div>
        </div>
        <div class="text-right">
          <div class="text-xs text-slate-500">GNU GPL v3.0</div>
          <div class="text-xs text-slate-500 mt-0.5">© 2026 LegendaryOS Team</div>
        </div>
      </div>
    </div>

    <div class="px-6 py-4 border-b border-white/5">
      <h2 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-3">Hardware</h2>
      <div class="space-y-1.5">
        {#each hwRows as { icon: Icon, label, value, sub } (label)}
          <div class="flex items-center gap-3 bg-slate-800/50 rounded-lg px-4 py-2.5 border border-white/5">
            <svelte:component this={Icon} size={15} class="text-slate-500 shrink-0" />
            <span class="text-slate-400 text-sm w-24 shrink-0">{label}</span>
            <div class="flex-1 min-w-0">
              <div class="text-white text-sm truncate">{value || '—'}</div>
              {#if sub}<div class="text-slate-500 text-xs">{sub}</div>{/if}
            </div>
          </div>
        {/each}
      </div>
    </div>

    <div class="px-6 py-4 border-b border-white/5">
      <h2 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-3">System</h2>
      <div class="space-y-1.5">
        {#each sysRows as { icon: Icon, label, value } (label)}
          <div class="flex items-center gap-3 bg-slate-800/50 rounded-lg px-4 py-2.5 border border-white/5">
            <svelte:component this={Icon} size={15} class="text-slate-500 shrink-0" />
            <span class="text-slate-400 text-sm w-24 shrink-0">{label}</span>
            <span class="text-white text-sm font-mono">{value || '—'}</span>
          </div>
        {/each}
      </div>
    </div>

    <div class="px-6 py-4 border-b border-white/5">
      <h2 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-3">Current User</h2>
      <div class="flex items-center gap-4 bg-slate-800/50 rounded-xl px-4 py-3 border border-white/5">
        <div class="w-10 h-10 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white font-bold text-lg shadow">
          {hw?.username?.charAt(0).toUpperCase() || 'U'}
        </div>
        <div>
          <div class="font-medium text-white">{hw?.username || '—'}</div>
          <div class="text-xs text-slate-500">{hw?.hostname || 'localhost'}</div>
        </div>
      </div>
    </div>

    <div class="px-6 py-4 flex items-center justify-between">
      {#if distro?.copyright}<p class="text-xs text-slate-600">{distro.copyright}</p>{/if}
      <div class="ml-auto">
        <button on:click={load} class="p-2 hover:bg-white/10 rounded-full text-slate-400 hover:text-white transition-colors" title="Refresh"><RefreshCw size={15} /></button>
      </div>
    </div>
  </div>
{/if}
