<script lang="ts">
  // Blue Messages — new app. Two-pane conversation list + thread view,
  // local-storage-backed (see src-tauri/src/BlueMessagesApp/mod.rs's
  // module doc for exactly what "local" means today and what a real
  // transport integration would add later).
  import { onMount } from 'svelte';
  import { Send, Plus, Pin, Trash2, MessageSquare, Search, X, Link2, LogOut, Loader2 } from 'lucide-svelte';
  import { createMessagesStore } from './messagesStore.svelte';
  import { CHANNEL_LABELS, type Channel } from './types';
  import { t } from '../../../stores/language';

  export let windowId: string;

  const store = createMessagesStore();
  const { conversations, activeId, thread, loading, sending, error, matrixLoggedIn, matrixBusy, matrixRooms, xmppLoggedIn, xmppBusy, smsAvailable, pairedPhones } = store;

  let search = '';
  let composeText = '';
  let showNewConversation = false;
  let newTitle = '';
  let newParticipant = '';
  /** For `newChannel === 'sms'`: which transport to use — a local modem
   * (ModemManager, the original path) or a specific paired phone via
   * Blue Connect (see sms.rs's two conversation-creation commands).
   * `null` means "use the modem". */
  let smsViaDeviceId: string | null = null;
  let newChannel: Channel = 'local';
  let threadEl: HTMLDivElement | null = null;

  let showMatrixLogin = false;
  let matrixHomeserver = 'https://matrix.org';
  let matrixUsername = '';
  let matrixPassword = '';
  let matrixLoginError = '';

  let showXmppLogin = false;
  let xmppJid = '';
  let xmppPassword = '';
  let xmppLoginError = '';

  async function submitXmppLogin() {
    xmppLoginError = '';
    const ok = await store.xmppLogin(xmppJid.trim(), xmppPassword);
    if (ok) {
      xmppPassword = ''; // never keep the typed password around longer than needed
      showXmppLogin = false;
    } else {
      xmppLoginError = 'Login failed — check your JID (user@domain) and password, and that the server supports STARTTLS + SASL PLAIN.';
    }
  }

  async function submitMatrixLogin() {
    matrixLoginError = '';
    const ok = await store.matrixLogin(matrixHomeserver.trim(), matrixUsername.trim(), matrixPassword);
    if (ok) {
      matrixPassword = ''; // never keep the typed password around longer than the one login call needs it
      showMatrixLogin = false;
    } else {
      matrixLoginError = 'Login failed — check your homeserver URL, username, and password.';
    }
  }

  async function importRoom(roomId: string, name: string) {
    await store.matrixImportRoom(roomId, name);
    showNewConversation = false;
  }

  $: filteredConversations = ($conversations || []).filter((c) =>
    !search.trim() || c.title.toLowerCase().includes(search.toLowerCase()) || c.participant.toLowerCase().includes(search.toLowerCase())
  );
  $: activeConversation = ($conversations || []).find((c) => c.id === $activeId) ?? null;

  onMount(() => { store.load(); });

  // Keep the thread scrolled to the newest message whenever it changes
  // (new message sent/received, or switching conversations) — a chat
  // view that doesn't auto-scroll reads as broken, not calm.
  $: if (threadEl && $thread) {
    queueMicrotask(() => { if (threadEl) threadEl.scrollTop = threadEl.scrollHeight; });
  }

  async function handleSend() {
    const body = composeText;
    composeText = '';
    await store.send(body);
  }

  function handleComposeKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  async function submitNewConversation() {
    if (!newTitle.trim()) return;
    const participant = newParticipant.trim() || newTitle.trim();
    if (newChannel === 'sms' && smsViaDeviceId) {
      await store.smsAddPhoneContact(smsViaDeviceId, participant, newTitle.trim());
    } else {
      await store.createConversation(newTitle.trim(), participant, newChannel);
    }
    newTitle = '';
    newParticipant = '';
    newChannel = 'local';
    smsViaDeviceId = null;
    showNewConversation = false;
  }

  function formatTime(iso: string): string {
    try {
      const d = new Date(iso);
      return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    } catch { return ''; }
  }
  function formatDay(iso: string): string {
    try {
      return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
    } catch { return ''; }
  }
</script>

<div class="relative flex h-full bg-slate-950 text-slate-100 text-sm">
  <!-- Conversation list -->
  <aside class="w-72 shrink-0 border-r border-white/10 flex flex-col">
    <div class="p-3 border-b border-white/10 flex items-center gap-2">
      <div class="relative flex-1">
        <Search class="w-3.5 h-3.5 absolute left-2 top-1/2 -translate-y-1/2 text-slate-500" />
        <input
          bind:value={search}
          placeholder="Search"
          class="w-full bg-slate-900 border border-white/10 rounded pl-7 pr-2 py-1.5 text-xs outline-none focus:border-blue-500"
        />
      </div>
      <button
        on:click={() => (showNewConversation = true)}
        class="shrink-0 w-7 h-7 flex items-center justify-center rounded bg-blue-600 hover:bg-blue-500 transition-colors"
        title="New conversation"
      >
        <Plus class="w-4 h-4" />
      </button>
      <button
        on:click={() => ($matrixLoggedIn ? store.matrixLogout() : (showMatrixLogin = true))}
        class="shrink-0 w-7 h-7 flex items-center justify-center rounded transition-colors {$matrixLoggedIn ? 'bg-emerald-600/30 text-emerald-300 hover:bg-emerald-600/40' : 'bg-slate-800 hover:bg-slate-700 text-slate-400'}"
        title={$matrixLoggedIn ? 'Connected to Matrix — click to log out' : 'Connect a Matrix account'}
      >
        {#if $matrixBusy}<Loader2 class="w-4 h-4 animate-spin" />{:else if $matrixLoggedIn}<LogOut class="w-4 h-4" />{:else}<Link2 class="w-4 h-4" />{/if}
      </button>
      <button
        on:click={() => ($xmppLoggedIn ? store.xmppLogout() : (showXmppLogin = true))}
        class="shrink-0 w-7 h-7 flex items-center justify-center rounded transition-colors {$xmppLoggedIn ? 'bg-emerald-600/30 text-emerald-300 hover:bg-emerald-600/40' : 'bg-slate-800 hover:bg-slate-700 text-slate-400'}"
        title={$xmppLoggedIn ? 'Connected to XMPP — click to log out' : 'Connect an XMPP account'}
      >
        {#if $xmppBusy}<Loader2 class="w-4 h-4 animate-spin" />{:else if $xmppLoggedIn}<LogOut class="w-4 h-4" />{:else}<Link2 class="w-4 h-4" />{/if}
      </button>
    </div>

    <div class="flex-1 overflow-y-auto">
      {#if $loading}
        <div class="p-4 text-center text-slate-500 text-xs">Loading…</div>
      {:else if filteredConversations.length === 0}
        <div class="p-6 text-center text-slate-500 text-xs flex flex-col items-center gap-2">
          <MessageSquare class="w-6 h-6 opacity-40" />
          No conversations yet.
        </div>
      {:else}
        {#each filteredConversations as c (c.id)}
          <button
            class="w-full text-left px-3 py-2.5 border-b border-white/5 hover:bg-white/5 transition-colors flex items-start gap-2 group {c.id === $activeId ? 'bg-white/10' : ''}"
            on:click={() => store.openConversation(c.id)}
          >
            <div class="w-8 h-8 rounded-full bg-blue-600/30 flex items-center justify-center text-xs font-medium shrink-0 mt-0.5">
              {c.title.slice(0, 1).toUpperCase()}
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center justify-between gap-1">
                <span class="font-medium truncate flex items-center gap-1">
                  {#if c.pinned}<Pin class="w-3 h-3 text-blue-400 shrink-0" />{/if}
                  {c.title}
                </span>
                <span class="text-[10px] text-slate-500 shrink-0">{formatDay(c.lastMessageAt)}</span>
              </div>
              <div class="flex items-center justify-between gap-1 mt-0.5">
                <span class="text-xs text-slate-400 truncate">{c.lastMessagePreview || CHANNEL_LABELS[c.channel]}</span>
                {#if c.unreadCount > 0}
                  <span class="shrink-0 min-w-[16px] h-4 px-1 rounded-full bg-blue-600 text-[10px] flex items-center justify-center">{c.unreadCount}</span>
                {/if}
              </div>
            </div>
            <div class="opacity-0 group-hover:opacity-100 transition-opacity flex flex-col gap-1 shrink-0">
              <button
                class="w-5 h-5 flex items-center justify-center rounded hover:bg-white/10"
                title={c.pinned ? 'Unpin' : 'Pin'}
                on:click|stopPropagation={() => store.togglePinned(c.id)}
              >
                <Pin class="w-3 h-3 {c.pinned ? 'text-blue-400' : 'text-slate-500'}" />
              </button>
              <button
                class="w-5 h-5 flex items-center justify-center rounded hover:bg-red-500/20"
                title="Delete"
                on:click|stopPropagation={() => store.deleteConversation(c.id)}
              >
                <Trash2 class="w-3 h-3 text-slate-500 hover:text-red-400" />
              </button>
            </div>
          </button>
        {/each}
      {/if}
    </div>
  </aside>

  <!-- Thread -->
  <div class="flex-1 flex flex-col min-w-0">
    {#if activeConversation}
      <div class="px-4 py-3 border-b border-white/10 flex items-center justify-between">
        <div>
          <div class="font-medium">{activeConversation.title}</div>
          <div class="text-[11px] text-slate-500">{CHANNEL_LABELS[activeConversation.channel]} · {activeConversation.participant}</div>
        </div>
      </div>

      <div bind:this={threadEl} class="flex-1 overflow-y-auto px-4 py-3 flex flex-col gap-2">
        {#each $thread as m (m.id)}
          <div class="flex {m.direction === 'outgoing' ? 'justify-end' : 'justify-start'}">
            <div
              class="max-w-[70%] rounded-2xl px-3 py-2 text-sm {m.direction === 'outgoing'
                ? 'bg-blue-600 text-white rounded-br-sm'
                : 'bg-slate-800 text-slate-100 rounded-bl-sm'}"
            >
              <div class="whitespace-pre-wrap break-words">{m.body}</div>
              <div class="text-[10px] opacity-60 mt-1 text-right">{formatTime(m.sentAt)}</div>
            </div>
          </div>
        {/each}
      </div>

      <div class="p-3 border-t border-white/10 flex items-end gap-2">
        <textarea
          bind:value={composeText}
          on:keydown={handleComposeKeydown}
          rows="1"
          placeholder="Type a message… (Enter to send, Shift+Enter for a new line)"
          class="flex-1 resize-none bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm outline-none focus:border-blue-500 max-h-28"
        />
        <button
          on:click={handleSend}
          disabled={!composeText.trim() || $sending}
          class="shrink-0 w-9 h-9 flex items-center justify-center rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          <Send class="w-4 h-4" />
        </button>
      </div>
    {:else}
      <div class="flex-1 flex items-center justify-center text-slate-500 text-sm flex-col gap-2">
        <MessageSquare class="w-8 h-8 opacity-30" />
        Select or start a conversation.
      </div>
    {/if}
  </div>

  {#if showNewConversation}
    <div class="absolute inset-0 bg-black/50 flex items-center justify-center z-10">
      <div class="bg-slate-900 border border-white/10 rounded-lg w-80 p-4 flex flex-col gap-3 max-h-[80%] overflow-y-auto">
        <div class="flex items-center justify-between">
          <span class="font-medium text-sm">New conversation</span>
          <button on:click={() => (showNewConversation = false)} class="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10">
            <X class="w-4 h-4" />
          </button>
        </div>

        <select bind:value={newChannel} class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500">
          {#each Object.entries(CHANNEL_LABELS) as [value, label]}
            <option {value}>{label}</option>
          {/each}
        </select>

        {#if newChannel === 'matrix'}
          {#if !$matrixLoggedIn}
            <p class="text-[11px] text-slate-400">
              Not connected to Matrix yet.
              <button class="text-blue-400 hover:underline" on:click={() => { showNewConversation = false; showMatrixLogin = true; }}>Connect an account</button>
              first.
            </p>
          {:else}
            <p class="text-[11px] text-slate-500">Pick a joined room to import as a conversation:</p>
            {#if $matrixBusy}
              <div class="flex items-center gap-2 text-xs text-slate-400 py-3 justify-center"><Loader2 class="w-3.5 h-3.5 animate-spin" /> Loading rooms…</div>
            {:else if $matrixRooms.length === 0}
              <p class="text-xs text-slate-500 py-2">No joined rooms found (or none loaded yet).</p>
              <button on:click={() => store.matrixLoadRooms()} class="text-xs text-blue-400 hover:underline self-start">Refresh room list</button>
            {:else}
              <div class="flex flex-col gap-1 max-h-48 overflow-y-auto">
                {#each $matrixRooms as room (room.roomId)}
                  <button
                    on:click={() => importRoom(room.roomId, room.name)}
                    class="text-left px-2 py-1.5 rounded bg-slate-800 hover:bg-slate-700 text-xs truncate transition-colors"
                  >
                    {room.name}
                  </button>
                {/each}
              </div>
            {/if}
          {/if}
        {:else if newChannel === 'xmpp' && !$xmppLoggedIn}
          <p class="text-[11px] text-slate-400">
            Not connected to XMPP yet.
            <button class="text-blue-400 hover:underline" on:click={() => { showNewConversation = false; showXmppLogin = true; }}>Connect an account</button>
            first.
          </p>
        {:else}
          <input
            bind:value={newTitle}
            placeholder="Title"
            class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500"
          />
          <input
            bind:value={newParticipant}
            placeholder={newChannel === 'xmpp' ? 'Contact JID (user@domain)' : newChannel === 'sms' ? 'Phone number' : 'Participant'}
            class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500"
          />
          {#if newChannel === 'sms' && !$smsAvailable && $pairedPhones.length === 0}
            <p class="text-[11px] text-amber-400/80">
              No modem detected via ModemManager and no paired phones via Blue Connect — messages will be saved locally but won't actually send until one is available (see sms.rs).
            </p>
          {:else if newChannel === 'sms'}
            {#if $pairedPhones.length > 0}
              <label class="text-[11px] text-slate-500">Send through</label>
              <select bind:value={smsViaDeviceId} class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500">
                <option value={null}>{$smsAvailable ? 'Local modem (ModemManager)' : 'Local modem (none detected)'}</option>
                {#each $pairedPhones as phone (phone.deviceId)}
                  <option value={phone.deviceId}>Paired phone: {phone.deviceName}</option>
                {/each}
              </select>
              {#if smsViaDeviceId}
                <p class="text-[11px] text-slate-500">
                  Relayed through the paired phone over Blue Connect — see sms.rs's send_sms_via_phone doc for what's and isn't guaranteed to work.
                </p>
              {/if}
            {/if}
          {:else if newChannel === 'xmpp'}
            <p class="text-[11px] text-slate-500">
              Messages will send/receive over a real (but not always-on — see xmpp.rs) connection to your XMPP server.
            </p>
          {/if}
          <button
            on:click={submitNewConversation}
            disabled={!newTitle.trim()}
            class="bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded px-3 py-1.5 text-sm font-medium transition-colors"
          >
            Create
          </button>
        {/if}
      </div>
    </div>
  {/if}

  {#if showMatrixLogin}
    <div class="absolute inset-0 bg-black/50 flex items-center justify-center z-10">
      <div class="bg-slate-900 border border-white/10 rounded-lg w-80 p-4 flex flex-col gap-3">
        <div class="flex items-center justify-between">
          <span class="font-medium text-sm flex items-center gap-1.5"><Link2 class="w-3.5 h-3.5" /> Connect Matrix account</span>
          <button on:click={() => (showMatrixLogin = false)} class="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10">
            <X class="w-4 h-4" />
          </button>
        </div>
        <p class="text-[11px] text-slate-500">
          Real login against your homeserver — password isn't stored, only the session token (see matrix.rs). No end-to-end encryption support yet.
        </p>
        <input
          bind:value={matrixHomeserver}
          placeholder="Homeserver (e.g. https://matrix.org)"
          class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500"
        />
        <input
          bind:value={matrixUsername}
          placeholder="Username"
          class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500"
        />
        <input
          bind:value={matrixPassword}
          type="password"
          placeholder="Password"
          on:keydown={(e) => e.key === 'Enter' && submitMatrixLogin()}
          class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500"
        />
        {#if matrixLoginError}
          <p class="text-[11px] text-red-400">{matrixLoginError}</p>
        {/if}
        <button
          on:click={submitMatrixLogin}
          disabled={!matrixHomeserver.trim() || !matrixUsername.trim() || !matrixPassword || $matrixBusy}
          class="bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded px-3 py-1.5 text-sm font-medium transition-colors flex items-center justify-center gap-2"
        >
          {#if $matrixBusy}<Loader2 class="w-3.5 h-3.5 animate-spin" />{/if} Log in
        </button>
      </div>
    </div>
  {/if}

  {#if showXmppLogin}
    <div class="absolute inset-0 bg-black/50 flex items-center justify-center z-10">
      <div class="bg-slate-900 border border-white/10 rounded-lg w-80 p-4 flex flex-col gap-3">
        <div class="flex items-center justify-between">
          <span class="font-medium text-sm flex items-center gap-1.5"><Link2 class="w-3.5 h-3.5" /> Connect XMPP account</span>
          <button on:click={() => (showXmppLogin = false)} class="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10">
            <X class="w-4 h-4" />
          </button>
        </div>
        <p class="text-[11px] text-slate-500">
          Real STARTTLS + SASL PLAIN login (see xmpp.rs) — the password is kept, encrypted at rest, since XMPP sends happen over fresh
          ephemeral connections rather than a long-lived session token like Matrix's.
        </p>
        <input
          bind:value={xmppJid}
          placeholder="JID (e.g. you@example.org)"
          class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500"
        />
        <input
          bind:value={xmppPassword}
          type="password"
          placeholder="Password"
          on:keydown={(e) => e.key === 'Enter' && submitXmppLogin()}
          class="bg-slate-800 border border-white/10 rounded px-2 py-1.5 text-sm outline-none focus:border-blue-500"
        />
        {#if xmppLoginError}
          <p class="text-[11px] text-red-400">{xmppLoginError}</p>
        {/if}
        <button
          on:click={submitXmppLogin}
          disabled={!xmppJid.trim() || !xmppPassword || $xmppBusy}
          class="bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded px-3 py-1.5 text-sm font-medium transition-colors flex items-center justify-center gap-2"
        >
          {#if $xmppBusy}<Loader2 class="w-3.5 h-3.5 animate-spin" />{/if} Log in
        </button>
      </div>
    </div>
  {/if}

  {#if $error}
    <div class="absolute bottom-3 right-3 bg-red-500/90 text-white text-xs px-3 py-2 rounded shadow-lg">{$error}</div>
  {/if}
</div>
