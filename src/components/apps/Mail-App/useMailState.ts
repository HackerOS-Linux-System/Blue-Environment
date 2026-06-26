import { useRef, useState, useCallback } from 'react';
import { SystemBridge } from '../../../utils/systemBridge';
import { useDialog } from '../../../contexts/DialogContext';
import { DEMO_EMAILS } from './demoData';
import { Email, ComposeData, FolderType, ReplyMode, EMPTY_COMPOSE } from './types';

// Remote email shape coming from the MailApp Rust backend
interface RemoteEmail {
    uid: string; from: string; to: string;
    subject: string; date: string; body: string; read: boolean;
}

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

export function useMailState(activeAccountId?: string) {
    const dialog = useDialog();
    const [emails, setEmails] = useState<Email[]>(DEMO_EMAILS);
    const [activeFolder, setActiveFolder] = useState<FolderType>('inbox');
    const [selectedEmail, setSelectedEmail] = useState<Email | null>(null);
    const [composeOpen, setComposeOpen] = useState(false);
    const [composeData, setComposeData] = useState<ComposeData>(EMPTY_COMPOSE);
    const [searchQuery, setSearchQuery] = useState('');
    const [loading, setLoading] = useState(false);
    const [selectedIds, setSelectedIds] = useState<string[]>([]);
    const [showCc, setShowCc] = useState(false);
    const [replyMode, setReplyMode] = useState<ReplyMode>(null);
    const [syncStatus, setSyncStatus] = useState('');
    const searchRef = useRef<HTMLInputElement>(null);

    // ── Real IMAP sync ────────────────────────────────────────────────────
    const fetchFromServer = useCallback(async (accountId: string, folder: FolderType = 'inbox') => {
        if (!SystemBridge.isTauri() || !accountId) return;
        setSyncStatus('Syncing…');
        setLoading(true);
        try {
            const raw = await SystemBridge.invoke<RemoteEmail[]>('mail_fetch_inbox', {
                accountId, folder: folder.toUpperCase(), limit: 50,
            });
            if (raw?.length) {
                const fetched = raw.map(r => remoteToEmail(r, accountId));
                // Merge with existing local emails: keep local-only, replace IMAP ones
                setEmails(prev => {
                    const local = prev.filter(e => !e.id.startsWith(`imap-${accountId}-`));
                    return [...local, ...fetched];
                });
                setSyncStatus(`Synced ${fetched.length} messages`);
            } else {
                setSyncStatus('No messages or connection failed');
            }
        } catch (e: any) {
            setSyncStatus(`Sync error: ${e?.message ?? e}`);
        } finally {
            setLoading(false);
            setTimeout(() => setSyncStatus(''), 4000);
        }
    }, []);

    const sendViaBackend = useCallback(async (
        accountId: string, to: string, cc: string, subject: string, body: string,
    ): Promise<boolean> => {
        if (!SystemBridge.isTauri() || !accountId) return false;
        const result = await SystemBridge.invoke<{ success: boolean; error?: string }>('mail_send', {
            accountId, to, cc: cc || null, subject, body,
        });
        if (!result?.success && result?.error) {
            await dialog.alert({ title: 'Send failed', message: result.error });
            return false;
        }
        return !!result?.success;
    }, [dialog]);

    const unreadCount = (folder: FolderType) =>
        emails.filter(e => e.folder === folder && !e.read).length;

    const totalUnread = emails.filter(e => e.folder === 'inbox' && !e.read).length;

    const displayEmails = emails
        .filter(e => {
            const matchFolder = e.folder === activeFolder;
            const q = searchQuery.toLowerCase();
            const matchSearch = !q || e.subject.toLowerCase().includes(q) ||
                e.from.name.toLowerCase().includes(q) ||
                e.from.email.toLowerCase().includes(q) ||
                e.body.toLowerCase().includes(q);
            return matchFolder && matchSearch;
        })
        .sort((a, b) => b.date.getTime() - a.date.getTime());

    const markRead = (id: string, read = true) => {
        setEmails(prev => prev.map(e => e.id === id ? { ...e, read } : e));
    };

    const toggleStar = (id: string) => {
        setEmails(prev => prev.map(e => e.id === id ? { ...e, starred: !e.starred } : e));
    };

    const moveToFolder = (id: string, folder: FolderType) => {
        setEmails(prev => prev.map(e => e.id === id ? { ...e, folder } : e));
        if (selectedEmail?.id === id) setSelectedEmail(null);
    };

    const deleteEmail = (id: string) => {
        const email = emails.find(e => e.id === id);
        if (!email) return;
        if (email.folder === 'trash') {
            setEmails(prev => prev.filter(e => e.id !== id));
        } else {
            moveToFolder(id, 'trash');
        }
        if (selectedEmail?.id === id) setSelectedEmail(null);
    };

    const selectEmail = (email: Email) => {
        setSelectedEmail(email);
        if (!email.read) markRead(email.id);
    };

    const openCompose = () => {
        setComposeOpen(true);
        setComposeData(EMPTY_COMPOSE);
        setReplyMode(null);
        setShowCc(false);
    };

    const closeCompose = () => setComposeOpen(false);

    const sendEmail = async (accountId?: string | null) => {
        if (!composeData.to.trim() || !composeData.subject.trim()) {
            await dialog.alert({ title: 'Missing information', message: 'Please fill in the To and Subject fields before sending.' });
            return;
        }
        // Try real SMTP if account is configured
        if (accountId && SystemBridge.isTauri()) {
            const sent = await sendViaBackend(accountId, composeData.to, composeData.cc, composeData.subject, composeData.body);
            if (!sent) return; // error already shown
        }
        const newEmail: Email = {
            id: Date.now().toString(),
            from: { name: SystemBridge.getUsername(), email: 'user@blue.env' },
            to: [{ name: composeData.to, email: composeData.to }],
            subject: composeData.subject, body: composeData.body,
            date: new Date(), read: true, starred: false, labels: [], folder: 'sent',
        };
        setEmails(prev => [newEmail, ...prev]);
        setComposeOpen(false); setComposeData(EMPTY_COMPOSE); setReplyMode(null); setShowCc(false);
    };

    const saveDraft = () => {
        const draft: Email = {
            id: Date.now().toString(),
            from: { name: SystemBridge.getUsername(), email: 'user@blue.env' },
            to: composeData.to ? [{ name: composeData.to, email: composeData.to }] : [],
            subject: composeData.subject || '(no subject)',
            body: composeData.body,
            date: new Date(),
            read: true,
            starred: false,
            labels: ['draft'],
            folder: 'drafts',
        };
        setEmails(prev => [draft, ...prev]);
        setComposeOpen(false);
        setComposeData(EMPTY_COMPOSE);
    };

    const openReply = (mode: 'reply' | 'replyAll' | 'forward') => {
        if (!selectedEmail) return;
        setReplyMode(mode);
        setComposeData({
            to: mode === 'forward' ? '' : selectedEmail.from.email,
            cc: mode === 'replyAll' ? selectedEmail.to.map(t => t.email).join(', ') : '',
            subject: mode === 'forward'
                ? `Fwd: ${selectedEmail.subject}`
                : `Re: ${selectedEmail.subject}`,
            body: mode === 'forward'
                ? `\n\n-------- Forwarded Message --------\nFrom: ${selectedEmail.from.name} <${selectedEmail.from.email}>\nDate: ${selectedEmail.date.toLocaleString()}\nSubject: ${selectedEmail.subject}\n\n${selectedEmail.body}`
                : `\n\n-------- Original Message --------\nFrom: ${selectedEmail.from.name} <${selectedEmail.from.email}>\nDate: ${selectedEmail.date.toLocaleString()}\n\n${selectedEmail.body}`,
            replyTo: selectedEmail,
        });
        setComposeOpen(true);
        if (mode === 'replyAll') setShowCc(true);
    };

    const bulkAction = (action: 'read' | 'unread' | 'star' | 'delete' | 'archive') => {
        setEmails(prev => prev.map(e => {
            if (!selectedIds.includes(e.id)) return e;
            if (action === 'read') return { ...e, read: true };
            if (action === 'unread') return { ...e, read: false };
            if (action === 'star') return { ...e, starred: !e.starred };
            if (action === 'delete') return { ...e, folder: 'trash' as FolderType };
            if (action === 'archive') return { ...e, folder: 'archive' as FolderType };
            return e;
        }));
        setSelectedIds([]);
    };

    const toggleSelectId = (id: string, checked: boolean) => {
        setSelectedIds(prev => checked ? [...prev, id] : prev.filter(x => x !== id));
    };

    const selectFolder = (folder: FolderType) => {
        setActiveFolder(folder);
        setSelectedEmail(null);
        setSelectedIds([]);
    };

    const formatDate = (date: Date): string => {
        const now = new Date();
        const diff = now.getTime() - date.getTime();
        if (diff < 86400000) return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
        if (diff < 604800000) return date.toLocaleDateString([], { weekday: 'short' });
        return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
    };

    return {
        emails, activeFolder, selectedEmail, composeOpen, composeData, searchQuery,
        loading, selectedIds, showCc, replyMode, searchRef, displayEmails, totalUnread,
        syncStatus, fetchFromServer,
        setComposeData, setSearchQuery, setLoading, setShowCc, setSelectedEmail,
        unreadCount, markRead, toggleStar, moveToFolder, deleteEmail, selectEmail,
        openCompose, closeCompose, sendEmail, saveDraft, openReply, bulkAction,
        toggleSelectId, selectFolder, formatDate,
    };
}

export type MailState = ReturnType<typeof useMailState>;
