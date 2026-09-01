<script lang="ts">
  // Blue Accounts — password/account manager. See
  // src-tauri/src/BlueAccounts/crypto.rs's module doc for the actual
  // encryption scheme (Argon2id + AES-256-GCM) backing this UI — this
  // component never touches raw crypto, only calls the vault commands
  // and holds the master password in a local `let` for the duration of
  // the session (never persisted, never sent anywhere except as a
  // parameter on each vault-mutating call — see SystemBridge's
  // accounts* wrappers).
  import { onMount } from 'svelte';
  import {
    KeyRound, Plus, Trash2, Eye, EyeOff, Copy, Check, Lock, Unlock,
    Search, RefreshCw, Loader2, Shield, X,
  } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import LoadingSpinner from '../../../LoadingSpinner.svelte';
  import { newEntry, type VaultEntry } from './types';

  /// Focuses an element on mount without using the `autofocus`
  /// HTML attribute — `autofocus` steals focus unconditionally and
  /// unpredictably interacts with screen readers/tab order (hence
  /// svelte-check's own a11y warning against it); a small action that
  /// focuses once, right after the element exists in the DOM, avoids
  /// that while still giving the master-password field focus the
  /// moment the unlock screen appears.
  function autofocusAction(node: HTMLElement) {
    node.focus();
  }

  export let windowId: string;

  type Screen = 'loading' | 'create' | 'unlock' | 'vault';
  let screen: Screen = 'loading';

  let masterPassword = '';
  let masterPasswordConfirm = '';
  let unlockError = '';
  let busy = false;

  // Held only for the current session, in memory, to pass as the
  // required parameter on every mutating command (see vault.rs's
  // module doc's "Session model" — the backend itself never caches
  // this either).
  let sessionPassword = '';

  let entries: VaultEntry[] = [];
  let search = '';
  let selectedId: string | null = null;
  let editing: VaultEntry | null = null;
  let showPasswordFor = new Set<string>();
  let copiedField: string | null = null;

  let showGenerator = false;
  let genLength = 20;
  let genSymbols = true;
  let genDigits = true;
  let genUppercase = true;
  let generatedPassword = '';

  $: filteredEntries = entries.filter(
    (e) => !search.trim() || e.title.toLowerCase().includes(search.toLowerCase()) || e.username.toLowerCase().includes(search.toLowerCase())
  );
  $: selectedEntry = entries.find((e) => e.id === selectedId) ?? null;

  onMount(async () => {
    const exists = await SystemBridge.accountsVaultExists();
    const unlocked = exists && (await SystemBridge.accountsIsUnlocked());
    if (!exists) {
      screen = 'create';
    } else if (unlocked) {
      await loadEntries();
      screen = 'vault';
    } else {
      screen = 'unlock';
    }
  });

  async function loadEntries() {
    const res = await SystemBridge.accountsListEntries();
    if (res.ok && res.entries) entries = res.entries;
  }

  async function submitCreate() {
    unlockError = '';
    if (!masterPassword) { unlockError = 'Master password cannot be empty.'; return; }
    if (masterPassword !== masterPasswordConfirm) { unlockError = "Passwords don't match."; return; }
    busy = true;
    try {
      const res = await SystemBridge.accountsCreateVault(masterPassword);
      if (!res.ok) { unlockError = res.error ?? 'Failed to create vault'; return; }
      sessionPassword = masterPassword;
      masterPassword = '';
      masterPasswordConfirm = '';
      await loadEntries();
      screen = 'vault';
    } finally {
      busy = false;
    }
  }

  async function submitUnlock() {
    unlockError = '';
    busy = true;
    try {
      const res = await SystemBridge.accountsUnlock(masterPassword);
      if (!res.ok) { unlockError = 'Incorrect master password.'; return; }
      sessionPassword = masterPassword;
      masterPassword = '';
      await loadEntries();
      screen = 'vault';
    } finally {
      busy = false;
    }
  }

  async function lockVault() {
    await SystemBridge.accountsLock();
    sessionPassword = '';
    entries = [];
    selectedId = null;
    editing = null;
    showPasswordFor = new Set();
    screen = 'unlock';
  }

  function startNewEntry() {
    editing = newEntry();
    selectedId = null;
  }

  function startEdit(entry: VaultEntry) {
    editing = { ...entry };
  }

  async function saveEditing() {
    if (!editing || !editing.title.trim()) return;
    busy = true;
    try {
      editing.updatedAt = new Date().toISOString();
      const isNew = !entries.some((e) => e.id === editing!.id);
      const res = isNew
        ? await SystemBridge.accountsAddEntry(editing, sessionPassword)
        : await SystemBridge.accountsUpdateEntry(editing, sessionPassword);
      if (res.ok) {
        await loadEntries();
        selectedId = editing.id;
        editing = null;
      }
    } finally {
      busy = false;
    }
  }

  async function deleteEntry(id: string) {
    busy = true;
    try {
      const res = await SystemBridge.accountsDeleteEntry(id, sessionPassword);
      if (res.ok) {
        await loadEntries();
        if (selectedId === id) selectedId = null;
      }
    } finally {
      busy = false;
    }
  }

  function togglePasswordVisible(id: string) {
    const next = new Set(showPasswordFor);
    if (next.has(id)) next.delete(id); else next.add(id);
    showPasswordFor = next;
  }

  async function copyToClipboard(text: string, field: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedField = field;
      setTimeout(() => { if (copiedField === field) copiedField = null; }, 1500);
    } catch { /* clipboard API unavailable — non-fatal */ }
  }

  async function generatePassword() {
    generatedPassword = await SystemBridge.accountsGeneratePassword(genLength, genSymbols, genDigits, genUppercase);
  }

  function useGeneratedPassword() {
    if (editing && generatedPassword) editing.password = generatedPassword;
    showGenerator = false;
  }
</script>

<div class="relative flex h-full bg-slate-950 text-slate-100 text-sm">
  {#if screen === 'loading'}
    <LoadingSpinner label="Checking vault status…" />

  {:else if screen === 'create'}
    <div class="flex-1 flex items-center justify-center">
      <div class="w-80 flex flex-col gap-3">
        <div class="flex flex-col items-center gap-2 mb-2">
          <Shield class="w-10 h-10 text-blue-400" />
          <h1 class="font-medium text-lg">Set up Blue Accounts</h1>
          <p class="text-xs text-slate-500 text-center">
            Choose a master password. It encrypts your whole vault (Argon2id + AES-256-GCM) — if you forget it, your saved accounts cannot be recovered.
          </p>
        </div>
        <input
          type="password"
          bind:value={masterPassword}
          placeholder="Master password"
          class="bg-slate-800 border border-white/10 rounded px-3 py-2 text-sm outline-none focus:border-blue-500"
        />
        <input
          type="password"
          bind:value={masterPasswordConfirm}
          placeholder="Confirm master password"
          on:keydown={(e) => e.key === 'Enter' && submitCreate()}
          class="bg-slate-800 border border-white/10 rounded px-3 py-2 text-sm outline-none focus:border-blue-500"
        />
        {#if unlockError}<p class="text-xs text-red-400">{unlockError}</p>{/if}
        <button
          on:click={submitCreate}
          disabled={busy}
          class="bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded px-3 py-2 text-sm font-medium flex items-center justify-center gap-2"
        >
          {#if busy}<Loader2 class="w-4 h-4 animate-spin" />{/if} Create vault
        </button>
      </div>
    </div>

  {:else if screen === 'unlock'}
    <div class="flex-1 flex items-center justify-center">
      <div class="w-80 flex flex-col gap-3">
        <div class="flex flex-col items-center gap-2 mb-2">
          <Lock class="w-10 h-10 text-slate-400" />
          <h1 class="font-medium text-lg">Vault locked</h1>
        </div>
        <input
          type="password"
          bind:value={masterPassword}
          placeholder="Master password"
          use:autofocusAction
          on:keydown={(e) => e.key === 'Enter' && submitUnlock()}
          class="bg-slate-800 border border-white/10 rounded px-3 py-2 text-sm outline-none focus:border-blue-500"
        />
        {#if unlockError}<p class="text-xs text-red-400">{unlockError}</p>{/if}
        <button
          on:click={submitUnlock}
          disabled={busy || !masterPassword}
          class="bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded px-3 py-2 text-sm font-medium flex items-center justify-center gap-2"
        >
          {#if busy}<Loader2 class="w-4 h-4 animate-spin" />{:else}<Unlock class="w-4 h-4" />{/if} Unlock
        </button>
      </div>
    </div>

  {:else}
    <!-- Entry list -->
    <aside class="w-64 shrink-0 border-r border-white/10 flex flex-col">
      <div class="p-3 border-b border-white/10 flex items-center gap-2">
        <div class="relative flex-1">
          <Search class="w-3.5 h-3.5 absolute left-2 top-1/2 -translate-y-1/2 text-slate-500" />
          <input bind:value={search} placeholder="Search" class="w-full bg-slate-900 border border-white/10 rounded pl-7 pr-2 py-1.5 text-xs outline-none focus:border-blue-500" />
        </div>
        <button on:click={startNewEntry} class="shrink-0 w-7 h-7 flex items-center justify-center rounded bg-blue-600 hover:bg-blue-500" title="New entry">
          <Plus class="w-4 h-4" />
        </button>
      </div>
      <div class="flex-1 overflow-y-auto">
        {#if filteredEntries.length === 0}
          <div class="p-6 text-center text-slate-500 text-xs">No saved accounts yet.</div>
        {:else}
          {#each filteredEntries as entry (entry.id)}
            <button
              class="w-full text-left px-3 py-2.5 border-b border-white/5 hover:bg-white/5 transition-colors {entry.id === selectedId ? 'bg-white/10' : ''}"
              on:click={() => { selectedId = entry.id; editing = null; }}
            >
              <div class="font-medium truncate">{entry.title || '(untitled)'}</div>
              <div class="text-xs text-slate-500 truncate">{entry.username}</div>
            </button>
          {/each}
        {/if}
      </div>
      <div class="p-2 border-t border-white/10">
        <button on:click={lockVault} class="w-full flex items-center justify-center gap-1.5 py-1.5 rounded text-xs text-slate-400 hover:bg-white/5">
          <Lock class="w-3.5 h-3.5" /> Lock vault
        </button>
      </div>
    </aside>

    <!-- Detail / editor -->
    <div class="flex-1 flex flex-col min-w-0">
      {#if editing}
        <div class="p-4 flex flex-col gap-3 overflow-y-auto">
          <div class="flex items-center justify-between">
            <span class="font-medium">{entries.some((e) => e.id === editing?.id) ? 'Edit entry' : 'New entry'}</span>
            <button on:click={() => (editing = null)} class="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10"><X class="w-4 h-4" /></button>
          </div>
          <input bind:value={editing.title} placeholder="Title" class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500" />
          <input bind:value={editing.username} placeholder="Username / email" class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500" />
          <div class="flex gap-2">
            <input bind:value={editing.password} type="text" placeholder="Password" class="flex-1 bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500 font-mono" />
            <button on:click={() => (showGenerator = true)} class="shrink-0 px-2.5 rounded bg-slate-800 hover:bg-slate-700 text-xs flex items-center gap-1" title="Generate password">
              <RefreshCw class="w-3.5 h-3.5" />
            </button>
          </div>
          <input bind:value={editing.url} placeholder="URL" class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500" />
          <textarea bind:value={editing.notes} placeholder="Notes" rows="3" class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500 resize-none" />
          <button on:click={saveEditing} disabled={busy || !editing.title.trim()} class="bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded px-3 py-1.5 text-sm font-medium flex items-center justify-center gap-2">
            {#if busy}<Loader2 class="w-4 h-4 animate-spin" />{/if} Save
          </button>
        </div>
      {:else if selectedEntry}
        <div class="p-4 flex flex-col gap-3">
          <div class="flex items-center justify-between">
            <h2 class="font-medium text-base">{selectedEntry.title}</h2>
            <div class="flex gap-2">
              <button on:click={() => startEdit(selectedEntry)} class="px-2.5 py-1 text-xs bg-slate-800 hover:bg-slate-700 rounded">Edit</button>
              <button on:click={() => deleteEntry(selectedEntry.id)} class="px-2.5 py-1 text-xs bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded flex items-center gap-1"><Trash2 class="w-3 h-3" /> Delete</button>
            </div>
          </div>

          <div class="space-y-2">
            <div class="flex items-center justify-between bg-slate-900 border border-white/10 rounded px-3 py-2">
              <div class="min-w-0">
                <div class="text-[10px] text-slate-500 uppercase tracking-wide">Username</div>
                <div class="truncate">{selectedEntry.username || '—'}</div>
              </div>
              <button on:click={() => copyToClipboard(selectedEntry.username, 'username')} class="shrink-0 w-7 h-7 flex items-center justify-center rounded hover:bg-white/10">
                {#if copiedField === 'username'}<Check class="w-3.5 h-3.5 text-emerald-400" />{:else}<Copy class="w-3.5 h-3.5 text-slate-400" />{/if}
              </button>
            </div>

            <div class="flex items-center justify-between bg-slate-900 border border-white/10 rounded px-3 py-2">
              <div class="min-w-0">
                <div class="text-[10px] text-slate-500 uppercase tracking-wide">Password</div>
                <div class="truncate font-mono">{showPasswordFor.has(selectedEntry.id) ? selectedEntry.password : '••••••••••••'}</div>
              </div>
              <div class="flex items-center gap-1 shrink-0">
                <button on:click={() => togglePasswordVisible(selectedEntry.id)} class="w-7 h-7 flex items-center justify-center rounded hover:bg-white/10">
                  {#if showPasswordFor.has(selectedEntry.id)}<EyeOff class="w-3.5 h-3.5 text-slate-400" />{:else}<Eye class="w-3.5 h-3.5 text-slate-400" />{/if}
                </button>
                <button on:click={() => copyToClipboard(selectedEntry.password, 'password')} class="w-7 h-7 flex items-center justify-center rounded hover:bg-white/10">
                  {#if copiedField === 'password'}<Check class="w-3.5 h-3.5 text-emerald-400" />{:else}<Copy class="w-3.5 h-3.5 text-slate-400" />{/if}
                </button>
              </div>
            </div>

            {#if selectedEntry.url}
              <div class="bg-slate-900 border border-white/10 rounded px-3 py-2">
                <div class="text-[10px] text-slate-500 uppercase tracking-wide">URL</div>
                <a href={selectedEntry.url} target="_blank" rel="noreferrer" class="text-blue-400 hover:underline truncate block">{selectedEntry.url}</a>
              </div>
            {/if}
            {#if selectedEntry.notes}
              <div class="bg-slate-900 border border-white/10 rounded px-3 py-2">
                <div class="text-[10px] text-slate-500 uppercase tracking-wide">Notes</div>
                <p class="whitespace-pre-wrap text-slate-300">{selectedEntry.notes}</p>
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <div class="flex-1 flex items-center justify-center text-slate-500 text-sm flex-col gap-2">
          <KeyRound class="w-8 h-8 opacity-30" />
          Select an entry, or create a new one.
        </div>
      {/if}
    </div>
  {/if}

  {#if showGenerator}
    <div class="absolute inset-0 bg-black/50 flex items-center justify-center z-10">
      <div class="bg-slate-900 border border-white/10 rounded-lg w-80 p-4 flex flex-col gap-3">
        <div class="flex items-center justify-between">
          <span class="font-medium text-sm">Generate password</span>
          <button on:click={() => (showGenerator = false)} class="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10"><X class="w-4 h-4" /></button>
        </div>
        <div class="bg-slate-800 rounded px-3 py-2 font-mono text-sm break-all min-h-[2.5rem]">{generatedPassword || '—'}</div>
        <label class="flex items-center justify-between text-xs">
          <span>Length: {genLength}</span>
          <input type="range" min="8" max="64" bind:value={genLength} class="flex-1 ml-3" />
        </label>
        <label class="flex items-center gap-2 text-xs"><input type="checkbox" bind:checked={genUppercase} /> Uppercase letters</label>
        <label class="flex items-center gap-2 text-xs"><input type="checkbox" bind:checked={genDigits} /> Digits</label>
        <label class="flex items-center gap-2 text-xs"><input type="checkbox" bind:checked={genSymbols} /> Symbols</label>
        <button on:click={generatePassword} class="bg-slate-800 hover:bg-slate-700 rounded px-3 py-1.5 text-sm flex items-center justify-center gap-2">
          <RefreshCw class="w-3.5 h-3.5" /> Generate
        </button>
        <button on:click={useGeneratedPassword} disabled={!generatedPassword} class="bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded px-3 py-1.5 text-sm font-medium">
          Use this password
        </button>
      </div>
    </div>
  {/if}
</div>
