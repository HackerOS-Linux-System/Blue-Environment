<script lang="ts">
  import { ShieldAlert, ShieldCheck, ShieldQuestion } from 'lucide-svelte';
  import { t } from '../stores/language';
  import { consentRequest, defaultSelection, isNew, type Risk } from '../stores/permissionConsent';

  let selected: Set<string> = new Set();
  let lastId = '';
  $: req = $consentRequest;
  $: if (req && req.preview.manifest.id + req.preview.manifest.version !== lastId) {
    lastId = req.preview.manifest.id + req.preview.manifest.version;
    selected = new Set(defaultSelection(req.preview));
  }

  const riskClass: Record<Risk, string> = {
    high: 'bg-red-500/20 text-red-300',
    medium: 'bg-amber-500/20 text-amber-300',
    low: 'bg-emerald-500/20 text-emerald-300',
  };

  function toggle(id: string) {
    const next = new Set(selected);
    next.has(id) ? next.delete(id) : next.add(id);
    selected = next;
  }
  const install = () => req?.answer([...selected]);
  const cancel = () => req?.answer(null);
</script>

<svelte:window on:keydown={(e) => req && e.key === 'Escape' && cancel()} />

{#if req}
  {@const p = req.preview}
  <div class="fixed inset-0 z-[10000] grid place-items-center bg-black/60 backdrop-blur-sm" role="presentation">
    <div class="w-[26rem] max-w-[92vw] rounded-2xl border border-white/10 bg-slate-800 p-5 shadow-2xl" role="dialog" aria-modal="true" aria-labelledby="consent-title">
      <h2 id="consent-title" class="text-base font-semibold text-white">{$t('store.consent.title', { name: p.manifest.name })}</h2>
      <p class="mt-0.5 text-xs text-slate-400">{p.manifest.author} · v{p.manifest.version}</p>

      <div class="mt-3 flex items-start gap-2 rounded-xl bg-slate-900/60 p-2.5 text-xs">
        {#if p.signature.status === 'trusted'}
          <ShieldCheck size={16} class="mt-0.5 shrink-0 text-emerald-400" />
          <span class="text-slate-200">{$t('store.consent.signed_by', { name: p.signature.name })}</span>
        {:else if p.signature.status === 'unknownSigner'}
          <ShieldQuestion size={16} class="mt-0.5 shrink-0 text-amber-400" />
          <span class="text-amber-200">{$t('store.consent.unknown_signer', { name: p.signature.signer })}</span>
        {:else if p.signature.status === 'invalid'}
          <ShieldAlert size={16} class="mt-0.5 shrink-0 text-red-400" />
          <span class="text-red-200">{$t('store.consent.invalid_signature')}</span>
        {:else}
          <ShieldQuestion size={16} class="mt-0.5 shrink-0 text-amber-400" />
          <span class="text-amber-200">{$t('store.consent.unsigned')}</span>
        {/if}
      </div>

      <p class="mt-3 text-xs text-slate-300">{$t('store.consent.asks')}</p>
      <ul class="mt-2 max-h-64 space-y-1.5 overflow-y-auto">
        {#each p.permissions as perm (perm.id)}
          <li>
            <label class="flex cursor-pointer items-start gap-2.5 rounded-lg p-2 hover:bg-white/5">
              <input type="checkbox" class="mt-0.5 h-4 w-4" checked={selected.has(perm.id)} on:change={() => toggle(perm.id)} />
              <span class="min-w-0 flex-1">
                <span class="flex flex-wrap items-center gap-1.5 text-sm text-white">
                  {perm.title}
                  <span class="rounded px-1.5 py-px text-[10px] uppercase tracking-wide {riskClass[perm.risk]}">{$t(`store.consent.risk_${perm.risk}`)}</span>
                  {#if isNew(p, perm.id)}<span class="rounded bg-blue-500/20 px-1.5 py-px text-[10px] uppercase tracking-wide text-blue-300">{$t('store.consent.new')}</span>{/if}
                </span>
                <span class="block text-xs text-slate-400">{perm.description}</span>
              </span>
            </label>
          </li>
        {/each}
      </ul>
      <p class="mt-2 text-[11px] text-slate-500">{$t('store.consent.footnote')}</p>

      <div class="mt-4 flex justify-end gap-2">
        <button class="rounded-lg bg-slate-700 px-4 py-2 text-sm text-white hover:bg-slate-600" on:click={cancel}>{$t('store.consent.cancel')}</button>
        <button class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500" on:click={install}>
          {selected.size === p.permissions.length ? $t('store.consent.install_all') : $t('store.consent.install_selected', { n: selected.size })}
        </button>
      </div>
    </div>
  </div>
{/if}
