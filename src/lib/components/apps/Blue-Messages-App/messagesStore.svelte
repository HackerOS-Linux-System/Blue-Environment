<script context="module" lang="ts">
  // NOTE: this file is a plain store module (no markup, no component
  // instance) — the only reason it carries a .svelte extension instead
  // of .ts (like its sibling Blue-Docs-App/document.ts) is to match
  // whatever the original convention here was; wrapped in a
  // `context="module"` script block so its named exports work exactly
  // like a plain TS module import (`import { createMessagesStore } from
  // './messagesStore'`) while keeping the file's name/path unchanged.
  // Previously this file had bare top-level `import`/`export` statements
  // with no `<script>` wrapper at all, which is invalid Svelte syntax
  // (svelte-check: "Unexpected token" — the compiler expects markup or a
  // script block, not a raw JS/TS module body) and made the whole file
  // fail to parse.
import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { Conversation, Message, Channel } from './types';

export function createMessagesStore() {
  const conversations = writable<Conversation[]>([]);
  const activeId = writable<string | null>(null);
  const thread = writable<Message[]>([]);
  const loading = writable(true);
  const sending = writable(false);
  const error = writable<string | null>(null);

  // Matrix session state — see matrix.rs's module doc for what "real"
  // means here (real login/send/receive, pull-based, no E2EE).
  const matrixLoggedIn = writable(false);
  const matrixBusy = writable(false);
  const matrixRooms = writable<{ roomId: string; name: string }[]>([]);

  // XMPP session state — see xmpp.rs's module doc for what "real" means
  // here (real STARTTLS+SASL PLAIN+bind+send, ephemeral connection per
  // action, no persistent/live connection).
  const xmppLoggedIn = writable(false);
  const xmppBusy = writable(false);

  // SMS (ModemManager) availability — see sms.rs's module doc. `false`
  // just means "no modem visible to ModemManager right now", not an
  // error; the UI uses this to decide whether to offer an SMS option.
  const smsAvailable = writable(false);
  // Paired phones (via Blue Connect) an SMS conversation can relay
  // through instead of a local modem — see sms.rs's
  // sms_list_paired_phones/send_sms_via_phone doc.
  const pairedPhones = writable<{ deviceId: string; deviceName: string }[]>([]);

  async function refreshMatrixSession() {
    matrixLoggedIn.set(await SystemBridge.matrixHasSession());
  }

  async function matrixLogin(homeserver: string, username: string, password: string) {
    matrixBusy.set(true);
    try {
      const res = await SystemBridge.matrixLogin(homeserver, username, password);
      if (!res.ok) { error.set(res.error ?? 'Matrix login failed'); return false; }
      matrixLoggedIn.set(true);
      await matrixLoadRooms();
      return true;
    } finally {
      matrixBusy.set(false);
    }
  }

  async function matrixLogout() {
    await SystemBridge.matrixLogout();
    matrixLoggedIn.set(false);
    matrixRooms.set([]);
  }

  async function matrixLoadRooms() {
    matrixBusy.set(true);
    try {
      matrixRooms.set(await SystemBridge.matrixListRooms());
    } finally {
      matrixBusy.set(false);
    }
  }

  async function matrixImportRoom(roomId: string, name: string) {
    const res = await SystemBridge.matrixImportRoom(roomId, name);
    if (!res.ok || !res.conversation) {
      error.set(res.error ?? 'Failed to import Matrix room');
      return null;
    }
    conversations.update((list) => [res.conversation!, ...list]);
    await openConversation(res.conversation.id);
    return res.conversation;
  }

  /// Pulls the latest messages for the active conversation if it's a
  /// Matrix one — see `matrixRefreshThread`'s own doc in matrix.rs for
  /// why this is a manual/on-open pull instead of a live stream.
  async function refreshIfMatrix(id: string) {
    const convo = get(conversations).find((c) => c.id === id);
    if (!convo || convo.channel !== 'matrix') return;
    matrixBusy.set(true);
    try {
      const updated = await SystemBridge.matrixRefreshThread(id);
      if (get(activeId) === id) thread.set(updated);
    } finally {
      matrixBusy.set(false);
    }
  }

  async function refreshXmppSession() {
    xmppLoggedIn.set(await SystemBridge.xmppHasSession());
  }

  async function xmppLogin(jid: string, password: string) {
    xmppBusy.set(true);
    try {
      const res = await SystemBridge.xmppLogin(jid, password);
      if (!res.ok) { error.set(res.error ?? 'XMPP login failed'); return false; }
      xmppLoggedIn.set(true);
      return true;
    } finally {
      xmppBusy.set(false);
    }
  }

  async function xmppLogout() {
    await SystemBridge.xmppLogout();
    xmppLoggedIn.set(false);
  }

  async function xmppAddContact(contactJid: string, name: string) {
    const res = await SystemBridge.xmppAddContact(contactJid, name);
    if (!res.ok || !res.conversation) {
      error.set(res.error ?? 'Failed to add XMPP contact');
      return null;
    }
    conversations.update((list) => [res.conversation!, ...list]);
    await openConversation(res.conversation.id);
    return res.conversation;
  }

  /// Same pull-on-open shape as `refreshIfMatrix` — see `xmpp_refresh_thread`'s
  /// doc in xmpp.rs for exactly what this ephemeral-connection poll does
  /// and doesn't cover (no live/persistent connection).
  async function refreshIfXmpp(id: string) {
    const convo = get(conversations).find((c) => c.id === id);
    if (!convo || convo.channel !== 'xmpp') return;
    xmppBusy.set(true);
    try {
      const updated = await SystemBridge.xmppRefreshThread(id);
      if (get(activeId) === id) thread.set(updated);
    } finally {
      xmppBusy.set(false);
    }
  }

  async function refreshSmsAvailability() {
    smsAvailable.set(await SystemBridge.smsModemAvailable());
    pairedPhones.set(await SystemBridge.smsListPairedPhones());
  }

  async function smsAddContact(phoneNumber: string, name: string) {
    const res = await SystemBridge.smsAddContact(phoneNumber, name);
    if (!res.ok || !res.conversation) {
      error.set(res.error ?? 'Failed to add SMS contact');
      return null;
    }
    conversations.update((list) => [res.conversation!, ...list]);
    await openConversation(res.conversation.id);
    return res.conversation;
  }

  /// Same as `smsAddContact` but relayed through a paired phone via
  /// Blue Connect instead of a local modem — see sms.rs's
  /// sms_add_phone_contact doc.
  async function smsAddPhoneContact(deviceId: string, phoneNumber: string, name: string) {
    const res = await SystemBridge.smsAddPhoneContact(deviceId, phoneNumber, name);
    if (!res.ok || !res.conversation) {
      error.set(res.error ?? 'Failed to add phone-relayed SMS contact');
      return null;
    }
    conversations.update((list) => [res.conversation!, ...list]);
    await openConversation(res.conversation.id);
    return res.conversation;
  }

  /// Same shape again for the ModemManager-backed SMS channel — see
  /// `sms_refresh_thread`'s doc in sms.rs.
  async function refreshIfSms(id: string) {
    const convo = get(conversations).find((c) => c.id === id);
    if (!convo || convo.channel !== 'sms') return;
    try {
      const updated = await SystemBridge.smsRefreshThread(id);
      if (get(activeId) === id) thread.set(updated);
    } catch { /* no modem / ModemManager unavailable — thread just stays as-is */ }
  }

  /// Subscribes to the background XMPP connection's live-push event
  /// (see xmpp.rs's `run_receive_loop`/`handle_incoming_message`) so a
  /// message arriving while the app is open shows up immediately —
  /// without this, XMPP would still only update via the manual
  /// `refreshIfXmpp` pull-on-open path despite the backend now having a
  /// real persistent connection to push from. No-ops outside Tauri.
  async function subscribeXmppIncoming() {
    if (!SystemBridge.isTauri()) return;
    try {
      const mod = await import('@tauri-apps/api/event');
      await mod.listen('blue-messages://xmpp-incoming', (e: any) => {
        const { conversationId, message } = e.payload ?? {};
        if (!conversationId || !message) return;
        conversations.update((list) =>
          list.map((c) =>
            c.id === conversationId
              ? { ...c, lastMessagePreview: message.body, lastMessageAt: message.sentAt, unreadCount: (get(activeId) === conversationId ? 0 : c.unreadCount + 1) }
              : c
          )
        );
        if (get(activeId) === conversationId) {
          thread.update((msgs) => [...msgs, message]);
        }
      });
    } catch {
      /* not running under Tauri, or the event API isn't available — the
         manual refresh-on-open path still works either way */
    }
  }

  async function load() {
    loading.set(true);
    try {
      const items = await SystemBridge.messagesLoadConversations();
      conversations.set(items);
      await refreshMatrixSession();
      if (get(matrixLoggedIn)) matrixLoadRooms();
      await refreshXmppSession();
      await refreshSmsAvailability();
      subscribeXmppIncoming();
      // Auto-open the first (pinned/most-recent, per the backend's own
      // sort — see messages_load_conversations) conversation so the
      // window never opens to a blank two-pane view on first launch.
      if (items.length > 0 && get(activeId) === null) {
        await openConversation(items[0].id);
      }
    } finally {
      loading.set(false);
    }
  }

  async function openConversation(id: string) {
    activeId.set(id);
    thread.set(await SystemBridge.messagesLoadThread(id));
    await SystemBridge.messagesMarkRead(id);
    conversations.update((list) => list.map((c) => (c.id === id ? { ...c, unreadCount: 0 } : c)));
    refreshIfMatrix(id); // fire-and-forget — thread already shows local history immediately
    refreshIfXmpp(id);
    refreshIfSms(id);
  }

  async function createConversation(title: string, participant: string, channel: Channel) {
    const res = await SystemBridge.messagesCreateConversation(title, participant, channel);
    if (!res.ok || !res.conversation) {
      error.set(res.error ?? 'Failed to create conversation');
      return null;
    }
    conversations.update((list) => [res.conversation!, ...list]);
    await openConversation(res.conversation.id);
    return res.conversation;
  }

  async function deleteConversation(id: string) {
    const res = await SystemBridge.messagesDeleteConversation(id);
    if (!res.ok) { error.set(res.error ?? 'Failed to delete conversation'); return; }
    conversations.update((list) => list.filter((c) => c.id !== id));
    if (get(activeId) === id) {
      activeId.set(null);
      thread.set([]);
      const remaining = get(conversations);
      if (remaining.length > 0) await openConversation(remaining[0].id);
    }
  }

  async function togglePinned(id: string) {
    const current = get(conversations).find((c) => c.id === id);
    if (!current) return;
    const next = !current.pinned;
    conversations.update((list) => list.map((c) => (c.id === id ? { ...c, pinned: next } : c)));
    await SystemBridge.messagesSetPinned(id, next);
    // Re-sort locally to match the backend's pinned-first ordering
    // without a full reload round-trip.
    conversations.update((list) => [...list].sort((a, b) => (Number(b.pinned) - Number(a.pinned)) || b.lastMessageAt.localeCompare(a.lastMessageAt)));
  }

  async function send(body: string) {
    const id = get(activeId);
    if (!id || !body.trim()) return;
    sending.set(true);
    try {
      const res = await SystemBridge.messagesSend(id, body.trim());
      if (!res.ok || !res.message) { error.set(res.error ?? 'Failed to send'); return; }
      thread.update((msgs) => [...msgs, res.message!]);
      conversations.update((list) =>
        list.map((c) => (c.id === id ? { ...c, lastMessagePreview: body.trim(), lastMessageAt: res.message!.sentAt } : c))
      );
    } finally {
      sending.set(false);
    }
  }

  return {
    conversations, activeId, thread, loading, sending, error,
    load, openConversation, createConversation, deleteConversation, togglePinned, send,
    matrixLoggedIn, matrixBusy, matrixRooms,
    matrixLogin, matrixLogout, matrixLoadRooms, matrixImportRoom,
    xmppLoggedIn, xmppBusy, xmppLogin, xmppLogout, xmppAddContact,
    smsAvailable, smsAddContact, pairedPhones, smsAddPhoneContact,
  };
}
</script>
