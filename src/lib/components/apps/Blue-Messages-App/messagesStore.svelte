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

  async function load() {
    loading.set(true);
    try {
      const items = await SystemBridge.messagesLoadConversations();
      conversations.set(items);
      await refreshMatrixSession();
      if (get(matrixLoggedIn)) matrixLoadRooms();
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
  };
}
