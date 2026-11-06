<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { KeyRound } from 'lucide-svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { t } from '../stores/language';

  /**
   * Master-password dialog for the Secret Service bridge. An application that
   * wants a secret (`secret-tool`, git credential helper, a browser…) triggers
   * `secrets:unlock-request`; the answer goes back through `secrets_prompt_reply`.
   * `secrets:unlock-hint` (an app searched while the vault is locked) opens the
   * same dialog and unlocks the vault directly.
   */
  interface Pending { promptId: string | null; client: string; vaultExists: boolean }
  let pending = $state<Pending | null>(null);
  let password = $state('');
  let confirm = $state('');
  let error = $state('');
  let busy = $state(false);
  let unlisten: Array<() => void> = [];
  let lastHint = 0;

  async function listenAll() {
    if (!SystemBridge.isTauri()) return;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(await listen<{ promptId: string; client: string; vaultExists: boolean }>('secrets:unlock-request', (e) => open({ promptId: e.payload.promptId, client: e.payload.client, vaultExists: e.payload.vaultExists })));
      unlisten.push(await listen<{ client: string }>('secrets:unlock-hint', async (e) => {
        const now = Date.now();
        if (pending || now - lastHint < 30_000) return; // don't nag
        lastHint = now;
        const exists = await SystemBridge.accountsVaultExists();
        if (exists) open({ promptId: null, client: e.payload.client, vaultExists: true });
      }));
      unlisten.push(await listen<{ promptId: string }>('secrets:prompt-done', (e) => { if (pending?.promptId === e.payload.promptId) close(); }));
    } catch { /* not running inside Tauri */ }
  }
  function open(p: Pending) { pending = p; password = ''; confirm = ''; error = ''; busy = false; }
  function close() { pending = null; password = ''; confirm = ''; }

  async function submit() {
    if (!pending) return;
    error = '';
    if (!pending.vaultExists && password !== confirm) { error = $t('secrets.prompt.mismatch'); return; }
    if (!password) return;
    busy = true;
    try {
      if (pending.promptId) {
        await SystemBridge.invoke('secrets_prompt_reply', { promptId: pending.promptId, password, create: !pending.vaultExists });
      } else {
        const res = await SystemBridge.accountsUnlock(password);
        if (!res.ok) throw new Error('wrong');
      }
      close();
    } catch {
      error = $t('secrets.prompt.wrong');
    } finally { busy = false; }
  }
  async function dismiss() {
    if (pending?.promptId) { try { await SystemBridge.invoke('secrets_prompt_reply', { promptId: pending.promptId, password: null, create: false }); } catch { /* already gone */ } }
    close();
  }

  onMount(listenAll);
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<svelte:window on:keydown={(e) => pending && e.key === 'Escape' && dismiss()} />

{#if pending}
  <div class="fixed inset-0 z-[10001] grid place-items-center bg-black/60 backdrop-blur-sm" role="presentation">
    <form class="w-[22rem] max-w-[92vw] rounded-2xl border border-white/10 bg-slate-800 p-5 shadow-2xl" role="dialog" aria-modal="true" aria-labelledby="secrets-title" onsubmit={(e) => { e.preventDefault(); submit(); }}>
      <div class="mb-3 flex items-center gap-2.5">
        <div class="grid h-9 w-9 place-items-center rounded-xl bg-blue-600/30"><KeyRound size={18} class="text-blue-300" /></div>
        <div>
          <h2 id="secrets-title" class="text-sm font-semibold text-white">{pending.vaultExists ? $t('secrets.prompt.title') : $t('secrets.prompt.title_create')}</h2>
          <p class="text-xs text-slate-400">{$t('secrets.prompt.wants', { app: pending.client })}</p>
        </div>
      </div>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="password" bind:value={password} autofocus placeholder={$t('secrets.prompt.password')} class="w-full rounded-lg border border-white/10 bg-slate-900 px-3 py-2 text-sm text-white outline-none focus:border-blue-500" />
      {#if !pending.vaultExists}
        <input type="password" bind:value={confirm} placeholder={$t('secrets.prompt.confirm')} class="mt-2 w-full rounded-lg border border-white/10 bg-slate-900 px-3 py-2 text-sm text-white outline-none focus:border-blue-500" />
        <p class="mt-1.5 text-[11px] text-slate-500">{$t('secrets.prompt.create_hint')}</p>
      {/if}
      {#if error}<p class="mt-2 text-xs text-red-300" role="alert">{error}</p>{/if}
      <div class="mt-4 flex justify-end gap-2">
        <button type="button" class="rounded-lg bg-slate-700 px-4 py-2 text-sm text-white hover:bg-slate-600" onclick={dismiss}>{$t('secrets.prompt.deny')}</button>
        <button type="submit" disabled={busy || !password} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50">{pending.vaultExists ? $t('secrets.prompt.unlock') : $t('secrets.prompt.create')}</button>
      </div>
    </form>
  </div>
{/if}
