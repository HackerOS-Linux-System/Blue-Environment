import React from 'react';
import { X, Paperclip, Send } from 'lucide-react';
import { MailState } from './useMailState';

interface Props {
    state: MailState;
    t: (key: string) => string;
}

const ComposeModal: React.FC<Props> = ({ state, t }) => {
    const { composeOpen, composeData, setComposeData, showCc, setShowCc, replyMode, sendEmail, saveDraft, closeCompose } = state;
    if (!composeOpen) return null;

    return (
        <div className="fixed inset-0 bg-black/60 flex items-end justify-end z-50 p-4 pb-16 backdrop-blur-sm">
            <div className="bg-slate-800 rounded-2xl w-[540px] shadow-2xl border border-white/10 flex flex-col max-h-[80vh]">
                <div className="flex items-center justify-between px-4 py-3 border-b border-white/5 bg-slate-700/50 rounded-t-2xl">
                    <span className="font-semibold text-white">
                        {replyMode === 'forward' ? 'Forward' : replyMode ? 'Reply' : t('mail.compose')}
                    </span>
                    <div className="flex gap-1">
                        <button onClick={saveDraft} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white text-xs px-2">
                            Save draft
                        </button>
                        <button onClick={closeCompose} className="p-1.5 hover:bg-white/10 rounded-lg">
                            <X size={16} className="text-slate-400" />
                        </button>
                    </div>
                </div>

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

                <textarea
                    value={composeData.body}
                    onChange={e => setComposeData(d => ({ ...d, body: e.target.value }))}
                    placeholder={t('mail.body')}
                    className="flex-1 bg-transparent text-sm text-slate-200 focus:outline-none p-4 resize-none min-h-[200px]"
                />

                <div className="flex items-center justify-between px-4 py-3 border-t border-white/5 bg-slate-700/30 rounded-b-2xl">
                    <div className="flex gap-2">
                        <button className="p-2 hover:bg-white/10 rounded-lg" title="Attach file">
                            <Paperclip size={16} className="text-slate-400" />
                        </button>
                    </div>
                    <button
                        onClick={() => sendEmail()}
                        disabled={!composeData.to.trim() || !composeData.subject.trim()}
                        className="px-5 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium text-sm flex items-center gap-2 disabled:opacity-40 transition-colors"
                    >
                        <Send size={15} /> {t('mail.send')}
                    </button>
                </div>
            </div>
        </div>
    );
};

export default ComposeModal;
