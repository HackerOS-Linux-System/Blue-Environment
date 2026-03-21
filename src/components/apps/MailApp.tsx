import React, { useState } from 'react';
import { AppProps } from '../../types';
import { useLanguage } from '../../contexts/LanguageContext';
import { Inbox, Send, FileText, Plus, Mail, Trash2 } from 'lucide-react';

interface Email {
    id: string;
    from: string;
    subject: string;
    body: string;
    date: Date;
    read: boolean;
}

const MailApp: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [activeFolder, setActiveFolder] = useState<'inbox' | 'sent' | 'drafts'>('inbox');
    const [selectedEmail, setSelectedEmail] = useState<Email | null>(null);
    const [composeOpen, setComposeOpen] = useState(false);
    const [emails, setEmails] = useState<Record<string, Email[]>>({
        inbox: [
            { id: '1', from: 'no-reply@blue.com', subject: 'Welcome to Blue Mail', body: 'This is a welcome email.', date: new Date(), read: false },
        ],
        sent: [],
        drafts: [],
    });

    const handleSend = (to: string, subject: string, body: string) => {
        const newEmail: Email = {
            id: Date.now().toString(),
            from: 'me@blue.com',
            subject,
            body,
            date: new Date(),
            read: true,
        };
        setEmails(prev => ({
            ...prev,
            sent: [newEmail, ...prev.sent],
        }));
        setComposeOpen(false);
    };

    const currentEmails = emails[activeFolder];

    return (
        <div className="flex h-full bg-slate-900 text-white">
        {/* Sidebar */}
        <div className="w-56 bg-slate-800/50 border-r border-white/5 p-4">
        <button
        onClick={() => setComposeOpen(true)}
        className="w-full bg-blue-600 hover:bg-blue-500 text-white py-2 rounded-lg flex items-center justify-center gap-2 mb-6"
        >
        <Plus size={16} /> {t('mail.compose')}
        </button>
        <div className="space-y-1">
        <button
        onClick={() => setActiveFolder('inbox')}
        className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg ${activeFolder === 'inbox' ? 'bg-blue-600' : 'hover:bg-white/10'}`}
        >
        <Inbox size={18} /> {t('mail.inbox')}
        </button>
        <button
        onClick={() => setActiveFolder('sent')}
        className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg ${activeFolder === 'sent' ? 'bg-blue-600' : 'hover:bg-white/10'}`}
        >
        <Send size={18} /> {t('mail.sent')}
        </button>
        <button
        onClick={() => setActiveFolder('drafts')}
        className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg ${activeFolder === 'drafts' ? 'bg-blue-600' : 'hover:bg-white/10'}`}
        >
        <FileText size={18} /> {t('mail.drafts')}
        </button>
        </div>
        </div>

        {/* Email list */}
        <div className="w-80 border-r border-white/5 overflow-y-auto">
        {currentEmails.map(email => (
            <div
            key={email.id}
            onClick={() => setSelectedEmail(email)}
            className={`p-3 border-b border-white/5 cursor-pointer hover:bg-white/5 ${!email.read ? 'bg-blue-600/10' : ''}`}
            >
            <div className="font-medium">{email.from}</div>
            <div className="text-sm truncate">{email.subject}</div>
            <div className="text-xs text-slate-400">{email.date.toLocaleDateString()}</div>
            </div>
        ))}
        </div>

        {/* Email view */}
        <div className="flex-1 p-4 overflow-y-auto">
        {selectedEmail ? (
            <div>
            <div className="text-xl font-bold">{selectedEmail.subject}</div>
            <div className="text-sm text-slate-400 mt-1">From: {selectedEmail.from}</div>
            <div className="mt-4 whitespace-pre-wrap">{selectedEmail.body}</div>
            </div>
        ) : (
            <div className="flex items-center justify-center h-full text-slate-500">
            {t('mail.select_email')}
            </div>
        )}
        </div>

        {/* Compose modal */}
        {composeOpen && (
            <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-slate-800 rounded-2xl p-6 w-96 border border-white/10">
            <h3 className="text-lg font-bold mb-4">{t('mail.compose')}</h3>
            <input type="text" placeholder="To" className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 mb-2 text-white" />
            <input type="text" placeholder={t('mail.subject')} className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 mb-2 text-white" />
            <textarea placeholder={t('mail.body')} rows={5} className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white" />
            <div className="flex justify-end gap-2 mt-4">
            <button onClick={() => setComposeOpen(false)} className="px-4 py-2 bg-slate-700 rounded-lg">{t('settings.cancel')}</button>
            <button onClick={() => handleSend('test@example.com', 'Test', 'Test body')} className="px-4 py-2 bg-blue-600 rounded-lg">{t('mail.send')}</button>
            </div>
            </div>
            </div>
        )}
        </div>
    );
};

export default MailApp;
