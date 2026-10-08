<script lang="ts">
  import { conflictRequest, type FmConflict, type FmDecision, type FmPolicy, type ExistsAnswer } from '../stores/conflictDialog';
  import { Folder, File, AlertTriangle } from 'lucide-svelte';

  let idx = 0;
  let applyAll = false;
  let inner: FmPolicy = 'skip';
  let decisions: Record<string, FmDecision> = {};
  let lastReq: unknown = null;

  // nowe pytanie → wyzeruj stan
  $: if ($conflictRequest !== lastReq) { lastReq = $conflictRequest; idx = 0; applyAll = false; inner = 'skip'; decisions = {}; }

  const fmtSize = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : b >= 1e3 ? `${Math.round(b / 1e3)} kB` : `${b} B`);
  const fmtDate = (t: number | null) => (t ? new Date(t * 1000).toLocaleString() : '—');

  function chooseTransfer(policy: FmPolicy) {
    const req = $conflictRequest;
    if (!req || req.kind !== 'transfer') return;
    const rest = applyAll ? req.conflicts.slice(idx) : [req.conflicts[idx]];
    for (const c of rest) {
      // „Scal" tylko folder→folder; dla reszty przy „do wszystkich" bezpieczny fallback = pomiń
      const p: FmPolicy = policy === 'merge' && !(c.src_is_dir && c.dest_is_dir) ? 'skip' : policy;
      decisions[c.src] = p === 'merge' ? { policy: p, inner } : { policy: p };
    }
    idx += rest.length;
    if (idx >= req.conflicts.length) { const d = decisions; $conflictRequest = null; req.resolve(d); }
  }
  function cancelTransfer() {
    const req = $conflictRequest;
    if (req?.kind === 'transfer') { $conflictRequest = null; req.resolve(null); }
  }
  function answerExists(a: ExistsAnswer) {
    const req = $conflictRequest;
    if (req?.kind === 'exists') { $conflictRequest = null; req.resolve(a); }
  }
  function onKey(e: KeyboardEvent) {
    if (!$conflictRequest || e.key !== 'Escape') return;
    e.stopPropagation();
    if ($conflictRequest.kind === 'transfer') cancelTransfer(); else answerExists('cancel');
  }
  $: cur = $conflictRequest?.kind === 'transfer' ? $conflictRequest.conflicts[idx] as FmConflict | undefined : undefined;
</script>

<svelte:window on:keydown|capture={onKey} />

{#if $conflictRequest?.kind === 'transfer' && cur}
  {@const req = $conflictRequest}
  {@const left = req.conflicts.length - idx}
  <div class="fixed inset-0 z-[10000] bg-black/60 flex items-center justify-center" role="presentation">
    <div class="bg-slate-800 border border-white/10 rounded-2xl shadow-2xl w-[34rem] max-w-[95vw] p-5 space-y-4 text-sm text-slate-200" role="dialog" aria-modal="true">
      <div class="flex items-start gap-3">
        <AlertTriangle class="text-yellow-400 shrink-0 mt-0.5" size={22} />
        <div>
          <div class="text-white font-semibold text-base">
            {cur.same_location ? 'Element o tej nazwie już istnieje w tym folderze' : cur.src_is_dir && cur.dest_is_dir ? 'Folder o tej nazwie już istnieje' : 'Plik o tej nazwie już istnieje'}
          </div>
          <div class="text-xs text-slate-400 mt-0.5">{req.mode === 'copy' ? 'Kopiowanie' : 'Przenoszenie'} · pozostało konfliktów: {left}</div>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3 text-xs">
        <div class="bg-slate-900/60 rounded-xl p-3">
          <div class="text-slate-400 mb-1">Istniejący</div>
          <div class="flex items-center gap-1.5 text-white font-medium break-all">{#if cur.dest_is_dir}<Folder size={14} />{:else}<File size={14} />{/if}{cur.name}</div>
          <div class="mt-1 text-slate-400">{fmtSize(cur.dest_size)}</div>
          <div class="text-slate-500">{fmtDate(cur.dest_modified)}</div>
        </div>
        <div class="bg-slate-900/60 rounded-xl p-3">
          <div class="text-slate-400 mb-1">{req.mode === 'copy' ? 'Kopiowany' : 'Przenoszony'}</div>
          <div class="flex items-center gap-1.5 text-white font-medium break-all">{#if cur.src_is_dir}<Folder size={14} />{:else}<File size={14} />{/if}{cur.name}</div>
          <div class="mt-1 text-slate-400">{fmtSize(cur.src_size)}</div>
          <div class="text-slate-500">
            {fmtDate(cur.src_modified)}
            {#if cur.src_modified && cur.dest_modified}
              <span class={cur.src_modified > cur.dest_modified ? 'text-green-400' : cur.src_modified < cur.dest_modified ? 'text-yellow-400' : ''}>
                {cur.src_modified > cur.dest_modified ? ' · nowszy' : cur.src_modified < cur.dest_modified ? ' · starszy' : ' · ta sama data'}
              </span>
            {/if}
          </div>
        </div>
      </div>

      {#if cur.src_is_dir && cur.dest_is_dir && !cur.same_location}
        <label class="flex items-center justify-between gap-3 text-xs">
          <span class="text-slate-400">Przy scalaniu, gdy plik w środku już istnieje:</span>
          <select class="bg-slate-700 rounded-lg px-2 py-1" bind:value={inner}>
            <option value="skip">pomiń istniejące</option>
            <option value="replace">zastąp istniejące</option>
            <option value="keep_both">zachowaj oba</option>
          </select>
        </label>
      {/if}

      {#if req.conflicts.length - idx > 1}
        <label class="flex items-center gap-2 text-xs text-slate-300"><input type="checkbox" bind:checked={applyAll} /> Zastosuj do wszystkich pozostałych konfliktów ({left})</label>
      {/if}

      <div class="flex flex-wrap gap-2 justify-end">
        <button class="px-3 py-1.5 rounded-lg hover:bg-white/10" on:click={cancelTransfer}>Anuluj</button>
        {#if !cur.same_location}<button class="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/20" on:click={() => chooseTransfer('skip')}>Pomiń</button>{/if}
        <button class="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/20" on:click={() => chooseTransfer('keep_both')}>Zachowaj oba</button>
        {#if cur.src_is_dir && cur.dest_is_dir && !cur.same_location}
          <button class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white" on:click={() => chooseTransfer('merge')}>Scal</button>
        {/if}
        {#if !cur.same_location}<button class="px-3 py-1.5 rounded-lg bg-red-600/80 hover:bg-red-500 text-white" on:click={() => chooseTransfer('replace')}>Zastąp</button>{/if}
      </div>
    </div>
  </div>
{:else if $conflictRequest?.kind === 'exists'}
  {@const req = $conflictRequest}
  <div class="fixed inset-0 z-[10000] bg-black/60 flex items-center justify-center" role="presentation">
    <div class="bg-slate-800 border border-white/10 rounded-2xl shadow-2xl w-[28rem] max-w-[95vw] p-5 space-y-4 text-sm text-slate-200" role="dialog" aria-modal="true">
      <div class="flex items-start gap-3">
        <AlertTriangle class="text-yellow-400 shrink-0 mt-0.5" size={22} />
        <div>
          <div class="text-white font-semibold text-base">{req.existing === 'folder' ? 'Folder' : 'Plik'} o nazwie „{req.name}” już istnieje</div>
          <div class="text-xs text-slate-400 mt-1">Co chcesz zrobić?</div>
        </div>
      </div>
      <div class="flex flex-wrap gap-2 justify-end">
        <button class="px-3 py-1.5 rounded-lg hover:bg-white/10" on:click={() => answerExists('cancel')}>Anuluj</button>
        <button class="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/20" on:click={() => answerExists('rename')}>Zmień nazwę…</button>
        {#if req.canKeepBoth}<button class="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/20" on:click={() => answerExists('keep_both')}>Zachowaj oba</button>{/if}
        {#if req.canReplace}<button class="px-3 py-1.5 rounded-lg bg-red-600/80 hover:bg-red-500 text-white" on:click={() => answerExists('replace')}>Zastąp</button>{/if}
      </div>
    </div>
  </div>
{/if}
