<script lang="ts">
  import { onMount } from 'svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import { Monitor, RefreshCw } from 'lucide-svelte';

  interface MonitorInfo { name: string; resolution: string; rate: string; scale: string; primary: boolean; connected: boolean; x: number; y: number; }

  let monitors: MonitorInfo[] = [];
  let loading = true;

  async function load() {
    loading = true;
    try {
      const result = await SystemBridge.executeCommand(
        "xrandr --query 2>/dev/null | grep -E 'connected|\\*' || wlr-randr 2>/dev/null || echo 'UNAVAILABLE'"
      );
      const out = typeof result === 'string' ? result : (result as any)?.stdout || '';
      if (out.includes('UNAVAILABLE') || !out.trim()) {
        monitors = [{ name: 'Primary Monitor', resolution: '1920x1080', rate: '60.0', scale: '1.0', primary: true, connected: true, x: 0, y: 0 }];
      } else {
        const mons: MonitorInfo[] = [];
        const lines = out.split('\n');
        let current: Partial<MonitorInfo> | null = null;
        for (const line of lines) {
          if (line.match(/^[A-Z].*connected/)) {
            if (current?.name) mons.push(current as MonitorInfo);
            const parts = line.split(' ');
            current = { name: parts[0], connected: true, primary: line.includes('primary'), resolution: '—', rate: '—', scale: '1.0', x: 0, y: 0 };
            const geom = line.match(/(\d+)x(\d+)\+(\d+)\+(\d+)/);
            if (geom) { current.resolution = `${geom[1]}x${geom[2]}`; current.x = parseInt(geom[3]); current.y = parseInt(geom[4]); }
          } else if (current && line.match(/^\s+\d+x\d+.*\*/)) {
            const m = line.match(/(\d+\.\d+)\*/);
            if (m) current.rate = m[1];
          }
        }
        if (current?.name) mons.push(current as MonitorInfo);
        monitors = mons.length > 0 ? mons : [{ name: 'Primary', resolution: '1920x1080', rate: '60.0', scale: '1.0', primary: true, connected: true, x: 0, y: 0 }];
      }
    } catch { monitors = []; }
    loading = false;
  }

  onMount(load);

  async function applyScale(mon: MonitorInfo, scale: string) {
    await SystemBridge.executeCommand(`xrandr --output "${mon.name}" --scale ${scale}x${scale}`).catch(() => {});
    load();
  }
  async function applyRotation(mon: MonitorInfo, rotation: string) {
    await SystemBridge.executeCommand(`xrandr --output "${mon.name}" --rotate ${rotation}`).catch(() => {});
  }

  const scales = ['0.5', '0.75', '1.0', '1.25', '1.5', '1.75', '2.0', '2.5', '3.0'];
</script>

{#if loading}
  <div class="flex items-center justify-center py-12"><RefreshCw size={20} class="animate-spin text-blue-400" /></div>
{:else}
  <div class="space-y-4">
    <div class="bg-slate-800 rounded-xl p-4 border border-white/5 relative overflow-hidden" style="min-height:120px;">
      <div class="flex items-center gap-2 flex-wrap">
        {#each monitors as mon (mon.name)}
          <div class="border-2 rounded-lg flex items-center justify-center {mon.primary ? 'border-blue-500' : 'border-white/20'}"
               style="width:120px; height:80px; background:rgba(255,255,255,0.05);">
            <div class="text-center">
              <Monitor size={20} class="mx-auto text-blue-400 mb-1" />
              <div class="text-xs text-white truncate px-1">{mon.name}</div>
              <div class="text-[10px] text-slate-400">{mon.resolution}</div>
            </div>
          </div>
        {/each}
      </div>
      <button on:click={load} class="absolute top-2 right-2 p-1 hover:bg-white/10 rounded text-slate-500"><RefreshCw size={12} /></button>
    </div>

    {#each monitors as mon (mon.name)}
      <div class="bg-slate-800 rounded-xl p-5 border border-white/5 space-y-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <Monitor size={20} class="text-blue-400" />
            <div>
              <div class="font-semibold text-white">{mon.name}</div>
              <div class="text-xs text-slate-500">{mon.connected ? 'Connected' : 'Disconnected'}</div>
            </div>
          </div>
          {#if mon.primary}<span class="text-xs bg-blue-600/20 text-blue-300 px-2 py-0.5 rounded-full border border-blue-500/20">Primary</span>{/if}
        </div>
        <div class="grid grid-cols-2 gap-3 text-sm">
          <div class="bg-slate-900/50 rounded-lg p-3">
            <div class="text-slate-400 text-xs mb-1">Resolution</div>
            <div class="font-medium">{mon.resolution}</div>
          </div>
          <div class="bg-slate-900/50 rounded-lg p-3">
            <div class="text-slate-400 text-xs mb-1">Refresh Rate</div>
            <div class="font-medium">{mon.rate} Hz</div>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <div class="flex-1">
            <label class="text-xs text-slate-400 block mb-1">Scale</label>
            <select value={mon.scale} on:change={(e) => applyScale(mon, e.currentTarget.value)}
              class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
              {#each scales as s (s)}<option value={s}>{(parseFloat(s) * 100).toFixed(0)}%</option>{/each}
            </select>
          </div>
          <div class="flex-1">
            <label class="text-xs text-slate-400 block mb-1">Rotation</label>
            <select value="normal" on:change={(e) => applyRotation(mon, e.currentTarget.value)}
              class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
              <option value="normal">Normal (0°)</option>
              <option value="right">Right (90°)</option>
              <option value="inverted">Inverted (180°)</option>
              <option value="left">Left (270°)</option>
            </select>
          </div>
        </div>
      </div>
    {/each}
  </div>
{/if}
