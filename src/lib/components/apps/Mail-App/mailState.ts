import { writable, derived, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import { dialogAlert } from '../../../stores/dialog';
import { DEMO_EMAILS } from './demoData';
import { EMPTY_COMPOSE } from './types';
import type { Email, ComposeData, FolderType, ReplyMode } from './types';

interface RemoteEmail { uid: string; from: string; to: string; subject: string; date: string; body: string; read: boolean; }

function remoteToEmail(r: RemoteEmail, accountId: string): Email {
  return {
    id: `imap-${accountId}-${r.uid}`,
    from: { name: r.from.split('<')[0].trim() || r.from, email: r.from.match(/<(.+)>/)?.[1] ?? r.from },
    to: [{ name: r.to, email: r.to }],
    subject: r.subject || '(no subject)',
    body: r.body || '',
    date: r.date ? new Date(r.date) : new Date(),
    read: r.read,
    starred: false,
    labels: [],
    folder: 'inbox',
  };
}

export function createMailState() {
  const emails = writable<Email[]>(DEMO_EMAILS);
  const activeFolder = writable<FolderType>('inbox');
  const selectedEmail = writable<Email | null>(null);
  const composeOpen = writable(false);
  const composeData = writable<ComposeData>(EMPTY_COMPOSE);
  const searchQuery = writable('');
  const loading = writable(false);
  const selectedIds = writable<string[]>([]);
  const showCc = writable(false);
  const replyMode = writable<ReplyMode>(null);
  const syncStatus = writable('');

  async function fetchFromServer(accountId: string, folder: FolderType = 'inbox') {
    if (!SystemBridge.isTauri() || !accountId) return;
    syncStatus.set('Syncing…');
    loading.set(true);
    try {
      const raw = await SystemBridge.invokeCommand<RemoteEmail[]>('mail_fetch_inbox', { accountId, folder: folder.toUpperCase(), limit: 50 });
      if (raw?.length) {
        const fetched = raw.map((r) => remoteToEmail(r, accountId));
        emails.update((prev) => [...prev.filter((e) => !e.id.startsWith(`imap-${accountId}-`)), ...fetched]);
        syncStatus.set(`Synced ${fetched.length} messages`);
      } else {
        syncStatus.set('No messages or connection failed');
      }
    } catch (e: any) {
      syncStatus.set(`Sync error: ${e?.message ?? e}`);
    } finally {
      loading.set(false);
      setTimeout(() => syncStatus.set(''), 4000);
    }
  }

  async function sendViaBackend(accountId: string, to: string, cc: string, subject: string, body: string): Promise<boolean> {
    if (!SystemBridge.isTauri() || !accountId) return false;
    const result = await SystemBridge.invokeCommand<{ success: boolean; error?: string }>('mail_send', { accountId, to, cc: cc || null, subject, body });
    if (!result?.success && result?.error) { await dialogAlert({ title: 'Send failed', message: result.error }); return false; }
    return !!result?.success;
  }

  function unreadCount(folder: FolderType) { return get(emails).filter((e) => e.folder === folder && !e.read).length; }

  const totalUnread = derived(emails, ($emails) => $emails.filter((e) => e.folder === 'inbox' && !e.read).length);

  const displayEmails = derived([emails, activeFolder, searchQuery], ([$emails, $activeFolder, $searchQuery]) => {
    const q = $searchQuery.toLowerCase();
    return $emails
      .filter((e) => {
        const matchFolder = e.folder === $activeFolder;
        const matchSearch = !q || e.subject.toLowerCase().includes(q) || e.from.name.toLowerCase().includes(q) || e.from.email.toLowerCase().includes(q) || e.body.toLowerCase().includes(q);
        return matchFolder && matchSearch;
      })
      .sort((a, b) => b.date.getTime() - a.date.getTime());
  });

  function markRead(id: string, read = true) { emails.update((prev) => prev.map((e) => (e.id === id ? { ...e, read } : e))); }
  function toggleStar(id: string) { emails.update((prev) => prev.map((e) => (e.id === id ? { ...e, starred: !e.starred } : e))); }
  function moveToFolder(id: string, folder: FolderType) {
    emails.update((prev) => prev.map((e) => (e.id === id ? { ...e, folder } : e)));
    if (get(selectedEmail)?.id === id) selectedEmail.set(null);
  }
  function deleteEmail(id: string) {
    const email = get(emails).find((e) => e.id === id);
    if (!email) return;
    if (email.folder === 'trash') emails.update((prev) => prev.filter((e) => e.id !== id));
    else moveToFolder(id, 'trash');
    if (get(selectedEmail)?.id === id) selectedEmail.set(null);
  }
  function selectEmail(email: Email) {
    selectedEmail.set(email);
    if (!email.read) markRead(email.id);

    // IMAP-sourced messages (id format `imap-{accountId}-{uid}`) come back
    // from `mail_fetch_inbox` with an empty `body` — headers only, see the
    // comment on `mail_fetch_inbox` in MailApp/mod.rs for why. Fetch the
    // actual body on demand now that the user is actually reading this
    // message, rather than fetching every body up front.
    const m = email.id.match(/^imap-(.+)-([^-]+)$/);
    if (m && !email.body && SystemBridge.isTauri()) {
      const [, accountId, uid] = m;
      SystemBridge.invokeCommand<string>('mail_fetch_body', { accountId, uid })
        .then((body) => {
          if (!body) return;
          emails.update((prev) => prev.map((e) => (e.id === email.id ? { ...e, body } : e)));
          selectedEmail.update((cur) => (cur && cur.id === email.id ? { ...cur, body } : cur));
        })
        .catch(() => {});
    }
  }

  function openCompose() { composeOpen.set(true); composeData.set(EMPTY_COMPOSE); replyMode.set(null); showCc.set(false); }
  function closeCompose() { composeOpen.set(false); }

  async function sendEmail(accountId?: string | null) {
    const data = get(composeData);
    if (!data.to.trim() || !data.subject.trim()) {
      await dialogAlert({ title: 'Missing information', message: 'Please fill in the To and Subject fields before sending.' });
      return;
    }
    if (accountId && SystemBridge.isTauri()) {
      const sent = await sendViaBackend(accountId, data.to, data.cc, data.subject, data.body);
      if (!sent) return;
    }
    const newEmail: Email = {
      id: Date.now().toString(),
      from: { name: SystemBridge.getUsername(), email: 'user@blue.env' },
      to: [{ name: data.to, email: data.to }],
      subject: data.subject, body: data.body,
      date: new Date(), read: true, starred: false, labels: [], folder: 'sent',
    };
    emails.update((prev) => [newEmail, ...prev]);
    composeOpen.set(false); composeData.set(EMPTY_COMPOSE); replyMode.set(null); showCc.set(false);
  }

  function saveDraft() {
    const data = get(composeData);
    const draft: Email = {
      id: Date.now().toString(),
      from: { name: SystemBridge.getUsername(), email: 'user@blue.env' },
      to: data.to ? [{ name: data.to, email: data.to }] : [],
      subject: data.subject || '(no subject)',
      body: data.body,
      date: new Date(), read: true, starred: false, labels: ['draft'], folder: 'drafts',
    };
    emails.update((prev) => [draft, ...prev]);
    composeOpen.set(false);
    composeData.set(EMPTY_COMPOSE);
  }

  function openReply(mode: 'reply' | 'replyAll' | 'forward') {
    const email = get(selectedEmail);
    if (!email) return;
    replyMode.set(mode);
    composeData.set({
      to: mode === 'forward' ? '' : email.from.email,
      cc: mode === 'replyAll' ? email.to.map((t) => t.email).join(', ') : '',
      subject: mode === 'forward' ? `Fwd: ${email.subject}` : `Re: ${email.subject}`,
      body: mode === 'forward'
        ? `\n\n-------- Forwarded Message --------\nFrom: ${email.from.name} <${email.from.email}>\nDate: ${email.date.toLocaleString()}\nSubject: ${email.subject}\n\n${email.body}`
        : `\n\n-------- Original Message --------\nFrom: ${email.from.name} <${email.from.email}>\nDate: ${email.date.toLocaleString()}\n\n${email.body}`,
      replyTo: email,
    });
    composeOpen.set(true);
    if (mode === 'replyAll') showCc.set(true);
  }

  function bulkAction(action: 'read' | 'unread' | 'star' | 'delete' | 'archive') {
    const ids = get(selectedIds);
    emails.update((prev) => prev.map((e) => {
      if (!ids.includes(e.id)) return e;
      if (action === 'read') return { ...e, read: true };
      if (action === 'unread') return { ...e, read: false };
      if (action === 'star') return { ...e, starred: !e.starred };
      if (action === 'delete') return { ...e, folder: 'trash' as FolderType };
      if (action === 'archive') return { ...e, folder: 'archive' as FolderType };
      return e;
    }));
    selectedIds.set([]);
  }

  function toggleSelectId(id: string, checked: boolean) {
    selectedIds.update((prev) => (checked ? [...prev, id] : prev.filter((x) => x !== id)));
  }

  function selectFolder(folder: FolderType) { activeFolder.set(folder); selectedEmail.set(null); selectedIds.set([]); }

  function formatDate(date: Date): string {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    if (diff < 86400000) return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    if (diff < 604800000) return date.toLocaleDateString([], { weekday: 'short' });
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }

  return {
    emails, activeFolder, selectedEmail, composeOpen, composeData, searchQuery,
    loading, selectedIds, showCc, replyMode, displayEmails, totalUnread, syncStatus,
    fetchFromServer, unreadCount, markRead, toggleStar, moveToFolder, deleteEmail, selectEmail,
    openCompose, closeCompose, sendEmail, saveDraft, openReply, bulkAction,
    toggleSelectId, selectFolder, formatDate,
  };
}

export type MailState = ReturnType<typeof createMailState>;
