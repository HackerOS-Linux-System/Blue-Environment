<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { Copy, FolderInput, Pause, Play, X } from 'lucide-svelte';
  import { t } from '../stores/language';
  import { transferJobs, updateJob, controlJob, fmtBytes, fmtEta, percent, type ProgressEvent } from '../stores/transferJobs';

  export let enabled = true;
  let unlisten: (() => void) | undefined;
  onMount(async () => { unlisten = await listen<ProgressEvent>('fm:progress', (e) => updateJob(e.payload)); });
  onDestroy(() => unlisten?.());

  $: visible = enabled ? $transferJobs.filter((j) => j.visible) : [];
</script>

{#if visible.length}
  <div data-shell-occluder="transfers" class="fixed right-4 bottom-16 z-[9990] flex flex-col gap-3 w-[22rem] max-w-[92vw]">
    {#each visible as j (j.jobId)}
      <div class="bg-slate-800/95 backdrop-blur border border-white/10 rounded-2xl shadow-2xl p-4 text-sm text-slate-200" role="status" aria-live="polite">
        <div class="flex items-center gap-2 mb-2">
          {#if j.mode === 'copy'}<Copy size={16} class="text-blue-400" />{:else}<FolderInput size={16} class="text-blue-400" />{/if}
          <div class="font-semibold text-white flex-1">
            {j.cancelling ? $t('transfer.cancelling') : j.paused ? $t('transfer.paused') : j.mode === 'copy' ? $t('transfer.copying') : $t('transfer.moving')}
          </div>
          <button class="p-1 rounded-lg hover:bg-white/10 disabled:opacity-40" title={j.paused ? $t('transfer.resume') : $t('transfer.pause')} disabled={j.cancelling}
            on:click={() => controlJob(j.jobId, j.paused ? 'resume' : 'pause')}>
            {#if j.paused}<Play size={15} />{:else}<Pause size={15} />{/if}
          </button>
          <button class="p-1 rounded-lg hover:bg-red-500/20 text-red-400 disabled:opacity-40" title={$t('transfer.cancel')} disabled={j.cancelling}
            on:click={() => controlJob(j.jobId, 'cancel')}><X size={15} /></button>
        </div>

        <div class="text-xs text-slate-300 truncate mb-2" title={j.current}>{j.current || '…'}</div>
        <div class="h-2 rounded-full bg-slate-700 overflow-hidden" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(percent(j))}>
          <div class="h-full bg-blue-500 transition-[width] duration-100 {j.paused ? 'opacity-50' : ''}" style="width:{percent(j)}%" />
        </div>
        <div class="flex justify-between text-[11px] text-slate-400 mt-1.5">
          <span>{Math.round(percent(j))}% · {fmtBytes(j.doneBytes)} / {fmtBytes(j.totalBytes)}</span>
          <span>{$t('transfer.files', { done: j.doneFiles, total: j.totalFiles })}</span>
        </div>
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>{j.paused ? '' : `${fmtBytes(j.bytesPerSec)}/s`}</span>
          <span>{j.paused || j.etaSecs == null ? '' : $t('transfer.eta', { time: fmtEta(j.etaSecs) })}</span>
        </div>
      </div>
    {/each}
  </div>
{/if}
