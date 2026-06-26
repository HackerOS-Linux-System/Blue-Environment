import React from 'react';
import { RefreshCw, Check, Archive, Trash2, Star, Mail as MailIcon } from 'lucide-react';
import { MailState } from './useMailState';

interface Props {
    state: MailState;
}

const EmailList: React.FC<Props> = ({ state }) => {
    const {
        activeFolder, selectedEmail, selectedIds, loading, displayEmails, searchQuery,
        setLoading, bulkAction, toggleSelectId, selectEmail, formatDate,
    } = state;

    return (
        <div className={`border-r border-white/5 flex flex-col ${selectedEmail ? 'w-72' : 'flex-1'}`}>
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
                            <input
                                type="checkbox"
                                checked={selectedIds.includes(email.id)}
                                onChange={e => { e.stopPropagation(); toggleSelectId(email.id, e.target.checked); }}
                                onClick={e => e.stopPropagation()}
                                className="mt-1 accent-blue-500 opacity-0 group-hover:opacity-100 transition-opacity"
                            />

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
    );
};

export default EmailList;
