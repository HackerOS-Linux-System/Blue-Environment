import React, { useState, useEffect, useRef } from 'react';
import { AppProps } from '../../types';
import { useLanguage } from '../../contexts/LanguageContext';
import { SystemBridge } from '../../utils/systemBridge';
import {
    Inbox, Send, FileText, Plus, Trash2, Star, StarOff,
    Reply, ReplyAll, Forward, Search, RefreshCw, Tag,
    ChevronLeft, Paperclip, X, Check, AlertCircle, Loader2,
    Mail as MailIcon, Archive, ShieldX, Settings, User
} from 'lucide-react';

interface EmailAddress {
    name: string;
    email: string;
}

interface Attachment {
    name: string;
    size: string;
    type: string;
}

interface Email {
    id: string;
    from: EmailAddress;
    to: EmailAddress[];
    cc?: EmailAddress[];
    subject: string;
    body: string;
    bodyHtml?: string;
    date: Date;
    read: boolean;
    starred: boolean;
    labels: string[];
    attachments?: Attachment[];
    folder: 'inbox' | 'sent' | 'drafts' | 'archive' | 'spam' | 'trash';
    threadId?: string;
}

interface ComposeData {
    to: string;
    cc: string;
    subject: string;
    body: string;
    replyTo?: Email;
}

const DEMO_EMAILS: Email[] = [
    {
        id: '1',
        from: { name: 'Blue Environment', email: 'noreply@blue.env' },
        to: [{ name: 'You', email: 'user@blue.env' }],
        subject: 'Welcome to Blue Mail!',
        body: 'Welcome to Blue Mail — a production-ready email client built into Blue Environment.\n\nFeatures:\n• Rich email viewer\n• Compose with CC/BCC\n• Star important emails\n• Archive & labels\n• Search across folders\n\nEnjoy!',
        date: new Date(),
        read: false,
        starred: true,
        labels: ['important'],
        folder: 'inbox',
    },
    {
        id: '2',
        from: { name: 'HackerOS Team', email: 'team@hackeros.dev' },
        to: [{ name: 'You', email: 'user@blue.env' }],
        subject: 'Blue Environment v0.4.0 Release Notes',
        body: 'Blue Environment v0.4.0 is here!\n\nNew features:\n• Production Wayland compositor (Smithay)\n• XWayland for X11 app support\n• DRM/KMS backend for bare-metal use\n• VirtualBox / VM nested rendering\n• Alt+Tab window switcher\n• 4 workspaces\n• Full keyboard shortcuts\n• External window tracking\n\nEnjoy the new release!',
        date: new Date(Date.now() - 3600000),
        read: true,
        starred: false,
        labels: ['update'],
        folder: 'inbox',
    },
    {
        id: '3',
        from: { name: 'System Monitor', email: 'monitor@blue.env' },
        to: [{ name: 'You', email: 'user@blue.env' }],
        subject: 'System Report: All services running',
        body: 'Daily system report:\n\nCPU: Normal load\nMemory: 45% used\nDisk: 60% used\nNetwork: Connected (BlueNet 5G)\n\nAll services running normally.',
        date: new Date(Date.now() - 86400000),
        read: true,
        starred: false,
        labels: [],
        folder: 'inbox',
    },
];

const FOLDERS = [
    { id: 'inbox', icon: Inbox, label: 'Inbox' },
    { id: 'sent', icon: Send, label: 'Sent' },
    { id: 'drafts', icon: FileText, label: 'Drafts' },
    { id: 'archive', icon: Archive, label: 'Archive' },
    { id: 'spam', icon: ShieldX, label: 'ShieldX' },
    { id: 'trash', icon: Trash2, label: 'Trash' },
] as const;

type FolderType = typeof FOLDERS[number]['id'];

const MailApp: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [emails, setEmails] = useState<Email[]>(DEMO_EMAILS);
    const [activeFolder, setActiveFolder] = useState<FolderType>('inbox');
    const [selectedEmail, setSelectedEmail] = useState<Email | null>(null);
    const [composeOpen, setComposeOpen] = useState(false);
    const [composeData, setComposeData] = useState<ComposeData>({ to: '', cc: '', subject: '', body: '' });
    const [searchQuery, setSearchQuery] = useState('');
    const [loading, setLoading] = useState(false);
    const [selectedIds, setSelectedIds] = useState<string[]>([]);
    const [showCc, setShowCc] = useState(false);
    const [replyMode, setReplyMode] = useState<'reply' | 'replyAll' | 'forward' | null>(null);
    const searchRef = useRef<HTMLInputElement>(null);

    // Count unread per folder
    const unreadCount = (folder: FolderType) =>
        emails.filter(e => e.folder === folder && !e.read).length;

    const totalUnread = emails.filter(e => e.folder === 'inbox' && !e.read).length;

    // Filtered email list
    const displayEmails = emails.filter(e => {
        const matchFolder = e.folder === activeFolder;
        const q = searchQuery.toLowerCase();
        const matchSearch = !q || e.subject.toLowerCase().includes(q) ||
            e.from.name.toLowerCase().includes(q) ||
            e.from.email.toLowerCase().includes(q) ||
            e.body.toLowerCase().includes(q);
        return matchFolder && matchSearch;
    }).sort((a, b) => b.date.getTime() - a.date.getTime());

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

    const sendEmail = () => {
        if (!composeData.to.trim() || !composeData.subject.trim()) {
            alert('Please fill in To and Subject fields');
            return;
        }
        const newEmail: Email = {
            id: Date.now().toString(),
            from: { name: SystemBridge.getUsername(), email: 'user@blue.env' },
            to: [{ name: composeData.to, email: composeData.to }],
            subject: composeData.subject,
            body: composeData.body,
            date: new Date(),
            read: true,
            starred: false,
            labels: [],
            folder: 'sent',
        };
        setEmails(prev => [newEmail, ...prev]);
        setComposeOpen(false);
        setComposeData({ to: '', cc: '', subject: '', body: '' });
        setReplyMode(null);
        setShowCc(false);
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
        setComposeData({ to: '', cc: '', subject: '', body: '' });
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

    const formatDate = (date: Date): string => {
        const now = new Date();
        const diff = now.getTime() - date.getTime();
        if (diff < 86400000) return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
        if (diff < 604800000) return date.toLocaleDateString([], { weekday: 'short' });
        return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
    };

    return (
        <div className="flex h-full bg-slate-900 text-white">
            {/* Sidebar */}
            <div className="w-52 bg-slate-800/50 border-r border-white/5 flex flex-col">
                <div className="p-3">
                    <button
                        onClick={() => { setComposeOpen(true); setComposeData({ to: '', cc: '', subject: '', body: '' }); setReplyMode(null); }}
                        className="w-full bg-blue-600 hover:bg-blue-500 text-white py-2.5 rounded-xl flex items-center justify-center gap-2 font-medium transition-colors shadow-lg shadow-blue-500/20"
                    >
                        <Plus size={18} /> {t('mail.compose')}
                    </button>
                </div>

                {/* Search */}
                <div className="px-3 pb-2">
                    <div className="flex items-center gap-2 bg-slate-700/50 rounded-lg px-3 py-2">
                        <Search size={13} className="text-slate-400 shrink-0" />
                        <input
                            ref={searchRef}
                            type="text"
                            placeholder="Search mail..."
                            value={searchQuery}
                            onChange={e => setSearchQuery(e.target.value)}
                            className="bg-transparent text-sm text-white flex-1 focus:outline-none placeholder-slate-500"
                        />
                        {searchQuery && <button onClick={() => setSearchQuery('')}><X size={12} className="text-slate-400 hover:text-white" /></button>}
                    </div>
                </div>

                {/* Folders */}
                <div className="flex-1 overflow-y-auto px-2 space-y-0.5">
                    {FOLDERS.map(folder => {
                        const count = unreadCount(folder.id);
                        return (
                            <button
                                key={folder.id}
                                onClick={() => { setActiveFolder(folder.id); setSelectedEmail(null); setSelectedIds([]); }}
                                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors text-sm ${
                                    activeFolder === folder.id
                                        ? 'bg-blue-600 text-white'
                                        : 'text-slate-400 hover:bg-white/5 hover:text-white'
                                }`}
                            >
                                <folder.icon size={16} />
                                <span className="flex-1 text-left">{folder.label}</span>
                                {count > 0 && (
                                    <span className={`text-xs px-1.5 py-0.5 rounded-full font-medium ${
                                        activeFolder === folder.id ? 'bg-white/20 text-white' : 'bg-blue-600/30 text-blue-400'
                                    }`}>
                                        {count}
                                    </span>
                                )}
                            </button>
                        );
                    })}
                </div>

                {/* Account */}
                <div className="p-3 border-t border-white/5">
                    <div className="flex items-center gap-2 px-2">
                        <div className="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-xs font-bold">
                            {SystemBridge.getUsername().charAt(0).toUpperCase()}
                        </div>
                        <div className="flex-1 min-w-0">
                            <div className="text-xs font-medium text-white truncate">{SystemBridge.getUsername()}</div>
                            <div className="text-[10px] text-slate-500 truncate">@blue.env</div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Email list */}
            <div className={`border-r border-white/5 flex flex-col ${selectedEmail ? 'w-72' : 'flex-1'}`}>
                {/* List header */}
                <div className="h-12 bg-slate-800/50 border-b border-white/5 flex items-center px-3 gap-2">
                    <span className="font-semibold text-white capitalize flex-1">{activeFolder}</span>
                    <button onClick={() => setLoading(l => !l)} className="p-1.5 hover:bg-white/10 rounded-lg" title="Refresh">
                        <RefreshCw size={15} className={loading ? 'animate-spin' : ''} />
                    </button>
                    {selectedIds.length > 0 && (
                        <div className="flex items-center gap-1">
                            <button onClick={() => bulkAction('read')} className="p-1.5 hover:bg-white/10 rounded text-xs text-slate-400 hover:text-white" title="Mark read">
                                <Check size={14} />
                            </button>
                            <button onClick={() => bulkAction('archive')} className="p-1.5 hover:bg-white/10 rounded text-xs text-slate-400 hover:text-white" title="Archive">
                                <Archive size={14} />
                            </button>
                            <button onClick={() => bulkAction('delete')} className="p-1.5 hover:bg-red-500/20 rounded text-red-400 hover:text-red-300" title="Delete">
                                <Trash2 size={14} />
                            </button>
                            <span className="text-xs text-slate-500">{selectedIds.length}</span>
                        </div>
                    )}
                </div>

                {/* Emails */}
                <div className="flex-1 overflow-y-auto">
                    {displayEmails.length === 0 && (
                        <div className="flex flex-col items-center justify-center h-full text-slate-500 gap-3">
                            <MailIcon size={40} className="opacity-20" />
                            <p className="text-sm">{searchQuery ? 'No results found' : 'No emails'}</p>
                        </div>
                    )}
                    {displayEmails.map(email => (
                        <div
                            key={email.id}
                            onClick={() => selectEmail(email)}
                            className={`p-3 border-b border-white/5 cursor-pointer transition-colors group ${
                                selectedEmail?.id === email.id
                                    ? 'bg-blue-600/20 border-l-2 border-l-blue-500'
                                    : !email.read
                                    ? 'bg-slate-800/30 hover:bg-white/5'
                                    : 'hover:bg-white/5'
                            }`}
                        >
                            <div className="flex items-start gap-2">
                                {/* Checkbox */}
                                <input
                                    type="checkbox"
                                    checked={selectedIds.includes(email.id)}
                                    onChange={e => {
                                        e.stopPropagation();
                                        setSelectedIds(prev =>
                                            e.target.checked
                                                ? [...prev, email.id]
                                                : prev.filter(id => id !== email.id)
                                        );
                                    }}
                                    onClick={e => e.stopPropagation()}
                                    className="mt-1 accent-blue-500 opacity-0 group-hover:opacity-100 transition-opacity"
                                />

                                {/* Avatar */}
                                <div className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold shrink-0 ${
                                    email.from.name === 'Blue Environment' ? 'bg-blue-600' : 'bg-slate-700'
                                }`}>
                                    {email.from.name.charAt(0).toUpperCase()}
                                </div>

                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center justify-between gap-1">
                                        <span className={`text-sm truncate ${!email.read ? 'font-semibold text-white' : 'text-slate-300'}`}>
                                            {email.from.name}
                                        </span>
                                        <div className="flex items-center gap-1 shrink-0">
                                            {email.starred && <Star size={12} className="text-yellow-400 fill-yellow-400" />}
                                            <span className="text-[10px] text-slate-500">{formatDate(email.date)}</span>
                                        </div>
                                    </div>
                                    <div className={`text-xs truncate mt-0.5 ${!email.read ? 'font-medium text-slate-200' : 'text-slate-400'}`}>
                                        {email.subject}
                                    </div>
                                    <div className="text-[11px] text-slate-500 truncate mt-0.5">
                                        {email.body.split('\n')[0]}
                                    </div>
                                </div>
                            </div>
                        </div>
                    ))}
                </div>
            </div>

            {/* Email viewer */}
            {selectedEmail && (
                <div className="flex-1 flex flex-col overflow-hidden">
                    {/* Email header */}
                    <div className="p-4 border-b border-white/5 bg-slate-800/30">
                        <div className="flex items-start justify-between gap-3">
                            <div className="flex-1 min-w-0">
                                <h2 className="text-lg font-semibold text-white">{selectedEmail.subject}</h2>
                                <div className="flex items-center gap-2 mt-1">
                                    <div className="w-7 h-7 rounded-full bg-blue-700 flex items-center justify-center text-xs font-bold shrink-0">
                                        {selectedEmail.from.name.charAt(0).toUpperCase()}
                                    </div>
                                    <div>
                                        <div className="text-sm font-medium text-white">{selectedEmail.from.name}</div>
                                        <div className="text-xs text-slate-400">&lt;{selectedEmail.from.email}&gt;</div>
                                    </div>
                                </div>
                                <div className="flex items-center gap-3 mt-1.5 text-xs text-slate-500">
                                    <span>To: {selectedEmail.to.map(t => t.email).join(', ')}</span>
                                    <span>{selectedEmail.date.toLocaleString()}</span>
                                </div>
                            </div>
                            {/* Actions */}
                            <div className="flex items-center gap-1 shrink-0">
                                <button onClick={() => toggleStar(selectedEmail.id)} className="p-2 hover:bg-white/10 rounded-lg" title="Star">
                                    {selectedEmail.starred
                                        ? <Star size={16} className="text-yellow-400 fill-yellow-400" />
                                        : <StarOff size={16} className="text-slate-400" />
                                    }
                                </button>
                                <button onClick={() => openReply('reply')} className="p-2 hover:bg-white/10 rounded-lg" title="Reply">
                                    <Reply size={16} className="text-slate-400" />
                                </button>
                                <button onClick={() => openReply('replyAll')} className="p-2 hover:bg-white/10 rounded-lg" title="Reply All">
                                    <ReplyAll size={16} className="text-slate-400" />
                                </button>
                                <button onClick={() => openReply('forward')} className="p-2 hover:bg-white/10 rounded-lg" title="Forward">
                                    <Forward size={16} className="text-slate-400" />
                                </button>
                                <button onClick={() => moveToFolder(selectedEmail.id, 'archive')} className="p-2 hover:bg-white/10 rounded-lg" title="Archive">
                                    <Archive size={16} className="text-slate-400" />
                                </button>
                                <button onClick={() => deleteEmail(selectedEmail.id)} className="p-2 hover:bg-red-500/20 rounded-lg" title="Delete">
                                    <Trash2 size={16} className="text-red-400" />
                                </button>
                                <button onClick={() => setSelectedEmail(null)} className="p-2 hover:bg-white/10 rounded-lg ml-1">
                                    <X size={16} className="text-slate-400" />
                                </button>
                            </div>
                        </div>
                    </div>

                    {/* Email body */}
                    <div className="flex-1 overflow-y-auto p-6">
                        <div className="max-w-3xl">
                            <pre className="text-sm text-slate-200 whitespace-pre-wrap break-words font-sans leading-relaxed">
                                {selectedEmail.body}
                            </pre>
                            {selectedEmail.attachments && selectedEmail.attachments.length > 0 && (
                                <div className="mt-6 pt-4 border-t border-white/5">
                                    <div className="text-xs font-medium text-slate-400 mb-2 flex items-center gap-1">
                                        <Paperclip size={12} /> {selectedEmail.attachments.length} attachment(s)
                                    </div>
                                    <div className="flex gap-2 flex-wrap">
                                        {selectedEmail.attachments.map((att, i) => (
                                            <div key={i} className="flex items-center gap-2 bg-slate-800 rounded-lg px-3 py-2 text-xs">
                                                <FileText size={14} className="text-blue-400" />
                                                <span className="text-slate-200">{att.name}</span>
                                                <span className="text-slate-500">{att.size}</span>
                                            </div>
                                        ))}
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            )}

            {/* Compose modal */}
            {composeOpen && (
                <div className="fixed inset-0 bg-black/60 flex items-end justify-end z-50 p-4 pb-16 backdrop-blur-sm">
                    <div className="bg-slate-800 rounded-2xl w-[540px] shadow-2xl border border-white/10 flex flex-col max-h-[80vh]">
                        {/* Compose header */}
                        <div className="flex items-center justify-between px-4 py-3 border-b border-white/5 bg-slate-700/50 rounded-t-2xl">
                            <span className="font-semibold text-white">
                                {replyMode === 'forward' ? 'Forward' : replyMode ? 'Reply' : t('mail.compose')}
                            </span>
                            <div className="flex gap-1">
                                <button onClick={saveDraft} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white text-xs px-2">
                                    Save draft
                                </button>
                                <button onClick={() => setComposeOpen(false)} className="p-1.5 hover:bg-white/10 rounded-lg">
                                    <X size={16} className="text-slate-400" />
                                </button>
                            </div>
                        </div>

                        {/* Fields */}
                        <div className="border-b border-white/5">
                            <div className="flex items-center px-4 py-2 border-b border-white/5">
                                <span className="text-xs text-slate-500 w-10">To</span>
                                <input
                                    type="email"
                                    value={composeData.to}
                                    onChange={e => setComposeData(d => ({ ...d, to: e.target.value }))}
                                    placeholder="recipient@example.com"
                                    className="flex-1 bg-transparent text-sm text-white focus:outline-none"
                                    autoFocus={!replyMode}
                                />
                                <button onClick={() => setShowCc(!showCc)} className="text-xs text-slate-500 hover:text-white">CC</button>
                            </div>
                            {showCc && (
                                <div className="flex items-center px-4 py-2 border-b border-white/5">
                                    <span className="text-xs text-slate-500 w-10">CC</span>
                                    <input
                                        type="text"
                                        value={composeData.cc}
                                        onChange={e => setComposeData(d => ({ ...d, cc: e.target.value }))}
                                        placeholder="cc@example.com"
                                        className="flex-1 bg-transparent text-sm text-white focus:outline-none"
                                    />
                                </div>
                            )}
                            <div className="flex items-center px-4 py-2">
                                <span className="text-xs text-slate-500 w-10">{t('mail.subject')}</span>
                                <input
                                    type="text"
                                    value={composeData.subject}
                                    onChange={e => setComposeData(d => ({ ...d, subject: e.target.value }))}
                                    placeholder="Subject"
                                    className="flex-1 bg-transparent text-sm text-white focus:outline-none font-medium"
                                />
                            </div>
                        </div>

                        {/* Body */}
                        <textarea
                            value={composeData.body}
                            onChange={e => setComposeData(d => ({ ...d, body: e.target.value }))}
                            placeholder={t('mail.body')}
                            className="flex-1 bg-transparent text-sm text-slate-200 focus:outline-none p-4 resize-none min-h-[200px]"
                        />

                        {/* Footer */}
                        <div className="flex items-center justify-between px-4 py-3 border-t border-white/5 bg-slate-700/30 rounded-b-2xl">
                            <div className="flex gap-2">
                                <button className="p-2 hover:bg-white/10 rounded-lg" title="Attach file">
                                    <Paperclip size={16} className="text-slate-400" />
                                </button>
                            </div>
                            <button
                                onClick={sendEmail}
                                disabled={!composeData.to.trim() || !composeData.subject.trim()}
                                className="px-5 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium text-sm flex items-center gap-2 disabled:opacity-40 transition-colors"
                            >
                                <Send size={15} /> {t('mail.send')}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};

export default MailApp;
