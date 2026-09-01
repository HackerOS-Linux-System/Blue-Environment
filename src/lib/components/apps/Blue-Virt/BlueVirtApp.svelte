<script lang="ts">
  // Blue Virt — VM manager, in the spirit of VirtualBox/GNOME Boxes.
  // See src-tauri/src/BlueVirt/mod.rs's module doc for exactly what's
  // real (real qcow2 disks, real qemu-system-x86_64 processes, real
  // graceful shutdown via QEMU's monitor socket) and what isn't yet
  // (no embedded display — each running VM opens its own QEMU window;
  // no snapshots, no USB passthrough).
  import { onMount } from 'svelte';
  import { Plus, Play, Square, Trash2, MonitorPlay, Loader2, Cpu, HardDrive, X, Info, Zap, ZapOff } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import LoadingSpinner from '../../../LoadingSpinner.svelte';
  import type { VmSummary, OsType } from './types';
  import { OS_TYPE_LABELS } from './types';

  export let windowId: string;

  let vms: VmSummary[] = [];
  let loading = true;
  let kvmAvailable = false;
  let busyVmId: string | null = null;
  let error: string | null = null;

  let showCreate = false;
  let newName = '';
  let newOsType: OsType = 'linux';
  let newCpuCores = 2;
  let newMemoryMb = 2048;
  let newDiskSizeGb = 20;
  let newIsoPath = '';
  let creating = false;

  async function refresh() {
    vms = await SystemBridge.bvListVms();
  }

  onMount(async () => {
    loading = true;
    try {
      kvmAvailable = await SystemBridge.bvIsKvmAvailable();
      await refresh();
    } finally {
      loading = false;
    }
  });

  async function createVm() {
    if (!newName.trim()) return;
    creating = true;
    error = null;
    try {
      const res = await SystemBridge.bvCreateVm(
        newName.trim(), newOsType, newCpuCores, newMemoryMb, newDiskSizeGb,
        newIsoPath.trim() || null
      );
      if (!res.ok) { error = res.error ?? 'Failed to create VM'; return; }
      showCreate = false;
      newName = '';
      newIsoPath = '';
      await refresh();
    } finally {
      creating = false;
    }
  }

  async function startVm(id: string) {
    busyVmId = id;
    error = null;
    try {
      const res = await SystemBridge.bvStartVm(id);
      if (!res.ok) error = res.error ?? 'Failed to start VM';
      await refresh();
    } finally {
      busyVmId = null;
    }
  }

  async function stopVm(id: string) {
    busyVmId = id;
    error = null;
    try {
      // Graceful by default — see mod.rs's bv_stop_vm doc for why this
      // takes a few real seconds (it waits for the guest to actually
      // shut down after asking nicely via the QEMU monitor socket).
      const res = await SystemBridge.bvStopVm(id, false);
      if (!res.ok) error = res.error ?? 'Failed to stop VM';
      await refresh();
    } finally {
      busyVmId = null;
    }
  }

  async function deleteVm(id: string) {
    busyVmId = id;
    error = null;
    try {
      const res = await SystemBridge.bvDeleteVm(id);
      if (!res.ok) error = res.error ?? 'Failed to delete VM';
      await refresh();
    } finally {
      busyVmId = null;
    }
  }
</script>

<div class="relative flex flex-col h-full bg-slate-950 text-slate-100 text-sm">
  <div class="px-4 py-3 border-b border-white/10 flex items-center justify-between">
    <div>
      <h1 class="font-medium">Blue Virt</h1>
      <p class="text-[11px] text-slate-500 flex items-center gap-1">
        {#if kvmAvailable}<Zap class="w-3 h-3 text-emerald-400" /> Hardware acceleration (KVM) available{:else}<ZapOff class="w-3 h-3 text-amber-400" /> No KVM — VMs will run in software emulation (slow){/if}
      </p>
    </div>
    <button on:click={() => (showCreate = true)} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs bg-blue-600 hover:bg-blue-500 transition-colors">
      <Plus class="w-3.5 h-3.5" /> New VM
    </button>
  </div>

  <div class="px-4 py-2 border-b border-white/5 flex items-start gap-1.5 text-[11px] text-slate-500">
    <Info class="w-3 h-3 shrink-0 mt-0.5" />
    Each running VM opens its own QEMU display window — display isn't embedded in this app yet. No snapshots or USB passthrough yet either.
  </div>

  <div class="flex-1 overflow-y-auto p-4">
    {#if loading}
      <LoadingSpinner label="Loading virtual machines…" />
    {:else if vms.length === 0}
      <div class="flex flex-col items-center gap-2 py-16 text-center text-slate-500">
        <MonitorPlay class="w-8 h-8 opacity-30" />
        <p class="text-sm">No virtual machines yet.</p>
        <p class="text-xs">Click "New VM" to create one.</p>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-3">
        {#each vms as vm (vm.id)}
          <div class="rounded-xl border border-white/10 bg-slate-900/60 p-3 flex flex-col gap-2">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2 min-w-0">
                <MonitorPlay class="w-4 h-4 text-blue-400 shrink-0" />
                <span class="font-medium truncate">{vm.name}</span>
              </div>
              <span class="text-[9px] uppercase tracking-wide px-1.5 py-0.5 rounded shrink-0 {vm.status === 'running' ? 'bg-emerald-600/20 text-emerald-400' : 'bg-slate-700/50 text-slate-400'}">
                {vm.status}
              </span>
            </div>
            <div class="text-[11px] text-slate-500 flex items-center gap-3">
              <span class="flex items-center gap-1"><Cpu class="w-3 h-3" /> {vm.cpuCores} vCPU · {(vm.memoryMb / 1024).toFixed(1)}GB RAM</span>
            </div>
            <div class="text-[11px] text-slate-500 flex items-center gap-1">
              <HardDrive class="w-3 h-3" /> {vm.diskSizeGb}GB · {OS_TYPE_LABELS[vm.osType]}
            </div>
            <div class="flex gap-2 mt-1">
              {#if vm.status === 'running'}
                <button on:click={() => stopVm(vm.id)} disabled={busyVmId === vm.id} class="flex-1 flex items-center justify-center gap-1 py-1.5 rounded bg-slate-800 hover:bg-slate-700 text-xs disabled:opacity-50">
                  {#if busyVmId === vm.id}<Loader2 class="w-3.5 h-3.5 animate-spin" />{:else}<Square class="w-3.5 h-3.5" />{/if} Stop
                </button>
              {:else}
                <button on:click={() => startVm(vm.id)} disabled={busyVmId === vm.id} class="flex-1 flex items-center justify-center gap-1 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-xs disabled:opacity-50">
                  {#if busyVmId === vm.id}<Loader2 class="w-3.5 h-3.5 animate-spin" />{:else}<Play class="w-3.5 h-3.5" />{/if} Start
                </button>
                <button on:click={() => deleteVm(vm.id)} disabled={busyVmId === vm.id} class="px-2.5 rounded bg-red-600/20 hover:bg-red-500/30 text-red-400 text-xs">
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if showCreate}
    <div class="absolute inset-0 bg-black/50 flex items-center justify-center z-10">
      <div class="bg-slate-900 border border-white/10 rounded-lg w-96 p-4 flex flex-col gap-3 max-h-[85%] overflow-y-auto">
        <div class="flex items-center justify-between">
          <span class="font-medium text-sm">New virtual machine</span>
          <button on:click={() => (showCreate = false)} class="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10"><X class="w-4 h-4" /></button>
        </div>
        <input bind:value={newName} placeholder="Name" class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500" />
        <select bind:value={newOsType} class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500">
          {#each Object.entries(OS_TYPE_LABELS) as [value, label]}
            <option {value}>{label}</option>
          {/each}
        </select>
        <label class="text-xs text-slate-400 flex items-center justify-between">
          <span>CPU cores: {newCpuCores}</span>
          <input type="range" min="1" max="16" bind:value={newCpuCores} class="w-40" />
        </label>
        <label class="text-xs text-slate-400 flex items-center justify-between">
          <span>Memory: {(newMemoryMb / 1024).toFixed(1)} GB</span>
          <input type="range" min="512" max="32768" step="512" bind:value={newMemoryMb} class="w-40" />
        </label>
        <label class="text-xs text-slate-400 flex items-center justify-between">
          <span>Disk size: {newDiskSizeGb} GB</span>
          <input type="range" min="5" max="500" bind:value={newDiskSizeGb} class="w-40" />
        </label>
        <input bind:value={newIsoPath} placeholder="Path to install ISO (optional)" class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500" />
        <button
          on:click={createVm}
          disabled={!newName.trim() || creating}
          class="bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded px-3 py-1.5 text-sm font-medium flex items-center justify-center gap-2"
        >
          {#if creating}<Loader2 class="w-4 h-4 animate-spin" /> Creating disk image…{:else}Create{/if}
        </button>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="absolute bottom-3 right-3 bg-red-500/90 text-white text-xs px-3 py-2 rounded shadow-lg">{error}</div>
  {/if}
</div>
