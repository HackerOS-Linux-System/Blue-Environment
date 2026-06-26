import React, { useState } from 'react';
import { X, Settings, Lock, Mail, Server, Eye, EyeOff } from 'lucide-react';
import { MailAccount } from './useAccounts';

interface Props {
    account?: MailAccount | null;
    onSave: (a: MailAccount) => void;
    onClose: () => void;
}

const PRESETS: Record<string, Partial<MailAccount>> = {
    gmail:   { imapHost: 'imap.gmail.com',   imapPort: 993, smtpHost: 'smtp.gmail.com',      smtpPort: 587, useSsl: true },
    outlook: { imapHost: 'outlook.office365.com', imapPort: 993, smtpHost: 'smtp.office365.com', smtpPort: 587, useSsl: true },
    yahoo:   { imapHost: 'imap.mail.yahoo.com', imapPort: 993, smtpHost: 'smtp.mail.yahoo.com', smtpPort: 587, useSsl: true },
    icloud:  { imapHost: 'imap.mail.me.com',  imapPort: 993, smtpHost: 'smtp.mail.me.com',    smtpPort: 587, useSsl: true },
};

const AccountModal: React.FC<Props> = ({ account, onSave, onClose }) => {
    const [form, setForm] = useState<MailAccount>(account ?? {
        id: Date.now().toString(), name: '', email: '', imapHost: '', imapPort: 993,
        smtpHost: '', smtpPort: 587, username: '', password: '', useSsl: true,
    });
    const [showPass, setShowPass] = useState(false);
    const [preset, setPreset] = useState('');

    const set = (k: keyof MailAccount, v: any) => setForm(f => ({ ...f, [k]: v }));

    const applyPreset = (p: string) => {
        setPreset(p);
        if (PRESETS[p]) setForm(f => ({ ...f, ...PRESETS[p] }));
    };

    const handleSave = () => {
        if (!form.email || !form.imapHost || !form.smtpHost) return;
        if (!form.name) setForm(f => ({ ...f, name: f.email.split('@')[0] }));
        onSave({ ...form, name: form.name || form.email.split('@')[0], username: form.username || form.email });
        onClose();
    };

    const field = (label: string, key: keyof MailAccount, type = 'text', placeholder = '') => (
        <div>
            <label className="block text-xs text-slate-400 mb-1">{label}</label>
            {key === 'password' ? (
                <div className="relative">
                    <input type={showPass ? 'text' : 'password'} value={form[key] as string ?? ''}
                        onChange={e => set(key, e.target.value)} placeholder={placeholder}
                        className="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50 pr-9"/>
                    <button type="button" onClick={() => setShowPass(s => !s)} className="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white">
                        {showPass ? <EyeOff size={14}/> : <Eye size={14}/>}
                    </button>
                </div>
            ) : (
                <input type={type} value={String(form[key] ?? '')}
                    onChange={e => set(key, type === 'number' ? parseInt(e.target.value) || 0 : e.target.value)}
                    placeholder={placeholder}
                    className="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50"/>
            )}
        </div>
    );

    return (
        <div className="absolute inset-0 bg-black/60 flex items-center justify-center z-50">
            <div className="bg-slate-900 border border-white/10 rounded-2xl w-[480px] max-h-[90%] overflow-y-auto shadow-2xl">
                <div className="flex items-center justify-between px-5 py-4 border-b border-white/5">
                    <div className="flex items-center gap-2">
                        <Settings size={16} className="text-blue-400"/>
                        <span className="font-semibold">{account ? 'Edit Account' : 'Add Account'}</span>
                    </div>
                    <button onClick={onClose} className="text-slate-400 hover:text-white"><X size={16}/></button>
                </div>

                <div className="p-5 space-y-4">
                    {/* Provider presets */}
                    <div>
                        <label className="block text-xs text-slate-400 mb-2">Quick setup</label>
                        <div className="flex gap-2 flex-wrap">
                            {Object.keys(PRESETS).map(p => (
                                <button key={p} onClick={() => applyPreset(p)}
                                    className={`px-3 py-1.5 rounded-lg text-xs capitalize transition-colors ${preset === p ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}`}>
                                    {p}
                                </button>
                            ))}
                        </div>
                    </div>

                    <div className="grid grid-cols-2 gap-3">
                        {field('Display name', 'name', 'text', 'Your Name')}
                        {field('Email address', 'email', 'email', 'you@example.com')}
                    </div>

                    <div className="grid grid-cols-2 gap-3">
                        {field('Login username', 'username', 'text', 'same as email')}
                        {field('Password', 'password', 'password', 'App password / token')}
                    </div>

                    <div className="border-t border-white/5 pt-3">
                        <div className="flex items-center gap-2 text-xs text-slate-400 mb-3">
                            <Server size={12}/> Incoming (IMAP)
                        </div>
                        <div className="grid grid-cols-3 gap-3">
                            <div className="col-span-2">{field('Host', 'imapHost', 'text', 'imap.example.com')}</div>
                            {field('Port', 'imapPort', 'number')}
                        </div>
                    </div>

                    <div className="border-t border-white/5 pt-3">
                        <div className="flex items-center gap-2 text-xs text-slate-400 mb-3">
                            <Mail size={12}/> Outgoing (SMTP)
                        </div>
                        <div className="grid grid-cols-3 gap-3">
                            <div className="col-span-2">{field('Host', 'smtpHost', 'text', 'smtp.example.com')}</div>
                            {field('Port', 'smtpPort', 'number')}
                        </div>
                    </div>

                    <label className="flex items-center gap-2 text-sm text-slate-300 cursor-pointer">
                        <input type="checkbox" checked={form.useSsl} onChange={e => set('useSsl', e.target.checked)}
                            className="w-4 h-4 accent-blue-500"/>
                        <Lock size={13} className="text-green-400"/> Use SSL/TLS
                    </label>

                    <div className="text-xs text-slate-500 bg-slate-800/50 rounded-lg p-3">
                        <strong className="text-slate-400">Note for Gmail/Outlook:</strong> Use an app-specific password, not your main account password. Enable IMAP access in your provider's settings.
                    </div>
                </div>

                <div className="flex justify-end gap-2 px-5 py-4 border-t border-white/5">
                    <button onClick={onClose} className="px-4 py-2 text-sm text-slate-400 hover:text-white">Cancel</button>
                    <button onClick={handleSave} disabled={!form.email || !form.imapHost}
                        className="px-4 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-xl text-sm text-white">
                        {account ? 'Save changes' : 'Add account'}
                    </button>
                </div>
            </div>
        </div>
    );
};

export default AccountModal;
